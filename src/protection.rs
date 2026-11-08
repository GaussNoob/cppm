mod bundled;

use crate::{
    build::BuildOptions,
    conan::InstallOptions,
    project::Project,
    storage::{write_atomic, write_if_changed},
};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub struct Configuration {
    pub directory: PathBuf,
    plugin: Option<PathBuf>,
    plugin_digest: Option<String>,
    managed: Option<bundled::Plugin>,
}

impl Configuration {
    pub fn prepare(project: &Project, options: &BuildOptions) -> Result<Option<Self>> {
        if !options.obfuscate {
            return Ok(None);
        }
        ensure!(
            project.manifest.package.target.is_native(),
            "the obf preset supports native C++ only; WebAssembly protection is not supported"
        );
        ensure!(
            options.release,
            "the obf preset requires Release dependencies"
        );
        cmake_path(&project.root)?;
        let managed = if options.obf_plugin.is_none() && !options.obf_basic {
            Some(bundled::Plugin::prepare(project, options.jobs)?)
        } else {
            None
        };
        let plugin = options
            .obf_plugin
            .as_deref()
            .or_else(|| managed.as_ref().map(|plugin| plugin.path.as_path()))
            .map(|path| {
                let path = std::path::absolute(path)?;
                ensure!(
                    path.is_file(),
                    "LLVM pass plugin {} is not a file",
                    path.display()
                );
                cmake_path(&path)?;
                Ok(path)
            })
            .transpose()?;
        let plugin_digest = plugin
            .as_ref()
            .map(|path| {
                fs::read(path)
                    .map(|bytes| blake3::hash(&bytes).to_hex().to_string())
                    .with_context(|| format!("cannot read LLVM pass plugin {}", path.display()))
            })
            .transpose()?;
        let directory = project.root.join(".cppm/build/obf");
        write_if_changed(
            &directory.join("cppm-protection.cmake"),
            include_bytes!("../cmake/protection.cmake"),
        )?;
        // A missing/overridden CMake injection must never silently produce an
        // unprotected build under the obf preset. Reset before every configure.
        write_atomic(&directory.join("cppm-protection.applied"), b"")?;
        Ok(Some(Self {
            directory,
            plugin,
            plugin_digest,
            managed,
        }))
    }

    pub fn configure(&self, command: &mut Command, project: &Project) -> Result<()> {
        command
            .arg(format!(
                "-DCMAKE_PROJECT_INCLUDE={}",
                cmake_path(&self.directory.join("cppm-protection.cmake"))?
            ))
            .arg(format!(
                "-DCPPM_PROTECTED_ROOT={}",
                cmake_path(&project.root)?
            ))
            .arg(format!(
                "-DCPPM_PROTECTED_TARGET={}",
                project.manifest.package.name
            ));
        if let Some(plugin) = &self.plugin {
            command
                .arg(format!("-DCPPM_OBF_PLUGIN={}", cmake_path(plugin)?))
                .arg(format!(
                    "-DCPPM_OBF_PLUGIN_DIGEST={}",
                    self.plugin_digest.as_deref().unwrap_or_default()
                ));
        }
        if let Some(plugin) = &self.managed {
            command
                .arg("--no-warn-unused-cli")
                .arg(format!("-DCMAKE_CXX_COMPILER={}", cmake_path(&plugin.cxx)?))
                .arg(format!("-DCMAKE_C_COMPILER={}", cmake_path(&plugin.cc)?));
        }
        Ok(())
    }

    pub fn install_options(
        &self,
        project: &Project,
        options: &InstallOptions,
    ) -> Result<InstallOptions> {
        match &self.managed {
            Some(plugin) => plugin.install_options(project, options),
            None => Ok(options.clone()),
        }
    }

    pub fn environment(&self, command: &mut Command) {
        if self.managed.is_some() {
            bundled::environment(command);
        }
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            fs::read(self.directory.join("cppm-protection.applied"))
                .ok()
                .as_deref()
                == Some(b"applied\n"),
            "CMake did not apply the obf preset; check whether the project overrides CMAKE_PROJECT_INCLUDE"
        );
        Ok(())
    }

    pub fn finish(&self, project: &Project) -> Result<()> {
        let binary = self.directory.join("bin").join(format!(
            "{}{}",
            project.manifest.package.name,
            std::env::consts::EXE_SUFFIX
        ));
        ensure!(
            binary.is_file(),
            "protected executable {} is missing; the CMake target must match package.name",
            binary.display()
        );
        println!("Protected Release: {}", binary.display());
        println!(
            "IPO/LTO, reduced symbols, debug metadata removal and source path remapping enabled."
        );
        if let Some(managed) = &self.managed {
            println!(
                "Bundled Obfuscator-LLVM (LLVM {}): strings, control-flow flattening, instruction substitution and block splitting.",
                managed.version
            );
        } else if let Some(plugin) = &self.plugin {
            println!(
                "LLVM pass plugin: {} (transformations depend on the plugin).",
                plugin.display()
            );
        } else {
            println!(
                "No LLVM obfuscation plugin: this preset reduces metadata; it does not encrypt strings or transform control flow."
            );
        }
        Ok(())
    }
}

fn cmake_path(path: &Path) -> Result<String> {
    let path = path
        .to_str()
        .context("the obf preset requires UTF-8 paths")?;
    ensure!(
        !path.contains([';', '\n', '\r', '<', '>']),
        "the obf preset cannot use paths containing ';', newlines, '<' or '>'"
    );
    #[cfg(windows)]
    {
        Ok(path.replace('\\', "/"))
    }
    #[cfg(not(windows))]
    {
        Ok(path.to_owned())
    }
}

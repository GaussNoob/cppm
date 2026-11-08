use crate::{
    build, conan,
    manifest::{Dependency, Manifest},
    packages,
    project::Project,
    storage::write_if_changed,
    store,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

pub use crate::conan::InstallOptions;

pub fn install(project: &Project, release: bool, options: &InstallOptions) -> Result<()> {
    let (project, _guard) = project.locked()?;
    prepare(&project, release, options)?.commit(&project.root, options.frozen)?;
    build::refresh_editor(&project, release, options);
    Ok(())
}

pub fn add(
    project: &Project,
    specs: &[String],
    cmake_package: Option<String>,
    targets: Vec<String>,
    release: bool,
    options: &InstallOptions,
) -> Result<Vec<String>> {
    ensure!(!options.frozen, "add cannot be used with --frozen-lockfile");
    ensure!(!specs.is_empty(), "specify at least one package");
    ensure!(
        specs.len() == 1 || (cmake_package.is_none() && targets.is_empty()),
        "--cmake-package and --target require exactly one package"
    );
    // Validate the whole request before contacting Conan or changing project files.
    let mut requested = BTreeMap::new();
    for spec in specs {
        let (name, version) = packages::parse_spec(spec)?;
        if let Some(previous) = requested.get(&name) {
            ensure!(
                previous == &version,
                "conflicting versions requested for '{name}'"
            );
        }
        requested.insert(name, version);
    }
    let (mut project, _guard) = project.locked()?;
    ensure!(
        project.manifest.package.target.is_native(),
        "Conan dependencies for WebAssembly are not supported yet; native libraries cannot be linked into a WASM build"
    );
    let names: Vec<String> = requested.keys().cloned().collect();
    for (name, version) in requested {
        let version = match version {
            Some(version) => version,
            None => conan::resolve_version(&project, &name, release, options)?,
        };
        let dependency = packages::dependency(version, cmake_package.clone(), targets.clone())?;
        project.manifest.dependencies.insert(name, dependency);
    }
    let prepared = prepare(&project, release, options)?;
    project.save_manifest()?;
    prepared.commit(&project.root, false)?;
    build::refresh_editor(&project, release, options);
    if names.iter().any(|name| name == "sdl")
        && let Some(sdl) = project.manifest.dependencies.get("sdl")
    {
        let version = sdl.version();
        if version.starts_with("2.") {
            println!(
                "SDL {version} ready: use #include <SDL.h> (SDL2). SDL3 examples require `cppm add sdl3` and <SDL3/SDL.h>."
            );
        } else if version.starts_with("3.") {
            println!(
                "SDL {version} ready: use #include <SDL3/SDL.h> (SDL3). For SDL2 code using <SDL.h>, run `cppm add sdl2`."
            );
        }
    }
    Ok(names)
}

pub fn remove(project: &Project, name: &str, options: &InstallOptions) -> Result<()> {
    let name = packages::canonical_name(name);
    ensure!(
        !options.frozen,
        "remove cannot be used with --frozen-lockfile"
    );
    let (mut project, _guard) = project.locked()?;
    ensure!(
        project.manifest.dependencies.remove(&name).is_some(),
        "'{name}' is not a direct dependency"
    );
    let prepared = prepare(&project, false, options)?;
    project.save_manifest()?;
    prepared.commit(&project.root, false)?;
    build::refresh_editor(&project, false, options);
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Lockfile {
    format: u32,
    dependencies: BTreeMap<String, Dependency>,
    conan: Value,
}

pub struct Installation {
    lockfile: Lockfile,
    integration: String,
    registration: Option<store::Pending>,
}

impl Installation {
    pub fn commit(self, root: &Path, frozen: bool) -> Result<()> {
        write_if_changed(
            &root.join(".cppm/dependencies.cmake"),
            self.integration.as_bytes(),
        )?;
        if !frozen {
            let mut bytes = serde_json::to_vec_pretty(&self.lockfile)?;
            bytes.push(b'\n');
            write_if_changed(&root.join("cppm.lock"), &bytes)?;
        }
        if let Some(registration) = self.registration {
            registration.commit(root, self.lockfile.dependencies)?;
        } else if self.lockfile.dependencies.is_empty() {
            store::clear_project(root)?;
        }
        Ok(())
    }
}

fn read_lock(root: &Path) -> Result<Option<Lockfile>> {
    let path = root.join("cppm.lock");
    let source = match fs::read(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let lock: Lockfile = serde_json::from_slice(&source).context("invalid cppm.lock")?;
    ensure!(
        lock.format == 1,
        "unsupported cppm.lock format {}",
        lock.format
    );
    Ok(Some(lock))
}

fn integration(manifest: &Manifest, graph: &Value) -> Result<String> {
    let mut source = format!(
        "# Generated by cppm. Edit cppm.toml to change dependencies.\nset(CPPM_CXX_STANDARD {})\nadd_library(cppm_dependencies INTERFACE)\n",
        manifest.package.cpp_standard
    );
    for (name, dependency) in &manifest.dependencies {
        let (package, targets) = packages::mapping(name, dependency, graph)?;
        source.push_str(&format!("find_package({package} REQUIRED)\ntarget_link_libraries(cppm_dependencies INTERFACE {})\n", targets.join(" ")));
    }
    source.push_str(&packages::cmake_fixes(graph));
    Ok(source)
}

pub fn prepare(project: &Project, release: bool, options: &InstallOptions) -> Result<Installation> {
    prepare_with_cache(project, release, options, false)
}

pub fn prepare_build(
    project: &Project,
    release: bool,
    options: &InstallOptions,
    reuse: bool,
) -> Result<Installation> {
    prepare_with_cache(project, release, options, reuse)
}

fn prepare_with_cache(
    project: &Project,
    release: bool,
    options: &InstallOptions,
    reuse: bool,
) -> Result<Installation> {
    project.manifest.validate()?;
    let previous = read_lock(&project.root)?;
    if options.frozen {
        let previous = previous
            .as_ref()
            .context("cppm.lock is missing; run `cppm install` before using --frozen-lockfile")?;
        ensure!(
            previous.dependencies == project.manifest.dependencies,
            "cppm.toml and cppm.lock disagree; run `cppm install` to update the lockfile"
        );
    }
    let (graph, metadata, registration) = if project.manifest.dependencies.is_empty() {
        (Value::Null, Value::Null, None)
    } else {
        let installed = conan::install(
            project,
            release,
            options,
            previous.as_ref().map(|lock| &lock.conan),
            reuse,
        )?;
        if options.frozen
            && previous
                .as_ref()
                .is_some_and(|lock| lock.conan != installed.lock)
        {
            bail!("dependency graph changed; run `cppm install` without --frozen-lockfile");
        }
        (
            installed.lock,
            installed.graph,
            Some(installed.registration),
        )
    };
    Ok(Installation {
        lockfile: Lockfile {
            format: 1,
            dependencies: project.manifest.dependencies.clone(),
            conan: graph,
        },
        integration: integration(&project.manifest, &metadata)?,
        registration,
    })
}

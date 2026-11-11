use crate::{
    conan::InstallOptions,
    platform, process,
    project::Project,
    storage::{write_atomic, write_if_changed},
    store,
};
use anyhow::{Context, Result, ensure};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const FUNCTION_PASSES: &str = "flattening,substitution,split-basic-blocks";
const MODULE_PASSES: &str = "string-encryption";

const SOURCES: &[(&str, &[u8])] = &[
    (
        "CMakeLists.txt",
        include_bytes!("../../vendor/llvm-obfuscator/CMakeLists.txt"),
    ),
    (
        "embed-bitcode.cmake",
        include_bytes!("../../vendor/llvm-obfuscator/embed-bitcode.cmake"),
    ),
    (
        "Plugin.cpp",
        include_bytes!("../../vendor/llvm-obfuscator/Plugin.cpp"),
    ),
    (
        "LICENSE-OBFUSCATOR.TXT",
        include_bytes!("../../vendor/llvm-obfuscator/LICENSE-OBFUSCATOR.TXT"),
    ),
    (
        "UPSTREAM.md",
        include_bytes!("../../vendor/llvm-obfuscator/UPSTREAM.md"),
    ),
    (
        "utils/Utils.h",
        include_bytes!("../../vendor/llvm-obfuscator/utils/Utils.h"),
    ),
    (
        "utils/Utils.cpp",
        include_bytes!("../../vendor/llvm-obfuscator/utils/Utils.cpp"),
    ),
    (
        "utils/CryptoUtils.h",
        include_bytes!("../../vendor/llvm-obfuscator/utils/CryptoUtils.h"),
    ),
    (
        "utils/CryptoUtils.cpp",
        include_bytes!("../../vendor/llvm-obfuscator/utils/CryptoUtils.cpp"),
    ),
    (
        "bogus/BogusControlFlow.h",
        include_bytes!("../../vendor/llvm-obfuscator/bogus/BogusControlFlow.h"),
    ),
    (
        "bogus/BogusControlFlow.cpp",
        include_bytes!("../../vendor/llvm-obfuscator/bogus/BogusControlFlow.cpp"),
    ),
    (
        "flattening/Flattening.h",
        include_bytes!("../../vendor/llvm-obfuscator/flattening/Flattening.h"),
    ),
    (
        "flattening/Flattening.cpp",
        include_bytes!("../../vendor/llvm-obfuscator/flattening/Flattening.cpp"),
    ),
    (
        "split/SplitBasicBlocks.h",
        include_bytes!("../../vendor/llvm-obfuscator/split/SplitBasicBlocks.h"),
    ),
    (
        "split/SplitBasicBlocks.cpp",
        include_bytes!("../../vendor/llvm-obfuscator/split/SplitBasicBlocks.cpp"),
    ),
    (
        "substitution/Substitution.h",
        include_bytes!("../../vendor/llvm-obfuscator/substitution/Substitution.h"),
    ),
    (
        "substitution/Substitution.cpp",
        include_bytes!("../../vendor/llvm-obfuscator/substitution/Substitution.cpp"),
    ),
    (
        "string/StringObfuscation.h",
        include_bytes!("../../vendor/llvm-obfuscator/string/StringObfuscation.h"),
    ),
    (
        "string/StringObfuscation.cpp",
        include_bytes!("../../vendor/llvm-obfuscator/string/StringObfuscation.cpp"),
    ),
    (
        "string/decode.c",
        include_bytes!("../../vendor/llvm-obfuscator/string/decode.c"),
    ),
];

pub(super) struct Plugin {
    pub path: PathBuf,
    pub cxx: PathBuf,
    pub cc: PathBuf,
    pub version: String,
    directory: PathBuf,
}

fn capture(program: impl AsRef<OsStr>, args: &[&str]) -> Result<String> {
    let output = Command::new(program.as_ref()).args(args).output()
        .with_context(|| format!("cannot run {}; the bundled obfuscator requires Clang and a matching LLVM development SDK (llvm-config); use --obf-basic for metadata only", program.as_ref().to_string_lossy()))?;
    ensure!(
        output.status.success(),
        "LLVM tool failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn executable(program: &OsStr) -> Result<PathBuf> {
    let path = Path::new(program);
    if path.components().count() > 1 {
        let path = std::path::absolute(path)?;
        ensure!(path.is_file(), "compiler {} is missing", path.display());
        return Ok(path);
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join(path))
        .find(|path| path.is_file())
        .with_context(|| format!("compiler {} is not in PATH", path.display()))
}

impl Plugin {
    pub fn prepare(project: &Project, jobs: Option<usize>) -> Result<Self> {
        ensure!(
            cfg!(any(target_os = "linux", target_os = "macos")),
            "the bundled LLVM plugin currently supports Linux/macOS hosts; use --obf-basic or --obf-plugin with a compatible host on this platform"
        );
        let llvm = std::env::var_os("CPPM_LLVM_CONFIG").unwrap_or_else(|| "llvm-config".into());
        let version = capture(&llvm, &["--version"])?;
        let major: u32 = version
            .split('.')
            .next()
            .context("invalid LLVM version")?
            .parse()?;
        ensure!(
            (18..=23).contains(&major),
            "bundled obfuscation supports LLVM 18–23, found {version}; choose CPPM_LLVM_CONFIG or use --obf-basic"
        );
        let bindir = PathBuf::from(capture(&llvm, &["--bindir"])?);
        let cmakedir = PathBuf::from(capture(&llvm, &["--cmakedir"])?);
        let cc = executable(
            &std::env::var_os("CC").unwrap_or_else(|| bindir.join("clang").into_os_string()),
        )?;
        let cxx = executable(
            &std::env::var_os("CXX").unwrap_or_else(|| bindir.join("clang++").into_os_string()),
        )?;
        let cxx_version = capture(&cxx, &["--version"])?;
        for (compiler, info) in [
            (&cc, capture(&cc, &["--version"])?),
            (&cxx, cxx_version.clone()),
        ] {
            let found = info
                .split("clang version ")
                .nth(1)
                .and_then(|s| s.split_whitespace().next());
            ensure!(
                found == Some(version.as_str()),
                "{} must be Clang {version} to match llvm-config; CC/CXX currently selects a different compiler. Choose matching CC/CXX and CPPM_LLVM_CONFIG, or use --obf-basic",
                compiler.display()
            );
        }
        let mut digest = blake3::Hasher::new();
        for (name, contents) in SOURCES {
            digest.update(name.as_bytes());
            digest.update(contents);
        }
        digest.update(FUNCTION_PASSES.as_bytes());
        digest.update(MODULE_PASSES.as_bytes());
        for identity in [
            &version,
            &cxx_version,
            &cmakedir.display().to_string(),
            &cxx.display().to_string(),
        ] {
            digest.update(identity.as_bytes());
        }
        let key = digest.finalize().to_hex().to_string();
        let directory = project
            .root
            .join(".cppm/tools/obfuscator")
            .join(format!("llvm-{version}-{}", &key[..16]));
        let source = directory.join("source");
        let build = directory.join("build");
        let path = build.join("lib/CppmObfuscator.so");
        let seal = directory.join("plugin.blake3");
        let cached = fs::read(&path)
            .ok()
            .zip(fs::read_to_string(&seal).ok())
            .is_some_and(|(bytes, expected)| {
                blake3::hash(&bytes).to_hex().as_str() == expected.trim()
            });
        if !cached {
            println!(
                "Preparing bundled Obfuscator-LLVM for Clang {version} (cached in .cppm/tools)…"
            );
            for (name, contents) in SOURCES {
                write_if_changed(&source.join(name), contents)?;
            }
            if path.exists() {
                fs::remove_file(&path).context("cannot rebuild the cached LLVM plugin")?;
            }
            let mut configure = Command::new("cmake");
            configure
                .args(["--fresh", "-S"])
                .arg(&source)
                .arg("-B")
                .arg(&build)
                .args([
                    "-G",
                    "Ninja",
                    "-DCMAKE_BUILD_TYPE=Release",
                    "--log-level=WARNING",
                ])
                .arg(format!("-DLLVM_DIR={}", cmakedir.display()))
                .arg(format!("-DCMAKE_CXX_COMPILER={}", cxx.display()))
                .arg(format!("-DCMAKE_C_COMPILER={}", cc.display()))
                .env_remove("CXXFLAGS")
                .env_remove("CFLAGS")
                .env_remove("LDFLAGS");
            process::run(&mut configure).context("cannot configure the bundled LLVM plugin; install the matching LLVM development headers/libraries or use --obf-basic")?;
            process::run(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&build)
                    .arg("--parallel")
                    .arg(
                        jobs.unwrap_or_else(|| {
                            std::thread::available_parallelism().map_or(2, |n| n.get().min(4))
                        })
                        .to_string(),
                    ),
            )
            .context("cannot compile the bundled LLVM plugin")?;
            let bytes = fs::read(&path).context("LLVM plugin binary was not generated")?;
            write_atomic(&seal, blake3::hash(&bytes).to_hex().as_bytes())?;
        }
        Ok(Self {
            path,
            cc,
            cxx,
            version,
            directory,
        })
    }

    pub fn install_options(
        &self,
        project: &Project,
        options: &InstallOptions,
    ) -> Result<InstallOptions> {
        let mut effective = options.clone();
        if !project.manifest.has_requirements() {
            return Ok(effective);
        }
        // Preserve user-selected profiles; reject a mislabeled GCC/MSVC graph
        // before compiling it with the automatically selected Clang.
        if options.profile != "default" {
            let mut show = platform::build_command("conan")?;
            store::configure(&mut show, options);
            let output = show
                .args([
                    "profile",
                    "show",
                    "--profile:host",
                    &options.profile,
                    "--profile:build",
                    &options.profile,
                    "--format=json",
                ])
                .output()?;
            ensure!(
                output.status.success(),
                "cannot inspect Conan profile: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            let major = self.version.split('.').next().unwrap_or_default();
            for context in ["host", "build"] {
                let settings = &report[context]["settings"];
                ensure!(
                    settings["compiler"] == "clang"
                        && settings["compiler.version"].as_str() == Some(major),
                    "Conan {context} profile '{}' must select clang {major} for bundled obfuscation; use a matching profile or the automatic default",
                    options.profile
                );
            }
            return Ok(effective);
        }
        let home = store::home(options)?;
        let _guard = store::lock(&home)?;
        let mut detect = platform::build_command("conan")?;
        store::configure(&mut detect, options);
        process::run(detect.args(["profile", "detect", "--exist-ok", "-vwarning"]))?;
        let compilers = serde_json::json!({"c": self.cc, "cpp": self.cxx});
        let profile = self.directory.join("clang-profile");
        let major = self.version.split('.').next().unwrap_or_default();
        let libcxx = if cfg!(target_os = "macos") {
            "libc++"
        } else {
            "libstdc++11"
        };
        write_if_changed(&profile, format!("include(default)\n\n[settings]\ncompiler=clang\ncompiler.version={major}\ncompiler.libcxx={libcxx}\n\n[conf]\ntools.build:compiler_executables={compilers}\n").as_bytes())?;
        effective.profile = profile
            .to_str()
            .context("LLVM profile path is not UTF-8")?
            .into();
        Ok(effective)
    }
}

pub(super) fn environment(command: &mut Command) {
    for variable in [
        "PEEPHOLE_PASSES",
        "VECTORIZERSTART_PASSES",
        "PIPELINESTART_PASSES",
        "PIPELINEEARLYSIMPLIFICATION_PASSES",
        "DEBUG_SEED",
        "SEED",
    ] {
        command.env_remove(format!("LLVM_OBF_{variable}"));
    }
    command
        .env("LLVM_OBF_SCALAROPTIMIZERLATE_PASSES", FUNCTION_PASSES)
        .env("LLVM_OBF_OPTIMIZERLASTEP_PASSES", MODULE_PASSES);
}

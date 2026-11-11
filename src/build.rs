use crate::{
    dependencies::{self, InstallOptions},
    editor, emscripten,
    fingerprint::Fingerprint,
    manifest::Target,
    platform, process,
    project::Project,
    protection, runtime,
    storage::write_if_changed,
    store,
};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

pub struct BuildOptions {
    pub release: bool,
    pub jobs: Option<usize>,
    pub obfuscate: bool,
    pub obf_plugin: Option<PathBuf>,
    pub obf_basic: bool,
    pub fresh: bool,
}

fn output_directory(project: &Project, options: &BuildOptions) -> PathBuf {
    if options.obfuscate {
        project.root.join(".cppm/build/obf")
    } else {
        directory(project, options.release)
    }
}

pub fn directory(project: &Project, release: bool) -> PathBuf {
    project
        .root
        .join(".cppm/build")
        .join(match (project.manifest.package.target, release) {
            (Target::Native, false) => "debug",
            (Target::Native, true) => "release",
            (Target::Wasm, false) => "wasm-debug",
            (Target::Wasm, true) => "wasm-release",
        })
}

pub fn build(project: &Project, options: &BuildOptions, install: &InstallOptions) -> Result<()> {
    let (project, _guard) = project.locked()?;
    build_locked(&project, options, install).map(|_| ())
}

fn build_locked(
    project: &Project,
    options: &BuildOptions,
    install: &InstallOptions,
) -> Result<InstallOptions> {
    let started = Instant::now();
    let protection = protection::Configuration::prepare(project, options)?;
    let install = match &protection {
        Some(protection) => protection.install_options(project, install)?,
        None => install.clone(),
    };
    prepare(project, options.release, &install, !options.fresh)?;
    let prepared = started.elapsed();
    configure_generated(
        project,
        options.release,
        install.verbose,
        protection.as_ref(),
        options.fresh,
        true,
    )?;
    let configured = started.elapsed();
    let directory = output_directory(project, options);
    let mut command = if project.manifest.package.target == Target::Wasm {
        emscripten::command("cmake")?
    } else {
        platform::build_command("cmake")?
    };
    command
        .arg("--build")
        .arg(&directory)
        .arg("--parallel")
        .current_dir(&project.root);
    if let Some(protection) = &protection {
        protection.environment(&mut command);
    }
    if let Some(jobs) = options.jobs {
        command.arg(jobs.to_string());
    }
    let result = process::run(&mut command);
    // Ninja may regenerate CMake after sources/includes change. Publish the
    // resulting editor database even when the subsequent compilation fails.
    let editor = editor::publish(&project.root, &directory);
    result?;
    editor?;
    if let Some(protection) = protection {
        protection.finish(project)?;
    }
    if install.verbose {
        eprintln!(
            "Build timings: dependencies {:.3}s, configure {:.3}s, compile {:.3}s, total {:.3}s.",
            prepared.as_secs_f64(),
            (configured - prepared).as_secs_f64(),
            (started.elapsed() - configured).as_secs_f64(),
            started.elapsed().as_secs_f64()
        );
    }
    Ok(install)
}

fn prepare(project: &Project, release: bool, install: &InstallOptions, reuse: bool) -> Result<()> {
    process::require_tool("cmake")?;
    process::require_tool("ninja")?;
    if project.manifest.package.target == Target::Wasm {
        emscripten::check(&project.root).with_context(|| {
            format!(
                "WebAssembly requires a working Emscripten SDK.\n{}",
                emscripten::SETUP_HINT
            )
        })?;
    }
    dependencies::prepare_build(project, release, install, reuse)?
        .commit(&project.root, install.frozen)
}

pub fn configure(
    project: &Project,
    release: bool,
    fresh: bool,
    install: &InstallOptions,
) -> Result<()> {
    let (project, _guard) = project.locked()?;
    prepare(&project, release, install, !fresh)?;
    configure_generated(&project, release, install.verbose, None, fresh, false)
}

pub fn refresh_editor(project: &Project, release: bool, install: &InstallOptions) {
    if let Err(error) = configure_generated(project, release, install.verbose, None, false, false) {
        eprintln!(
            "Warning: dependencies were installed, but editor configuration could not finish: {error:#}; run `cppm configure` to retry."
        );
    }
}

fn configure_generated(
    project: &Project,
    release: bool,
    verbose: bool,
    protection: Option<&protection::Configuration>,
    fresh: bool,
    reuse: bool,
) -> Result<()> {
    let directory = protection.map_or_else(|| directory(project, release), |p| p.directory.clone());
    let wasm = project.manifest.package.target == Target::Wasm;
    let mut configure = if wasm {
        emscripten::cmake()?
    } else {
        platform::build_command("cmake")?
    };
    configure
        .arg("-S")
        .arg(&project.root)
        .arg("-B")
        .arg(&directory)
        .args(["-G", "Ninja"])
        .arg(format!(
            "-DCMAKE_BUILD_TYPE={}",
            if release { "Release" } else { "Debug" }
        ))
        .arg(format!(
            "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY={}",
            directory.join("bin").display()
        ))
        .arg("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON")
        .arg(if verbose {
            "--log-level=STATUS"
        } else {
            "--log-level=WARNING"
        })
        .current_dir(&project.root);
    if project.manifest.has_requirements() {
        let toolchain = project
            .root
            .join(".cppm/conan")
            .join(if release { "release" } else { "debug" })
            .join("conan_toolchain.cmake");
        configure.arg(format!("-DCMAKE_TOOLCHAIN_FILE={}", toolchain.display()));
    }
    if let Some(protection) = protection {
        protection.configure(&mut configure, project)?;
    }
    let signature = configuration_key(project, release, &directory, &configure).ok();
    let marker = directory.join("cppm-configure.key");
    let unchanged = signature
        .as_deref()
        .is_some_and(|signature| fs::read_to_string(&marker).ok().as_deref() == Some(signature))
        && directory.join("CMakeCache.txt").is_file()
        && directory.join("build.ninja").is_file()
        && directory.join("compile_commands.json").is_file();
    if unchanged && !fresh && reuse && protection.is_none() {
        if verbose {
            eprintln!("Reusing CMake configuration; Ninja checks source and CMake changes.");
        }
        return Ok(());
    }
    if fresh || !unchanged {
        configure.arg("--fresh");
    }
    // An interrupted/failed configure must not leave a reusable success marker.
    match fs::remove_file(&marker) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let configured = process::run(&mut configure);
    if let Some(protection) = protection {
        // A project that disables the injected hook fails differently across
        // CMake versions; always name the cause instead of a bare exit status.
        protection.verify()?;
    }
    configured?;
    if let Ok(signature) = configuration_key(project, release, &directory, &configure) {
        write_if_changed(&marker, signature.as_bytes())?;
    }
    editor::publish(&project.root, &directory)
}

fn configuration_key(
    project: &Project,
    release: bool,
    directory: &Path,
    command: &Command,
) -> Result<String> {
    let mut value = Fingerprint::new("cmake-configuration-v1");
    value.os(project.root.as_os_str());
    value.os(command.get_program());
    for argument in command.get_args() {
        if argument != "--fresh" && !argument.to_string_lossy().starts_with("--log-level=") {
            value.os(argument);
        }
    }
    for (name, entry) in command.get_envs() {
        value.os(name);
        if let Some(entry) = entry {
            value.os(entry);
        }
    }
    value.environment()?;
    if project.manifest.package.target == Target::Wasm {
        emscripten::fingerprint(&mut value)?;
    }
    value.file(&project.root.join("CMakeLists.txt"))?;
    value.file(&project.root.join("web/shell.html"))?;
    value.file(&project.root.join(".cppm/dependencies.cmake"))?;
    value.file(&directory.join("cppm-protection.cmake"))?;
    if project.manifest.has_requirements() {
        value.directory(
            &project
                .root
                .join(".cppm/conan")
                .join(if release { "release" } else { "debug" }),
            "cppm-install.json",
        )?;
    }
    // Detect a compiler being replaced/upgraded at the same configured path.
    if let Ok(cache) = fs::read_to_string(directory.join("CMakeCache.txt")) {
        for line in cache.lines() {
            if (line.starts_with("CMAKE_CXX_COMPILER:") || line.starts_with("CMAKE_C_COMPILER:"))
                && let Some((_, path)) = line.split_once('=')
            {
                value.add(path.as_bytes());
                if let Ok(metadata) = fs::metadata(path) {
                    value.add(&metadata.len().to_le_bytes());
                    value.add(format!("{:?}", metadata.modified()).as_bytes());
                } else {
                    value.add(b"missing compiler");
                }
            }
        }
    }
    Ok(value.finish())
}

pub fn exec(
    project: &Project,
    options: &BuildOptions,
    install: &InstallOptions,
    args: &[String],
) -> Result<()> {
    let (project, guard) = project.locked()?;
    ensure!(
        project.manifest.package.target == Target::Native,
        "WebAssembly runs in a browser; use `cppm serve` or `cppm run dev`"
    );
    let install = build_locked(&project, options, install)?;
    let binary = output_directory(&project, options)
        .join("bin")
        .join(format!(
            "{}{}",
            project.manifest.package.name,
            std::env::consts::EXE_SUFFIX
        ));
    ensure!(
        binary.is_file(),
        "executable {} is missing; the CMake target must match package.name",
        binary.display()
    );
    let mut command = platform::build_command(binary)?;
    runtime::configure(&mut command, &project, options.release, install.verbose)?;
    // Keep the exact dependency revisions alive while an application is using
    // them, even if another command upgrades/removes its project's dependencies.
    let _lease = store::lease(&project, options.release, &install)?;
    // Capture the matching runtime environment before releasing the project lock.
    // Let another command build while the user's application is running.
    drop(guard);
    process::run(command.args(args).current_dir(&project.root))
}

pub fn test(
    project: &Project,
    options: &BuildOptions,
    install: &InstallOptions,
    args: &[String],
) -> Result<()> {
    let (project, _guard) = project.locked()?;
    let _install = build_locked(&project, options, install)?;
    let mut command = if project.manifest.package.target == Target::Wasm {
        emscripten::command("ctest")?
    } else {
        platform::build_command("ctest")?
    };
    runtime::configure(&mut command, &project, options.release, install.verbose)?;
    process::run(
        command
            .arg("--test-dir")
            .arg(output_directory(&project, options))
            .args(["--output-on-failure", "--no-tests=error"])
            .args(args)
            .current_dir(&project.root),
    )
}

pub fn serve(
    project: &Project,
    options: &BuildOptions,
    install: &InstallOptions,
    port: u16,
) -> Result<()> {
    let (project, guard) = project.locked()?;
    ensure!(
        project.manifest.package.target == Target::Wasm,
        "serve is for WebAssembly projects; use `cppm run dev` for a native application"
    );
    let _install = build_locked(&project, options, install)?;
    let output = output_directory(&project, options).join("bin");
    let html = output.join(format!("{}.html", project.manifest.package.name));
    ensure!(html.is_file(), "{} was not generated", html.display());
    println!(
        "Open http://127.0.0.1:{port}/{}.html",
        project.manifest.package.name
    );
    drop(guard);
    let python = if cfg!(windows) { "python" } else { "python3" };
    process::run(
        emscripten::command(python)?
            .args([
                "-m",
                "http.server",
                &port.to_string(),
                "--bind",
                "127.0.0.1",
                "--directory",
            ])
            .arg(output)
            .current_dir(&project.root),
    )
}

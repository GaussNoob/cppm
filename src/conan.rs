use crate::{platform, process, project::Project, storage::write_atomic, store};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

mod cache;
pub mod graph;
pub mod versions;

pub use graph::Graph;

#[derive(Clone, Debug)]
pub struct InstallOptions {
    pub profile: String,
    pub store_dir: Option<PathBuf>,
    pub offline: bool,
    pub frozen: bool,
    pub verbose: bool,
}

pub struct Installed {
    pub lock: Value,
    pub graph: Value,
    pub registration: store::Pending,
}

fn command(options: &InstallOptions) -> Result<Command> {
    let mut command = platform::build_command("conan")?;
    store::configure(&mut command, options);
    Ok(command)
}

/// The Conan version, checked once per process.
fn version(options: &InstallOptions) -> Result<&'static str> {
    static VERSION: OnceLock<String> = OnceLock::new();
    if let Some(version) = VERSION.get() {
        return Ok(version);
    }
    let output = command(options)?.arg("--version").output()
        .context("dependencies require Conan 2; install it with `pipx install conan` or `uv tool install conan`")?;
    let version = String::from_utf8_lossy(&output.stdout).into_owned();
    ensure!(
        output.status.success() && version.starts_with("Conan version 2."),
        "cppm requires Conan 2"
    );
    Ok(VERSION.get_or_init(|| version))
}

/// Locks the shared store and makes sure the selected profile can be read.
fn initialize(project: &Project, options: &InstallOptions) -> Result<(PathBuf, File)> {
    let home = store::home(options)?;
    // Conan's package cache is shared, but concurrent writes are not supported.
    // Keep this guard through resolution/installation, including profile detection.
    let guard = store::lock(&home)?;
    // `profile detect --exist-ok` is a no-op for an existing profile; skip
    // starting Conan for it on every command after the first.
    if options.profile == "default" && !home.join("profiles/default").is_file() {
        let mut detect = command(options)?;
        detect
            .args(["profile", "detect", "--exist-ok"])
            .current_dir(&project.root);
        if !options.verbose {
            detect.arg("-vwarning");
        }
        process::run(&mut detect)?;
    }
    Ok((home, guard))
}

fn configure(command: &mut Command, project: &Project, release: bool, options: &InstallOptions) {
    configure_profiles(command, project, release, options);
    if options.offline {
        command.arg("--no-remote");
    }
}

fn configure_profiles(
    command: &mut Command,
    project: &Project,
    release: bool,
    options: &InstallOptions,
) {
    command
        .arg("--profile:host")
        .arg(&options.profile)
        .arg("--profile:build")
        .arg(&options.profile)
        .arg("--settings:host")
        .arg(format!(
            "build_type={}",
            if release { "Release" } else { "Debug" }
        ))
        .arg("--settings:host")
        .arg(format!(
            "compiler.cppstd={}",
            project.manifest.package.cpp_standard
        ))
        .current_dir(&project.root);
    // Qt's Linux build requires GNU integer extensions (notably __int128).
    // Keep this scoped to Qt: the application still uses its declared standard.
    #[cfg(target_os = "linux")]
    command.arg("--settings:host").arg(format!(
        "qt/*:compiler.cppstd=gnu{}",
        project.manifest.package.cpp_standard
    ));
    // Conan's batch activation can interpret '&' in an inherited Windows PATH.
    // Its PowerShell launchers keep those tool directories as literal values.
    #[cfg(windows)]
    command
        .arg("--conf:all")
        .arg("tools.env.virtualenv:powershell=powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass");
    if !options.verbose {
        command.arg("-vwarning");
    }
}

pub fn resolve_version(
    project: &Project,
    name: &str,
    release: bool,
    options: &InstallOptions,
) -> Result<String> {
    version(options)?;
    let (_home, _guard) = initialize(project, options)?;
    let staging = tempfile::tempdir_in(project.root.join(".cppm"))?;
    let output = staging.path().join("graph.json");
    let mut command = command(options)?;
    command
        .args(["graph", "info"])
        .arg(format!("--requires={name}/[>=0]"))
        .arg("--format=json")
        .arg("--out-file")
        .arg(&output);
    // Without --update Conan prefers cached versions and only queries remotes
    // when necessary, so a new project can reuse an already installed recipe.
    configure(&mut command, project, release, options);
    process::run(&mut command).with_context(|| {
        format!("cannot resolve '{name}' from Conan; check its recipe name or use name@version")
    })?;
    let graph: Value = serde_json::from_slice(&fs::read(output)?)?;
    let version = Graph::new(&graph)
        .host(name)
        .and_then(|node| node.version())
        .context("Conan did not return the resolved package version")?;
    crate::manifest::validate_version(version)?;
    Ok(version.to_owned())
}

fn output_directory(root: &Path, release: bool) -> PathBuf {
    root.join(".cppm/conan")
        .join(if release { "release" } else { "debug" })
}

/// The consumer recipe that requires the manifest's packages.
fn recipe(project: &Project) -> Result<String> {
    let requires: Vec<_> = project
        .manifest
        .requirements()
        .map(|(name, dependency)| format!("{name}/{}", dependency.version()))
        .collect();
    let mut defaults = Vec::new();
    if project.manifest.requires("opencv") {
        // Images work without building a complete audio/video codec stack.
        // A Conan profile can opt in with opencv/*:with_ffmpeg=True.
        defaults.push("\"opencv/*:with_ffmpeg\": False");
        // Native Linux webcams use V4L2 independently of FFmpeg. The Conan
        // recipe disables it by default, leaving ordinary USB cameras unusable.
        // Keep this a default so a profile can still explicitly opt out.
        #[cfg(target_os = "linux")]
        defaults.push("\"opencv/*:with_v4l\": True");
    }
    if project.manifest.requires("qt") {
        // Qt's dynamic libraries keep the framework's bundled implementation
        // details inside Qt instead of exposing missing static recipe metadata.
        defaults.extend(["\"qt/*:shared\": True", "\"qt/*:with_pq\": False"]);
    }
    Ok(include_str!("conanfile.py")
        .replace("@CPPM_REQUIRES@", &serde_json::to_string(&requires)?)
        .replace("@CPPM_OPTIONS@", &format!("{{{}}}", defaults.join(", "))))
}

/// The profiles as Conan resolves them, including `include()` and templates.
fn resolved_profiles(
    project: &Project,
    release: bool,
    options: &InstallOptions,
) -> Result<Option<Vec<u8>>> {
    let mut profile = command(options)?;
    profile.args(["profile", "show", "--format=json"]);
    configure_profiles(&mut profile, project, release, options);
    let profiles = profile.output()?;
    if !profiles.status.success() {
        return Ok(None);
    }
    let profiles: Value = serde_json::from_slice(&profiles.stdout)?;
    Ok(Some(serde_json::to_vec(&profiles)?))
}

/// The last graph resolved for this project, without running Conan.
pub fn resolved_graph(root: &Path, release: bool) -> Option<(Value, Value)> {
    cache::graph(&output_directory(root, release))
}

pub fn install(
    project: &Project,
    release: bool,
    options: &InstallOptions,
    previous: Option<&Value>,
    reuse: bool,
) -> Result<Installed> {
    let mode = if release { "release" } else { "debug" };
    let output = output_directory(&project.root, release);
    let contents = recipe(project)?;
    let request = cache::Request {
        root: &project.root,
        profile: &options.profile,
        store_dir: options.store_dir.as_deref(),
        release,
        cpp_standard: project.manifest.package.cpp_standard,
        recipe: &contents,
    };
    let reused = |home: PathBuf, guard: File, cached: cache::Cached| -> Result<Installed> {
        if options.verbose {
            eprintln!(
                "Reusing installed Conan dependencies ({mode}; profile {}).",
                options.profile
            );
        }
        Ok(Installed {
            registration: store::Pending::new(home, guard, &cached.graph, options, release)?,
            lock: cached.lock,
            graph: cached.graph,
        })
    };
    // An unchanged build is the common case. Prove nothing Conan reads has
    // changed since the last validated installation and skip starting it.
    if reuse
        && let Some((cached, guard)) =
            cache::read_fast(&output, &request, previous, |home| store::lock(home).ok())
    {
        let home = cached.home.clone();
        store::remember_home(options, &home);
        return reused(home, guard, cached);
    }
    fs::create_dir_all(&output)?;
    let (home, guard) = initialize(project, options)?;
    // Both queries only read; run them together to pay for one Conan start.
    let (version, profiles) = std::thread::scope(|scope| {
        let profiles = scope.spawn(|| resolved_profiles(project, release, options));
        (version(options), profiles.join())
    });
    let version = version?;
    let key = profiles
        .ok()
        .and_then(Result::ok)
        .flatten()
        .and_then(|profiles| cache::key(&home, version, &contents, &profiles).ok());
    if reuse
        && let Some(cached) = key
            .as_deref()
            .and_then(|key| cache::read(&output, key, previous))
    {
        // Conan confirmed the installation; let the next build skip the query.
        let fast = cache::fast_key(&request, &home).ok().flatten();
        if fast.is_some()
            && let Err(error) = cache::save(
                &output,
                cache::Snapshot {
                    key: key.clone().unwrap_or_default(),
                    fast,
                    home: &home,
                    lock: &cached.lock,
                    graph: &cached.graph,
                },
            )
            && options.verbose
        {
            eprintln!("Could not cache dependency preparation: {error:#}");
        }
        return reused(home, guard, cached);
    }
    let staging = tempfile::tempdir_in(project.root.join(".cppm"))?;
    let recipe = staging.path().join("conanfile.py");
    write_atomic(&recipe, contents.as_bytes())?;
    let output_lock = staging.path().join("resolved.lock");
    let graph_file = staging.path().join("graph.json");
    let mut command = command(options)?;
    command
        .arg("install")
        .arg(&recipe)
        .arg("--output-folder")
        .arg(&output)
        .arg("--build=missing")
        .arg("--conf")
        .arg("tools.cmake.cmaketoolchain:generator=Ninja")
        .arg("--lockfile-out")
        .arg(&output_lock)
        .arg("--format=json")
        .arg("--out-file")
        .arg(&graph_file);
    configure(&mut command, project, release, options);
    if let Some(previous) = previous.filter(|graph| !graph.is_null()) {
        let input_lock = staging.path().join("input.lock");
        write_atomic(&input_lock, &serde_json::to_vec_pretty(previous)?)?;
        command.arg("--lockfile").arg(input_lock);
        if !options.frozen {
            command.arg("--lockfile-partial").arg("--lockfile-clean");
        }
    } else {
        command.arg("--lockfile=");
    }
    process::run(&mut command)
        .context("dependency installation failed; cppm.toml and cppm.lock were not changed")?;
    let lock = serde_json::from_slice(&fs::read(output_lock)?)
        .context("invalid lockfile returned by Conan")?;
    let graph: Value = serde_json::from_slice(&fs::read(&graph_file)?)
        .context("invalid package metadata returned by Conan")?;
    // A cleanup failure must not turn a successful installation into a failed
    // add. The user can retry cleanup later with `cppm store prune`.
    if let Err(error) = store::clean_new_packages(&graph, &graph_file, options) {
        eprintln!(
            "Warning: could not clean dependency build files: {error:#}; run `cppm store prune` to retry."
        );
    }
    if let Some(key) = key
        && let Err(error) = cache::save(
            &output,
            cache::Snapshot {
                key,
                // Computed after the installation: Conan creates its default
                // configuration files the first time it uses a home.
                fast: cache::fast_key(&request, &home).ok().flatten(),
                home: &home,
                lock: &lock,
                graph: &graph,
            },
        )
        && options.verbose
    {
        eprintln!("Could not cache dependency preparation: {error:#}");
    }
    let registration = store::Pending::new(home, guard, &graph, options, release)?;
    Ok(Installed {
        lock,
        graph,
        registration,
    })
}

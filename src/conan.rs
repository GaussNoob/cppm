use crate::{platform, process, project::Project, storage::write_atomic, store};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    fs::{self, File},
    path::PathBuf,
    process::Command,
};

mod cache;

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

fn initialize(project: &Project, options: &InstallOptions) -> Result<(PathBuf, File, String)> {
    let version = command(options)?.arg("--version").output()
        .context("dependencies require Conan 2; install it with `pipx install conan` or `uv tool install conan`")?;
    ensure!(
        version.status.success()
            && String::from_utf8_lossy(&version.stdout).starts_with("Conan version 2."),
        "cppm requires Conan 2"
    );
    // Conan's package cache is shared, but concurrent writes are not supported.
    // Keep this guard through resolution/installation, including profile detection.
    let home = store::home(options)?;
    let guard = store::lock(&home)?;
    if options.profile == "default" {
        let mut detect = command(options)?;
        detect
            .args(["profile", "detect", "--exist-ok"])
            .current_dir(&project.root);
        if !options.verbose {
            detect.arg("-vwarning");
        }
        process::run(&mut detect)?;
    }
    Ok((
        home,
        guard,
        String::from_utf8_lossy(&version.stdout).into_owned(),
    ))
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
    let (_home, _guard, _version) = initialize(project, options)?;
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
    let version = graph["graph"]["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes
                .values()
                .find(|node| node["name"] == name && node["context"] == "host")
        })
        .and_then(|node| node["version"].as_str())
        .context("Conan did not return the resolved package version")?;
    crate::manifest::validate_version(version)?;
    Ok(version.to_owned())
}

pub fn install(
    project: &Project,
    release: bool,
    options: &InstallOptions,
    previous: Option<&Value>,
    reuse: bool,
) -> Result<Installed> {
    let (home, guard, version) = initialize(project, options)?;
    let mode = if release { "release" } else { "debug" };
    let output = project.root.join(".cppm/conan").join(mode);
    fs::create_dir_all(&output)?;
    let staging = tempfile::tempdir_in(project.root.join(".cppm"))?;
    let recipe = staging.path().join("conanfile.py");
    let requires: Vec<_> = project
        .manifest
        .dependencies
        .iter()
        .map(|(name, dependency)| format!("{name}/{}", dependency.version()))
        .collect();
    let mut defaults = Vec::new();
    if project.manifest.dependencies.contains_key("opencv") {
        // Images work without building a complete audio/video codec stack.
        // A Conan profile can opt in with opencv/*:with_ffmpeg=True.
        defaults.push("\"opencv/*:with_ffmpeg\": False");
        // Native Linux webcams use V4L2 independently of FFmpeg. The Conan
        // recipe disables it by default, leaving ordinary USB cameras unusable.
        // Keep this a default so a profile can still explicitly opt out.
        #[cfg(target_os = "linux")]
        defaults.push("\"opencv/*:with_v4l\": True");
    }
    if project.manifest.dependencies.contains_key("qt") {
        // Qt's dynamic libraries keep the framework's bundled implementation
        // details inside Qt instead of exposing missing static recipe metadata.
        defaults.extend(["\"qt/*:shared\": True", "\"qt/*:with_pq\": False"]);
    }
    let contents = include_str!("conanfile.py")
        .replace("@CPPM_REQUIRES@", &serde_json::to_string(&requires)?)
        .replace("@CPPM_OPTIONS@", &format!("{{{}}}", defaults.join(", ")));
    let key = (|| -> Result<Option<String>> {
        let mut profile = command(options)?;
        profile.args(["profile", "show", "--format=json"]);
        configure_profiles(&mut profile, project, release, options);
        let profiles = profile.output()?;
        if !profiles.status.success() {
            return Ok(None);
        }
        let profiles: Value = serde_json::from_slice(&profiles.stdout)?;
        Ok(Some(cache::key(
            &home,
            &version,
            &contents,
            &serde_json::to_vec(&profiles)?,
        )?))
    })()
    .ok()
    .flatten();
    if reuse
        && let Some(cached) = key
            .as_deref()
            .and_then(|key| cache::read(&output, key, previous))
    {
        if options.verbose {
            eprintln!(
                "Reusing installed Conan dependencies ({mode}; profile {}).",
                options.profile
            );
        }
        return Ok(Installed {
            registration: store::Pending::new(home, guard, &cached.graph, options, release)?,
            lock: cached.lock,
            graph: cached.graph,
        });
    }
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
        && let Err(error) = cache::save(&output, key, &lock, &graph)
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

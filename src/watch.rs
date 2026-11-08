use crate::{conan::InstallOptions, process::CommandFailure, project::Project};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, PartialEq, Eq)]
struct FileState {
    bytes: u64,
    modified: SystemTime,
}

type Snapshot = BTreeMap<PathBuf, FileState>;

pub struct Options {
    pub script: String,
    pub interval: Duration,
    pub debounce: Duration,
    pub paths: Vec<PathBuf>,
    pub args: Vec<String>,
}

pub fn run(project: &Project, install: &InstallOptions, options: &Options) -> Result<()> {
    ensure!(
        std::env::var_os("CPPM_WATCH_ACTIVE").is_none(),
        "nested watch commands are not supported; watch a build, exec or serve script"
    );
    ensure!(
        project.manifest.scripts.contains_key(&options.script),
        "unknown script '{}'; use `cppm run` to list scripts",
        options.script
    );
    let roots = if options.paths.is_empty() {
        vec![project.root.clone()]
    } else {
        options
            .paths
            .iter()
            .map(|path| project.root.join(path))
            .collect()
    };
    for path in &roots {
        ensure!(
            path.exists(),
            "watch path {} does not exist",
            path.display()
        );
    }
    let all_files = !options.paths.is_empty();
    let mut previous = snapshot(&roots, all_files)?;
    let stopping = Arc::new(AtomicBool::new(false));
    let interrupted = stopping.clone();
    ctrlc::set_handler(move || interrupted.store(true, Ordering::SeqCst))
        .context("cannot install the watch shutdown handler")?;
    println!(
        "Watching {} · script '{}' · Ctrl+C stops",
        project.root.display(),
        options.script
    );
    let mut running = Some(start(project, install, options)?);
    let mut changed = BTreeSet::new();
    let mut last_change = Instant::now();
    while !stopping.load(Ordering::SeqCst) {
        if let Some(child) = &mut running
            && let Some(status) = child.0.try_wait()?
        {
            eprintln!(
                "Script '{}' exited with status {}; waiting for changes.",
                options.script,
                status.code().unwrap_or(130)
            );
            running.take();
        }
        let current = snapshot(&roots, all_files)?;
        if previous != current {
            changed.extend(
                previous
                    .keys()
                    .chain(current.keys())
                    .filter(|path| previous.get(*path) != current.get(*path))
                    .cloned(),
            );
            previous = current;
            last_change = Instant::now();
        }
        if !changed.is_empty() && last_change.elapsed() >= options.debounce {
            let first = changed.first().expect("nonempty change set");
            println!(
                "Restarting '{}': {} ({} changed files)",
                options.script,
                first.strip_prefix(&project.root).unwrap_or(first).display(),
                changed.len()
            );
            running.take();
            if stopping.load(Ordering::SeqCst) {
                break;
            }
            running = Some(start(project, install, options)?);
            changed.clear();
        }
        thread::sleep(options.interval);
    }
    drop(running);
    Err(CommandFailure(130).into())
}

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        if let Err(error) = stop(&mut self.0) {
            eprintln!("Could not stop the watched process tree: {error:#}");
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn start(project: &Project, install: &InstallOptions, options: &Options) -> Result<Running> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .current_dir(&project.root)
        .env("CPPM_WATCH_ACTIVE", "1")
        .arg("--profile")
        .arg(&install.profile);
    if let Some(store) = &install.store_dir {
        command.arg("--store-dir").arg(store);
    }
    if install.offline {
        command.arg("--offline");
    }
    if install.frozen {
        command.arg("--frozen-lockfile");
    }
    if install.verbose {
        command.arg("--verbose");
    }
    command.args(["run", &options.script]);
    if !options.args.is_empty() {
        command.arg("--").args(&options.args);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP
    }
    Ok(Running(
        command.spawn().context("cannot start the watched script")?,
    ))
}

#[cfg(unix)]
fn stop(child: &mut Child) -> Result<()> {
    let group = -i32::try_from(child.id()).context("invalid watched process group")?;
    // This group is created for our child before it executes. Signals cannot
    // target the user's shell or another concurrently running cppm command.
    signal(group, libc::SIGTERM)?;
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        child.try_wait()?;
        // SAFETY: kill with signal 0 only checks whether this process group exists.
        if unsafe { libc::kill(group, 0) } == -1 {
            break;
        }
        if Instant::now() >= deadline {
            signal(group, libc::SIGKILL)?;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    child.wait()?;
    Ok(())
}

#[cfg(unix)]
fn signal(group: i32, signal: i32) -> Result<()> {
    // SAFETY: the negative group ID comes from a child we started in its own group.
    if unsafe { libc::kill(group, signal) } == -1 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error.into());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn stop(child: &mut Child) -> Result<()> {
    if child.try_wait()?.is_none() {
        let status = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        ensure!(
            status.success() || child.try_wait()?.is_some(),
            "taskkill could not stop the watched process tree"
        );
    }
    child.wait()?;
    Ok(())
}

fn snapshot(roots: &[PathBuf], all_files: bool) -> Result<Snapshot> {
    let mut files = BTreeMap::new();
    for root in roots {
        visit(root, &mut files, all_files)?;
    }
    Ok(files)
}

fn visit(path: &Path, files: &mut Snapshot, all_files: bool) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot watch {}", path.display()));
        }
    };
    if metadata.is_dir() {
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.')
                || matches!(
                    name.as_ref(),
                    "build"
                        | "target"
                        | "node_modules"
                        | "dist"
                        | "out"
                        | "bin"
                        | "venv"
                        | "__pycache__"
                )
            {
                continue;
            }
            visit(&entry.path(), files, all_files)?;
        }
    } else if all_files || relevant(path) {
        // Follow file symlinks, while directory symlinks are deliberately not
        // traversed (cycles and entire external dependency trees).
        let metadata = if metadata.file_type().is_symlink() {
            match fs::metadata(path) {
                Ok(metadata) if metadata.is_file() => metadata,
                _ => return Ok(()),
            }
        } else {
            metadata
        };
        files.insert(
            path.to_path_buf(),
            FileState {
                bytes: metadata.len(),
                modified: metadata.modified()?,
            },
        );
    }
    Ok(())
}

fn relevant(path: &Path) -> bool {
    if path
        .file_name()
        .is_some_and(|name| name == "compile_commands.json")
    {
        return false;
    }
    if path
        .file_name()
        .is_some_and(|name| name == "CMakeLists.txt")
    {
        return true;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension,
                "c" | "cc"
                    | "cpp"
                    | "cxx"
                    | "h"
                    | "hh"
                    | "hpp"
                    | "hxx"
                    | "inl"
                    | "ipp"
                    | "inc"
                    | "cmake"
                    | "toml"
                    | "json"
                    | "yaml"
                    | "yml"
                    | "html"
                    | "css"
                    | "scss"
                    | "js"
                    | "mjs"
                    | "ts"
                    | "tsx"
                    | "jsx"
                    | "glsl"
                    | "vert"
                    | "frag"
                    | "wgsl"
                    | "hlsl"
                    | "svg"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "webp"
                    | "py"
            )
        })
}

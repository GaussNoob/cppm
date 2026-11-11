use crate::{
    conan::{Graph, InstallOptions},
    platform, process,
};
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, File, TryLockError},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

mod content;
mod registry;
pub(crate) use registry::{Pending, clear_project, lease};

#[derive(Subcommand)]
pub enum Action {
    /// Print the shared store location; works outside a project.
    Path,
    /// Show cached recipes, binary configurations and home directory size.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Share identical package files by content, including across versions.
    Dedupe,
    /// List projects registered with this shared store.
    Projects,
    /// Remove unreferenced managed packages and temporary files.
    Prune {
        /// Also remove downloaded sources; rebuilding may need the network again.
        #[arg(long)]
        sources: bool,
        /// Preview unused packages without deleting anything.
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn configure(command: &mut Command, options: &InstallOptions) {
    if let Some(directory) = &options.store_dir {
        command.env("CONAN_HOME", directory);
    }
}

fn command(options: &InstallOptions) -> Command {
    let mut command = platform::command("conan");
    configure(&mut command, options);
    command
}

fn capture(command: &mut Command) -> Result<Vec<u8>> {
    let output = command.output().context(
        "cannot start Conan 2; install it with `pipx install conan` or `uv tool install conan`",
    )?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(process::CommandFailure(output.status.code().unwrap_or(130)).into());
    }
    Ok(output.stdout)
}

// One command asks for the store several times (installation, registration,
// execution lease). Its location cannot change while the process runs, and
// every question used to cost a Conan start.
static HOMES: Mutex<BTreeMap<Option<PathBuf>, PathBuf>> = Mutex::new(BTreeMap::new());

pub fn home(options: &InstallOptions) -> Result<PathBuf> {
    if let Some(home) = HOMES
        .lock()
        .ok()
        .and_then(|homes| homes.get(&options.store_dir).cloned())
    {
        return Ok(home);
    }
    // Ask Conan instead of guessing: this respects CONAN_HOME and Conan's
    // platform-specific configuration, and keeps existing packages available.
    let output = String::from_utf8(capture(command(options).args(["config", "home"]))?)?;
    let home = PathBuf::from(output.trim());
    ensure!(
        home.is_absolute(),
        "Conan returned an invalid store location"
    );
    fs::create_dir_all(&home)?;
    let home = home.canonicalize().context("cannot locate shared store")?;
    remember_home(options, &home);
    Ok(home)
}

/// Records a store location already confirmed for these options.
pub fn remember_home(options: &InstallOptions, home: &Path) {
    if let Ok(mut homes) = HOMES.lock() {
        homes.insert(options.store_dir.clone(), home.to_path_buf());
    }
}

pub fn lock(home: &Path) -> Result<File> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("cppm-store.lock"))
        .context("cannot open shared store lock")?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            eprintln!("Waiting for another cppm command to finish using the shared store...");
            file.lock().context("cannot lock shared store")?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(file)
}

pub fn clean_new_packages(
    graph: &Value,
    graph_file: &Path,
    options: &InstallOptions,
) -> Result<()> {
    let has_new_packages = Graph::new(graph)
        .nodes()
        .any(|node| matches!(node.binary(), Some("Build" | "Download")));
    if !has_new_packages {
        return Ok(());
    }
    // Let Conan select exact revisions and binary IDs from this installation.
    // Never sweep unrelated packages as a side effect of adding a dependency.
    let list = graph_file.with_file_name("cleanup.json");
    capture(
        command(options)
            .arg("list")
            .arg("--graph")
            .arg(graph_file)
            .args([
                "--graph-binaries=build",
                "--graph-binaries=download",
                "--format=json",
                "--out-file",
            ])
            .arg(&list),
    )?;
    capture(
        command(options)
            .args(["cache", "clean", "--list"])
            .arg(list)
            .args(["--build", "--download"]),
    )?;
    Ok(())
}

#[derive(Serialize)]
struct Status {
    home: PathBuf,
    home_bytes: u64,
    recipes: usize,
    binaries: usize,
    projects: usize,
    managed_binaries: usize,
    #[serde(flatten)]
    content: content::ContentStats,
}

fn directory_bytes(root: &Path) -> Result<u64> {
    let mut bytes = 0;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        // Do not traverse symlinks/junctions into another directory or count
        // their targets as duplicate data stored inside this home.
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() {
            bytes += directory_bytes(&entry.path())?;
        } else if metadata.is_file() {
            bytes += metadata.len();
        }
    }
    Ok(bytes)
}

pub fn run(action: Action, options: &InstallOptions) -> Result<()> {
    let home = home(options)?;
    if matches!(action, Action::Path) {
        println!("{}", home.display());
        return Ok(());
    }
    let _guard = lock(&home)?;
    match action {
        Action::Path => unreachable!(),
        Action::Status { json } => {
            let index = registry::read(&home)?;
            let listing: Value = serde_json::from_slice(&capture(command(options).args([
                "list",
                "*:*",
                "--format=json",
            ]))?)
            .context("invalid store metadata returned by Conan")?;
            let recipes = listing["Local Cache"]
                .as_object()
                .context("Conan did not return the local cache")?;
            let binaries = recipes
                .values()
                .filter_map(|recipe| recipe["revisions"].as_object())
                .flat_map(|revisions| revisions.values())
                .filter_map(|revision| revision["packages"].as_object())
                .map(|packages| packages.len())
                .sum();
            let status = Status {
                home_bytes: directory_bytes(&home)?,
                recipes: recipes.len(),
                binaries,
                projects: index.projects.len(),
                managed_binaries: index.owned.binaries.len(),
                content: content::stats(&home, index.owned.binaries.values())?,
                home,
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("Store: {}", status.home.display());
                println!("Recipes: {}", status.recipes);
                println!("Binary configurations: {}", status.binaries);
                println!("Home size: {} bytes", status.home_bytes);
                println!("Registered projects: {}", status.projects);
                println!(
                    "Managed payload: {} logical bytes, {} unique bytes",
                    status.content.logical_bytes, status.content.unique_bytes
                );
                println!(
                    "Saved by file sharing: {} bytes",
                    status.content.deduplicated_bytes
                );
            }
            Ok(())
        }
        Action::Projects => {
            for project in registry::read(&home)?.projects.values() {
                println!(
                    "{}{}",
                    project.root.display(),
                    if registry::project_exists(&project.root)? {
                        ""
                    } else {
                        " (missing)"
                    }
                );
            }
            Ok(())
        }
        Action::Dedupe => {
            let index = registry::read(&home)?;
            let report = content::dedupe(&home, index.owned.binaries.values())?;
            println!(
                "Shared {} duplicate files; saved {} bytes ({} unsupported files left intact).",
                report.linked_files, report.saved_bytes, report.skipped_files
            );
            Ok(())
        }
        Action::Prune { sources, dry_run } => {
            let report = registry::prune(&home, options, dry_run)?;
            if dry_run {
                println!(
                    "Would remove {} unused binaries and {} recipe revisions; {} missing projects.",
                    report.removed_binaries, report.removed_recipes, report.forgotten_projects
                );
                return Ok(());
            }
            let mut command = command(options);
            command.args(["cache", "clean", "*", "--build", "--download", "--temp"]);
            if sources {
                command.arg("--source");
            }
            process::run(&mut command)?;
            println!(
                "Removed {} unused binaries, {} recipe revisions and {} unreferenced shared files; active projects preserved.",
                report.removed_binaries, report.removed_recipes, report.removed_shared_files
            );
            Ok(())
        }
    }
}

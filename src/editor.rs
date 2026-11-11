use crate::{
    manifest::Target,
    project::Project,
    storage::{write_atomic, write_if_changed},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path};

// clangd discovers this file beside cppm.toml, even when the editor opened a
// parent directory. Keep a snapshot so a user's own database is never replaced.
pub fn publish(root: &Path, build: &Path) -> Result<()> {
    let contents = fs::read(build.join("compile_commands.json"))
        .context("CMake did not export compile_commands.json")?;
    let snapshot = root.join(".cppm/compile_commands.json");
    let previous = read(&snapshot)?;
    let destination = root.join("compile_commands.json");
    let current = read(&destination)?;
    let is_symlink = fs::symlink_metadata(&destination)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);
    if !is_symlink
        && (current.is_none() || current == previous || current.as_deref() == Some(&contents))
    {
        write_if_changed(&destination, &contents)?;
        let ignore = root.join(".gitignore");
        let mut text = String::from_utf8(read(&ignore)?.unwrap_or_default())?;
        if !text.lines().any(|line| line == "/compile_commands.json") {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str("/compile_commands.json\n");
            write_if_changed(&ignore, text.as_bytes())?;
        }
    } else {
        eprintln!(
            "Existing compile_commands.json preserved; the current cppm database is in .cppm/compile_commands.json."
        );
    }
    write_if_changed(&snapshot, &contents)?;
    register_workspace(root);
    Ok(())
}

fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn unpublish(root: &Path, removed: &Path) -> Result<()> {
    let snapshot = root.join(".cppm/compile_commands.json");
    let Some(previous) = read(&snapshot)? else {
        return Ok(());
    };
    let Ok(database) = serde_json::from_slice::<Value>(&previous) else {
        return Ok(());
    };
    #[cfg(windows)]
    let removed_path = windows_comparison_path(
        &root.canonicalize()?.join(
            removed
                .strip_prefix(root)
                .context("build directory is outside the project")?,
        ),
    );
    #[cfg(windows)]
    let removed = removed_path.as_path();
    let affected = database.as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            entry["directory"].as_str().is_some_and(|directory| {
                #[cfg(windows)]
                let directory = windows_comparison_path(Path::new(directory));
                #[cfg(not(windows))]
                let directory = Path::new(directory);
                directory.starts_with(removed)
            })
        })
    });
    if !affected {
        return Ok(());
    }
    let destination = root.join("compile_commands.json");
    let symlink =
        fs::symlink_metadata(&destination).is_ok_and(|metadata| metadata.file_type().is_symlink());
    if !symlink && read(&destination)?.as_ref() == Some(&previous) {
        fs::remove_file(destination)?;
    }
    fs::remove_file(snapshot)?;
    Ok(())
}

#[cfg(windows)]
fn windows_comparison_path(path: &Path) -> std::path::PathBuf {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = if let Some(network) = path.strip_prefix("//?/UNC/") {
        format!("//{network}")
    } else {
        path.strip_prefix("//?/").unwrap_or(&path).to_owned()
    };
    // CMake uses normal Windows paths, while Rust canonicalization can return
    // extended-length paths. Windows comparisons also ignore case.
    path.to_lowercase().into()
}

pub fn vscode_tasks(project_path: &str, target: Target) -> Value {
    let application = if target == Target::Wasm {
        "serve"
    } else {
        "exec"
    };
    let task = |label: &str, action: &str| {
        json!({
            "label": label,
            "type": "process",
            "command": "cppm",
            "args": ["-C", project_path, action],
            "problemMatcher": ["$gcc"],
            "presentation": {"reveal": "always", "panel": "dedicated"}
        })
    };
    let mut build = task("cppm: build", "build");
    build["group"] = json!({"kind": "build", "isDefault": true});
    let mut test = task("cppm: test", "test");
    test["group"] = json!({"kind": "test", "isDefault": true});
    json!({"version": "2.0.0", "tasks": [build, task("cppm: run", application), test, task("cppm: configure", "configure"), task("cppm: watch", "watch")]})
}

const COMPILE_COMMANDS: &str = "C_Cpp.default.compileCommands";

/// Points the Microsoft C/C++ extension at the database cppm publishes, so
/// IntelliSense finds dependency headers. clangd finds the file on its own.
/// A list lets one workspace folder hold several cppm projects.
pub fn vscode_settings(project_path: &str) -> Value {
    json!({ COMPILE_COMMANDS: [database(project_path)] })
}

fn database(project_path: &str) -> String {
    format!("{project_path}/compile_commands.json")
}

/// `${workspaceFolder}`-relative location of a project inside a workspace.
fn workspace_path(workspace: &Path, root: &Path) -> Option<String> {
    let relative = root.strip_prefix(workspace).ok()?;
    Some(if relative.as_os_str().is_empty() {
        "${workspaceFolder}".to_owned()
    } else {
        format!(
            "${{workspaceFolder}}/{}",
            relative.to_string_lossy().replace('\\', "/")
        )
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Registration {
    Added,
    Present,
    /// The file has comments or a database the user chose; left untouched.
    Preserved,
}

/// Adds a project's database to `.vscode/settings.json` without disturbing
/// the user's other settings.
fn register(path: &Path, project_path: &str) -> Result<Registration> {
    let entry = Value::String(database(project_path));
    let Some(current) = read(path)? else {
        let mut bytes = serde_json::to_vec_pretty(&vscode_settings(project_path))?;
        bytes.push(b'\n');
        write_atomic(path, &bytes)?;
        return Ok(Registration::Added);
    };
    // VS Code allows comments (JSONC). Rewriting such a file would drop them,
    // so only plain JSON objects are extended.
    let Ok(Value::Object(mut settings)) = serde_json::from_slice::<Value>(&current) else {
        return Ok(Registration::Preserved);
    };
    let generated = |value: &str| {
        value.starts_with("${workspaceFolder}") && value.ends_with("/compile_commands.json")
    };
    let databases = match settings.get(COMPILE_COMMANDS) {
        None => vec![entry],
        Some(Value::Array(list)) if list.contains(&entry) => return Ok(Registration::Present),
        Some(Value::Array(list)) => list.iter().cloned().chain([entry]).collect(),
        Some(Value::String(value)) if Value::String(value.clone()) == entry => {
            return Ok(Registration::Present);
        }
        // A single database written by an earlier cppm becomes a list.
        Some(Value::String(value)) if generated(value) => vec![Value::String(value.clone()), entry],
        Some(_) => return Ok(Registration::Preserved),
    };
    settings.insert(COMPILE_COMMANDS.into(), Value::Array(databases));
    let mut bytes = serde_json::to_vec_pretty(&settings)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)?;
    Ok(Registration::Added)
}

fn home() -> Option<std::path::PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(std::path::PathBuf::from)
        .and_then(|home| home.canonicalize().ok())
}

/// A folder VS Code may have open around the project: the nearest parent that
/// already has `.vscode`. The home directory holds VS Code's own data in
/// `.vscode`, so the search stops below it.
fn enclosing_workspace(root: &Path) -> Option<&Path> {
    let home = home();
    root.ancestors()
        .skip(1)
        .take_while(|ancestor| Some(*ancestor) != home.as_deref() && ancestor.parent().is_some())
        .find(|ancestor| ancestor.join(".vscode").is_dir())
}

fn announce(settings: &Path, registration: Registration) {
    if registration == Registration::Added {
        println!("VS Code IntelliSense: {}", display(settings));
    }
}

/// Registers the project in the VS Code folder that contains it, so opening
/// either the project or its parent folder resolves dependency headers. Runs
/// after every configuration; editor integration never fails a build.
pub fn register_workspace(root: &Path) {
    let Ok(root) = root.canonicalize() else {
        return;
    };
    let Some(workspace) = enclosing_workspace(&root) else {
        return;
    };
    if let Some(project_path) = workspace_path(workspace, &root) {
        let settings = workspace.join(".vscode/settings.json");
        if let Ok(registration) = register(&settings, &project_path) {
            announce(&settings, registration);
        }
    }
}

/// `cppm init sub/dir` from VS Code's terminal: the terminal starts in the
/// open folder, so register the new project there right away.
pub fn register_init_workspace(cwd: &Path, root: &Path) {
    if std::env::var_os("TERM_PROGRAM").is_none_or(|program| program != "vscode") {
        return;
    }
    let (Ok(cwd), Ok(root)) = (cwd.canonicalize(), root.canonicalize()) else {
        return;
    };
    if cwd == root || Some(&cwd) == home().as_ref() || cwd.parent().is_none() {
        return;
    }
    if let Some(project_path) = workspace_path(&cwd, &root) {
        let settings = cwd.join(".vscode/settings.json");
        if let Ok(registration) = register(&settings, &project_path) {
            announce(&settings, registration);
        }
    }
}

/// Paths without Windows' extended-length `\\?\` prefix, for messages.
pub fn display(path: &Path) -> String {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(network) => format!(r"\\{network}"),
        None => text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned(),
    }
}

pub fn vscode(project: &Project, workspace: &Path) -> Result<()> {
    let root = project.root.canonicalize()?;
    let workspace = workspace
        .canonicalize()
        .context("cannot locate the VS Code workspace folder")?;
    let project_path = workspace_path(&workspace, &root)
        .context("the VS Code workspace must contain this cppm project")?;
    let settings = workspace.join(".vscode/settings.json");
    match register(&settings, &project_path)? {
        Registration::Preserved => eprintln!(
            "{} was preserved; add \"{}\" to \"{COMPILE_COMMANDS}\" for IntelliSense.",
            display(&settings),
            database(&project_path)
        ),
        _ => println!("VS Code IntelliSense: {}", display(&settings)),
    }
    let path = workspace.join(".vscode/tasks.json");
    let tasks = vscode_tasks(&project_path, project.manifest.package.target);
    // Existing custom tasks may contain comments and unrelated commands. Leave
    // them intact instead of rewriting their JSONC or changing their defaults.
    if let Some(current) = read(&path)? {
        let current = serde_json::from_slice::<Value>(&current).ok();
        // Upgrade only the exact tasks generated by earlier cppm versions.
        // A user's edited JSON/JSONC remains untouched.
        let mut previous = tasks.clone();
        previous["tasks"]
            .as_array_mut()
            .expect("generated task array")
            .retain(|task| task["label"] != "cppm: watch");
        ensure!(
            current.as_ref() == Some(&tasks) || current.as_ref() == Some(&previous),
            "{} already exists; custom tasks were preserved",
            path.display()
        );
        if current.as_ref() == Some(&previous) {
            write_if_changed(
                &path,
                format!("{}\n", serde_json::to_string_pretty(&tasks)?).as_bytes(),
            )?;
        }
    } else {
        let mut bytes = serde_json::to_vec_pretty(&tasks)?;
        bytes.push(b'\n');
        write_atomic(&path, &bytes)?;
    }
    println!("VS Code tasks: {}", path.display());
    println!("Build: Ctrl+Shift+B. Run: Tasks: Run Task -> cppm: run.");
    Ok(())
}

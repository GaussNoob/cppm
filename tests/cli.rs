#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

fn cppm(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .env_remove("CPPM_PROFILE")
        .env_remove("CPPM_STORE_DIR")
        .env_remove("CPPM_OFFLINE")
        .env_remove("CPPM_FROZEN_LOCKFILE")
        .env_remove("CPPM_VERBOSE")
        .env_remove("CPPM_SCRIPT_STACK")
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn init() -> TempDir {
    let temp = tempfile::tempdir().unwrap();
    success(cppm(temp.path(), &["init", "--name", "hello"]));
    temp
}

#[test]
fn interactive_aliases_offer_help_and_reject_pipes_without_changing_files() {
    let temp = tempfile::tempdir().unwrap();
    for alias in ["ui", "tui", "interactive"] {
        let help = success(cppm(temp.path(), &[alias, "--help"]));
        assert!(help.contains("interactive package explorer"));
        let output = cppm(temp.path(), &[alias]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("precisa de um terminal"));
        assert!(output.stdout.is_empty());
    }
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn packages_lists_database_defaults_and_aliases_without_conan() {
    let temp = tempfile::tempdir().unwrap();
    let output = success(cppm(temp.path(), &["packages"]));
    for entry in [
        "mariadb-connector-c@3.4.8",
        "libpq@17.11",
        "hiredis@1.3.0",
        "mongo-c-driver@2.5.8",
        "mysql=mariadb-connector-c@3.4.8",
        "postgres=libpq@17.11",
        "postgresql=libpq@17.11",
        "redis=hiredis@1.3.0",
        "mongodb=mongo-c-driver@2.5.8",
    ] {
        assert!(output.contains(entry), "missing {entry} in {output}");
    }
    assert!(output.contains("database servers run separately"));
}

#[cfg(unix)]
fn store_tools(root: &Path) -> std::path::PathBuf {
    let tools = root.join("tools");
    fs::create_dir(&tools).unwrap();
    let conan = tools.join("conan");
    fs::write(
        &conan,
        r#"#!/bin/sh
case "$1 $2" in
    'config home') printf '%s\n' "$CONAN_HOME";;
    'list *:*') printf '%s\n' '{"Local Cache":{"example/1.0":{"revisions":{"rev":{"packages":{"debug":{},"release":{}}}}}}}';;
    'cache clean') printf '%s\n' "$@" > "$CONAN_HOME/clean-args";;
    *) exit 42;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(conan, fs::Permissions::from_mode(0o755)).unwrap();
    tools
}

#[test]
#[cfg(unix)]
fn shared_store_commands_work_without_a_project_and_preserve_binaries() {
    let temp = tempfile::tempdir().unwrap();
    let tools = store_tools(temp.path());
    let inherited = temp.path().join("inherited");
    let selected = temp.path().join("selected store");
    let cli = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cppm"));
        command
            .current_dir(temp.path())
            .env("PATH", &tools)
            .env("CONAN_HOME", &inherited)
            .env("CPPM_STORE_DIR", &selected);
        command
    };
    assert_eq!(
        success(cli().args(["store", "path"]).output().unwrap()).trim(),
        selected.to_str().unwrap()
    );
    assert!(!inherited.exists());
    let status: serde_json::Value = serde_json::from_str(&success(
        cli().args(["store", "status", "--json"]).output().unwrap(),
    ))
    .unwrap();
    assert_eq!(status["recipes"], 1);
    assert_eq!(status["binaries"], 2);
    assert!(status["home_bytes"].is_u64());
    success(cli().args(["store", "prune"]).output().unwrap());
    assert_eq!(
        fs::read_to_string(selected.join("clean-args")).unwrap(),
        "cache\nclean\n*\n--build\n--download\n--temp\n"
    );
    success(
        cli()
            .args(["store", "prune", "--sources"])
            .output()
            .unwrap(),
    );
    assert!(
        fs::read_to_string(selected.join("clean-args"))
            .unwrap()
            .ends_with("--source\n")
    );
    // The explicit flag wins over both environment variables.
    assert_eq!(
        success(
            cli()
                .args(["--store-dir", "relative store", "store", "path"])
                .output()
                .unwrap()
        )
        .trim(),
        temp.path().join("relative store").to_str().unwrap()
    );
    assert_eq!(
        success(
            cli()
                .env_remove("CPPM_STORE_DIR")
                .args(["store", "path"])
                .output()
                .unwrap()
        )
        .trim(),
        inherited.to_str().unwrap()
    );
}

#[test]
#[cfg(unix)]
fn store_pruning_waits_for_another_project_to_release_the_shared_store() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    let temp = tempfile::tempdir().unwrap();
    let tools = store_tools(temp.path());
    let store = temp.path().join("store");
    fs::create_dir(&store).unwrap();
    let guard = fs::File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(store.join("cppm-store.lock"))
        .unwrap();
    guard.lock().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(temp.path())
        .env("PATH", &tools)
        .env("CPPM_STORE_DIR", &store)
        .args(["store", "prune"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut message = String::new();
    BufReader::new(child.stderr.take().unwrap())
        .read_line(&mut message)
        .unwrap();
    assert!(message.contains("Waiting for another cppm command"));
    assert!(child.try_wait().unwrap().is_none());
    assert!(!store.join("clean-args").exists());
    drop(guard);
    success(child.wait_with_output().unwrap());
    assert!(store.join("clean-args").is_file());
}

#[test]
fn packages_lists_network_gui_and_testing_libraries() {
    let temp = tempfile::tempdir().unwrap();
    let output = success(cppm(temp.path(), &["packages"]));
    for entry in [
        "boost@1.91.0",
        "cpp-httplib@0.62.0",
        "asio@1.38.2",
        "imgui@1.92.9b",
        "catch2@3.7.1",
        "gtest@1.18.0",
        "qt@6.12.0",
        "httplib=cpp-httplib@0.62.0",
        "dear-imgui=imgui@1.92.9b",
        "googletest=gtest@1.18.0",
        "qt6=qt@6.12.0",
        "opencv@4.14.0",
        "eigen@3.4.0",
        "protobuf@7.36.2",
        "glm@1.0.3",
        "onetbb@2023.1.0",
    ] {
        assert!(output.contains(entry), "missing {entry} in {output}");
    }
}

#[test]
fn init_preserves_existing_files() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join(".gitignore"), "secrets/").unwrap();
    success(cppm(temp.path(), &["init", "--name", "hello"]));
    assert_eq!(
        fs::read_to_string(temp.path().join(".gitignore")).unwrap(),
        "secrets/\n/.cppm/\n"
    );
    fs::write(temp.path().join("src/main.cpp"), "user code").unwrap();
    assert!(
        !cppm(temp.path(), &["init", "--name", "hello"])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("src/main.cpp")).unwrap(),
        "user code"
    );
}

#[test]
fn doctor_reports_all_missing_tools_as_json_without_a_project() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(temp.path())
        .env("PATH", temp.path())
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["format"], 1);
    assert_eq!(report["ok"], false);
    let checks = report["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 4);
    for name in ["cmake", "ninja", "conan"] {
        let check = checks.iter().find(|check| check["name"] == name).unwrap();
        assert_eq!(check["status"], "error");
        assert!(!check["hint"].as_str().unwrap().is_empty());
    }
    assert_eq!(checks[3]["status"], "skipped");
}

#[test]
fn batch_add_rejects_conflicting_versions_and_custom_targets_before_changes() {
    let temp = init();
    let before = fs::read(temp.path().join("cppm.toml")).unwrap();
    for args in [
        vec!["add", "json@3.11.3", "nlohmann_json@3.11.2"],
        vec!["add", "fmt", "json", "--target", "fmt::fmt"],
        vec!["install", "fmt", "bad/name"],
    ] {
        assert!(!cppm(temp.path(), &args).status.success());
        assert_eq!(fs::read(temp.path().join("cppm.toml")).unwrap(), before);
        assert!(!temp.path().join("cppm.lock").exists());
    }
}

#[test]
fn native_project_builds_runs_tests_and_preserves_exit_status() {
    let temp = init();
    let source = r#"#include <iostream>
#include <string>
int main(int argc, char** argv) {
    if (argc > 1 && std::string(argv[1]) == "fail") return 23;
    std::cout << (argc > 1 ? argv[1] : "native-ok") << '\n';
}

"#;
    fs::write(temp.path().join("src/main.cpp"), source).unwrap();
    // Discover the root from a subdirectory and forward one argument with spaces.
    let output = success(cppm(
        &temp.path().join("src"),
        &["run", "dev", "--", "hello world"],
    ));
    assert!(output.contains("hello world"));
    assert!(
        temp.path()
            .join(".cppm/build/debug/compile_commands.json")
            .is_file()
    );
    assert!(success(cppm(temp.path(), &["run", "test"])).contains("100% tests passed"));
    success(cppm(temp.path(), &["run", "build"]));
    assert!(
        temp.path()
            .join(format!(
                ".cppm/build/release/bin/hello{}",
                std::env::consts::EXE_SUFFIX
            ))
            .is_file()
    );
    assert_eq!(
        cppm(temp.path(), &["exec", "--", "fail"]).status.code(),
        Some(23)
    );
}

#[test]
fn configure_publishes_editor_flags_without_compiling_or_overwriting_custom_files() {
    let temp = init();
    fs::write(
        temp.path().join("src/main.cpp"),
        "#include <not-installed.h>\ninvalid code\n",
    )
    .unwrap();
    success(cppm(temp.path(), &["configure", "--offline"]));
    let database = temp.path().join("compile_commands.json");
    let debug = fs::read(&database).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&debug).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert!(
        parsed[0]["directory"]
            .as_str()
            .unwrap()
            .replace('\\', "/")
            .ends_with(".cppm/build/debug")
    );
    assert!(
        !temp
            .path()
            .join(format!(
                ".cppm/build/debug/bin/hello{}",
                std::env::consts::EXE_SUFFIX
            ))
            .exists()
    );
    success(cppm(temp.path(), &["configure", "--release", "--offline"]));
    assert_ne!(debug, fs::read(&database).unwrap());
    assert!(
        fs::read_to_string(temp.path().join(".gitignore"))
            .unwrap()
            .contains("/compile_commands.json")
    );
    fs::write(&database, "[]\n").unwrap();
    success(cppm(temp.path(), &["configure", "--offline"]));
    assert_eq!(fs::read_to_string(database).unwrap(), "[]\n");
    let snapshot = fs::read_to_string(temp.path().join(".cppm/compile_commands.json")).unwrap();
    assert!(snapshot.replace('\\', "/").contains(".cppm/build/debug"));
}

#[test]
fn configuration_output_hides_status_but_keeps_warnings_errors_and_verbose_details() {
    let temp = init();
    let path = temp.path().join("CMakeLists.txt");
    let base = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        format!(
            "{base}\nmessage(STATUS \"cppm-test-status\")\nmessage(WARNING \"cppm-test-warning\")\n"
        ),
    )
    .unwrap();
    let output = cppm(temp.path(), &["configure", "--offline"]);
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("cppm-test-status"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cppm-test-warning"));
    let output = cppm(temp.path(), &["configure", "--offline", "--verbose"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("cppm-test-status"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cppm-test-warning"));
    let manifest = temp.path().join("cppm.toml");
    let mut text = fs::read_to_string(&manifest).unwrap();
    text.push_str("\nconfigure = ['cppm', 'configure', '--offline']\n");
    fs::write(manifest, text).unwrap();
    assert!(
        success(cppm(temp.path(), &["--verbose", "run", "configure"])).contains("cppm-test-status")
    );
    fs::write(
        &path,
        format!("{base}\nmessage(FATAL_ERROR \"cppm-test-fatal\")\n"),
    )
    .unwrap();
    let output = cppm(temp.path(), &["configure", "--offline"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cppm-test-fatal"));
}

#[test]
fn frozen_install_requires_a_matching_lockfile() {
    let temp = init();
    assert!(
        !cppm(temp.path(), &["install", "--frozen-lockfile"])
            .status
            .success()
    );
    success(cppm(temp.path(), &["install"]));
    let before = fs::read(temp.path().join("cppm.lock")).unwrap();
    success(cppm(
        temp.path(),
        &["install", "--frozen-lockfile", "--offline"],
    ));
    assert_eq!(before, fs::read(temp.path().join("cppm.lock")).unwrap());
    let path = temp.path().join("cppm.toml");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("[dependencies]", "[dependencies]\nfmt = \"11.0.2\"");
    fs::write(path, text).unwrap();
    let output = cppm(temp.path(), &["install", "--frozen-lockfile"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("disagree"));
    assert_eq!(before, fs::read(temp.path().join("cppm.lock")).unwrap());
}

#[test]
fn vscode_tasks_build_and_run_the_project_from_a_parent_workspace_with_spaces() {
    let temp = tempfile::Builder::new()
        .prefix("cppm vscode workspace ")
        .tempdir()
        .unwrap();
    let root = temp.path().join("my app");
    success(cppm(temp.path(), &["init", "my app", "--name", "hello"]));
    // New projects point IntelliSense at the database cppm publishes.
    let own: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(".vscode/settings.json")).unwrap()).unwrap();
    assert_eq!(
        own["C_Cpp.default.compileCommands"],
        "${workspaceFolder}/compile_commands.json"
    );
    // A parent workspace keeps its own settings and gains the project's database.
    fs::create_dir(temp.path().join(".vscode")).unwrap();
    fs::write(
        temp.path().join(".vscode/settings.json"),
        "{\"editor.tabSize\": 2}",
    )
    .unwrap();
    success(cppm(&root, &["vscode", "--workspace", ".."]));
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.path().join(".vscode/settings.json")).unwrap())
            .unwrap();
    assert_eq!(settings["editor.tabSize"], 2);
    assert_eq!(
        settings["C_Cpp.default.compileCommands"],
        "${workspaceFolder}/my app/compile_commands.json"
    );
    let path = temp.path().join(".vscode/tasks.json");
    let tasks: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(tasks["tasks"][0]["group"]["isDefault"], true);
    for action in ["cppm: build", "cppm: run"] {
        let task = tasks["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|task| task["label"] == action)
            .unwrap();
        assert_eq!(task["type"], "process");
        let args: Vec<_> = task["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| {
                arg.as_str()
                    .unwrap()
                    .replace("${workspaceFolder}", temp.path().to_str().unwrap())
            })
            .collect();
        let args: Vec<_> = args.iter().map(String::as_str).collect();
        let output = success(cppm(temp.path(), &args));
        if action == "cppm: run" {
            assert!(output.contains("Hello from C++!"));
        }
    }
    fs::write(
        &path,
        "{\n // Custom user tasks\n \"version\": \"2.0.0\", \"tasks\": []\n}\n",
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        !cppm(&root, &["vscode", "--workspace", ".."])
            .status
            .success()
    );
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
#[cfg(unix)]
fn scripts_preserve_literal_arguments_and_inherit_options() {
    let temp = init();
    let path = temp.path().join("cppm.toml");
    let mut manifest = fs::read_to_string(&path).unwrap();
    manifest.push_str(
        "\nargs = \"printf '%s\\n'\"\ncycle = \"cppm run cycle\"\ninstall = \"cppm install\"\n",
    );
    fs::write(&path, manifest).unwrap();
    let literal = "$(touch should-not-exist); hello world";
    assert_eq!(
        success(cppm(temp.path(), &["run", "args", "--", literal])).trim(),
        literal
    );
    assert!(!temp.path().join("should-not-exist").exists());
    let output = cppm(temp.path(), &["run", "cycle"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("recursive script"));
    // The nested install must inherit --frozen-lockfile and reject a missing lock.
    assert!(
        !cppm(temp.path(), &["--frozen-lockfile", "run", "install"])
            .status
            .success()
    );
}

#[test]
#[cfg(unix)]
fn failed_add_does_not_modify_manifest_or_lockfile() {
    let temp = init();
    success(cppm(temp.path(), &["install"]));
    let manifest = fs::read(temp.path().join("cppm.toml")).unwrap();
    let lock = fs::read(temp.path().join("cppm.lock")).unwrap();
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let conan = tools.join("conan");
    fs::write(&conan, "#!/bin/sh\ncase \"$1\" in\n--version) echo 'Conan version 2.34.0';;\nprofile) exit 0;;\n*) exit 42;;\nesac\n").unwrap();
    fs::set_permissions(&conan, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(temp.path())
        .env("PATH", tools)
        .env_remove("CPPM_FROZEN_LOCKFILE")
        .args(["add", "fmt"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert_eq!(manifest, fs::read(temp.path().join("cppm.toml")).unwrap());
    assert_eq!(lock, fs::read(temp.path().join("cppm.lock")).unwrap());
}

#[test]
fn concurrent_mutation_is_rejected() {
    let temp = init();
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .open(temp.path().join(".cppm/project.lock"))
        .unwrap();
    lock.lock().unwrap();
    let output = cppm(temp.path(), &["install"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("another cppm command"));
}

#[test]
fn unknown_fields_and_unsafe_cmake_names_are_rejected() {
    let temp = init();
    let path = temp.path().join("cppm.toml");
    fs::write(
        &path,
        "[package]\nname='hello'\nversion='0.1.0'\ncpp_standrad=20\n",
    )
    .unwrap();
    assert!(!cppm(temp.path(), &["install"]).status.success());
    fs::write(&path, "[package]\nname='hello'\nversion='0.1.0'\n[dependencies.evil]\nversion='1.0'\ncmake-package='x)\\nexecute_process(COMMAND touch injected)'\ntargets=['x']\n").unwrap();
    assert!(!cppm(temp.path(), &["install"]).status.success());
    assert!(!temp.path().join("injected").exists());
}

#[test]
fn portable_scripts_preserve_arguments_and_inherit_options() {
    let temp = init();
    let path = temp.path().join("cppm.toml");
    let mut manifest = fs::read_to_string(&path).unwrap();
    manifest.push_str("\ncycle = ['cppm', 'run', 'cycle']\ninstall = ['cppm', 'install']\n");
    fs::write(&path, manifest).unwrap();
    fs::write(
        temp.path().join("src/main.cpp"),
        r#"#include <iostream>
int main(int argc, char** argv) {
    for (int i = 1; i < argc; ++i) std::cout << argv[i] << '\n';
}
"#,
    )
    .unwrap();
    let literal = "spaces & pipes | dollar $ percent % quotes \" '";
    let output = success(cppm(temp.path(), &["run", "dev", "--", literal]));
    assert!(output.lines().any(|line| line == literal));
    let output = cppm(temp.path(), &["run", "cycle"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("recursive script"));
    fs::remove_file(temp.path().join("cppm.lock")).unwrap();
    assert!(
        !cppm(temp.path(), &["--frozen-lockfile", "run", "install"])
            .status
            .success()
    );
}

#[cfg(windows)]
#[test]
fn powershell_scripts_preserve_data_and_exit_status() {
    let temp = init();
    let path = temp.path().join("cppm.toml");
    let mut manifest = fs::read_to_string(&path).unwrap();
    manifest.push_str("\nargs = 'Write-Output'\nfail = 'exit 23;'\ncycle = 'cppm run cycle'\n");
    fs::write(path, manifest).unwrap();
    let literal = "$(New-Item injected); hello & world";
    let output = success(cppm(temp.path(), &["run", "args", "--", literal]));
    assert_eq!(output.trim(), literal);
    assert!(!temp.path().join("injected").exists());
    assert_eq!(cppm(temp.path(), &["run", "fail"]).status.code(), Some(23));
    assert!(!cppm(temp.path(), &["run", "cycle"]).status.success());
}

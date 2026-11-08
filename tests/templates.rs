use std::{fs, path::Path, process::Command};

fn without_sdk(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cppm"));
    command.current_dir(root).args(args);
    for name in [
        "CPPM_EMSDK",
        "EMSDK",
        "EMSDK_NODE",
        "EMSDK_PYTHON",
        "EM_CONFIG",
        "EM_CACHE",
        "EM_NODE_JS",
        "EM_LLVM_ROOT",
        "EM_BINARYEN_ROOT",
    ] {
        command.env_remove(name);
    }
    let paths: Vec<_> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|directory| {
            !["em++", "em++.bat", "emcmake", "emcmake.bat"]
                .iter()
                .any(|name| directory.join(name).is_file())
        })
        .collect();
    command.env("PATH", std::env::join_paths(paths).unwrap());
    command
}

fn sdk_success(command: &mut Command) -> String {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires a configured SDK in ~/emsdk or the cppm checkout; no shell activation"]
fn wasm_discovers_installed_sdk_without_environment_activation() {
    let temp = tempfile::Builder::new()
        .prefix("cppm SDK discovery ")
        .tempdir()
        .unwrap();
    let root = temp.path();
    run(root, &["init", "--template", "wasm", "--name", "web-app"]);
    let report: serde_json::Value =
        serde_json::from_str(&sdk_success(&mut without_sdk(root, &["doctor", "--json"]))).unwrap();
    assert_eq!(report["ok"], true);
    let sdk = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "Emscripten")
        .unwrap();
    assert!(sdk["detail"].as_str().unwrap().contains("SDK:"));
    sdk_success(&mut without_sdk(root, &["build"]));
    let wasm = fs::read(root.join(".cppm/build/wasm-debug/bin/web-app.wasm")).unwrap();
    assert_eq!(&wasm[..4], b"\0asm");
    assert!(
        sdk_success(&mut without_sdk(root, &["run", "test", "--", "--verbose"]))
            .contains("Hello from C++!")
    );
}

#[test]
#[cfg(unix)]
#[ignore = "requires the cppm checkout SDK; checks an SDK path with spaces and shell metacharacters"]
fn wasm_explicit_sdk_directory_works_with_spaces_and_overrides_emsdk() {
    use std::os::unix::fs::symlink;
    let installed = Path::new(env!("CARGO_MANIFEST_DIR")).join(".qa/emsdk");
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    run(root, &["init", "--template", "wasm", "--name", "web-app"]);
    let sdk = root.join("SDK with spaces & $literal");
    fs::create_dir(&sdk).unwrap();
    fs::copy(installed.join(".emscripten"), sdk.join(".emscripten")).unwrap();
    symlink(installed.join("upstream"), sdk.join("upstream")).unwrap();
    symlink(installed.join("node"), sdk.join("node")).unwrap();
    let mut build = without_sdk(root, &["build"]);
    build
        .env("CPPM_EMSDK", &sdk)
        .env("EM_CACHE", root.join("isolated-cache"))
        .env("EMSDK", root.join("missing-other-sdk"));
    sdk_success(&mut build);
    let mut test = without_sdk(root, &["run", "test", "--", "--verbose"]);
    test.env("CPPM_EMSDK", &sdk)
        .env("EM_CACHE", root.join("isolated-cache"));
    assert!(sdk_success(&mut test).contains("Hello from C++!"));
}

fn run(root: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn wasm_rejects_native_dependencies_without_changing_the_manifest() {
    let temp = tempfile::tempdir().unwrap();
    run(
        temp.path(),
        &["init", "--template", "wasm", "--name", "web-app"],
    );
    let manifest = fs::read(temp.path().join("cppm.toml")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(temp.path())
        .args(["add", "json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("WebAssembly"));
    assert_eq!(manifest, fs::read(temp.path().join("cppm.toml")).unwrap());
}

#[test]
#[ignore = "requires a configured Emscripten SDK, CMake, Ninja, Node.js, Python 3 and localhost HTTP"]
fn wasm_builds_real_artifacts_and_runs_in_node() {
    let temp = tempfile::Builder::new()
        .prefix("cppm wasm ")
        .tempdir()
        .unwrap();
    let root = temp.path();
    run(root, &["init", "--template", "wasm", "--name", "web-app"]);
    run(root, &["build"]);
    let wasm = fs::read(root.join(".cppm/build/wasm-debug/bin/web-app.wasm")).unwrap();
    assert_eq!(&wasm[..4], b"\0asm");
    let output = run(root, &["run", "test", "--", "--verbose"]);
    assert!(output.contains("100% tests passed"));
    assert!(output.contains("Hello from C++!"));
    run(root, &["run", "build"]);
    for extension in ["html", "js", "wasm"] {
        assert!(
            root.join(format!(".cppm/build/wasm-release/bin/web-app.{extension}"))
                .is_file()
        );
    }
    let report: serde_json::Value =
        serde_json::from_str(&run(root, &["doctor", "--json"])).unwrap();
    assert_eq!(report["ok"], true);
    let checks = report["checks"].as_array().unwrap();
    assert!(!checks.iter().any(|check| check["name"] == "conan"));
    assert!(checks.iter().any(|check| {
        check["name"] == "WebAssembly" && check["detail"].as_str().unwrap().contains("Node.js")
    }));
    dev_server(root, "wasm");
    watch_server(root, "wasm");
    let shell = root.join("web/shell.html");
    fs::write(
        &shell,
        fs::read_to_string(&shell)
            .unwrap()
            .replace("C++ running in WebAssembly", "Updated WebAssembly shell"),
    )
    .unwrap();
    run(root, &["build"]);
    assert!(
        fs::read_to_string(root.join(".cppm/build/wasm-debug/bin/web-app.html"))
            .unwrap()
            .contains("Updated WebAssembly shell")
    );
}

#[test]
#[ignore = "requires Conan, Python 3, a C++ toolchain and network access on first run"]
fn drogon_template_serves_json_over_http() {
    let temp = tempfile::Builder::new()
        .prefix("cppm drogon ")
        .tempdir()
        .unwrap();
    let root = temp.path();
    run(root, &["init", "--template", "drogon", "--name", "api-app"]);
    assert!(run(root, &["run", "test"]).contains("100% tests passed"));
    run(root, &["run", "build"]);
    dev_server(root, "drogon");
    watch_server(root, "drogon");
}

fn dev_server(root: &Path, template: &str) {
    check_server(root, template, false);
}

fn watch_server(root: &Path, template: &str) {
    check_server(root, template, true);
}

fn check_server(root: &Path, template: &str, watch: bool) {
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/template-dev.py"))
        .arg(env!("CARGO_BIN_EXE_cppm"))
        .arg(root)
        .arg(template)
        .arg(if watch { "watch" } else { "run" })
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{template} dev (watch={watch})\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn wasm_doctor_reports_the_sdk_instead_of_native_tools() {
    let temp = tempfile::tempdir().unwrap();
    run(
        temp.path(),
        &["init", "--template", "wasm", "--name", "web"],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(temp.path())
        .env("PATH", temp.path())
        .env_remove("EMSDK_NODE")
        .env("CPPM_EMSDK", temp.path().join("missing-sdk"))
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], false);
    let checks = report["checks"].as_array().unwrap();
    for name in ["Emscripten", "Node.js", "Python 3"] {
        let check = checks.iter().find(|check| check["name"] == name).unwrap();
        assert_eq!(check["status"], "error");
    }
    assert!(!checks.iter().any(|check| check["name"] == "conan"));
    assert!(!checks.iter().any(|check| check["name"] == "C++"));
    let probe = checks
        .iter()
        .find(|check| check["name"] == "WebAssembly")
        .unwrap();
    assert_eq!(probe["status"], "skipped");
}

#[test]
#[cfg(unix)]
fn wasm_without_sdk_explains_activation_before_preparing_dependencies() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    run(
        temp.path(),
        &["init", "--template", "wasm", "--name", "web"],
    );
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    for name in ["cmake", "ninja"] {
        let path = tools.join(name);
        fs::write(&path, "#!/bin/sh\nprintf '3.31.6\\n'\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let before = fs::read(temp.path().join("cppm.toml")).unwrap();
    let sdk = temp.path().join("selected-sdk");
    let sdk_tools = sdk.join("upstream/emscripten");
    fs::create_dir_all(&sdk_tools).unwrap();
    fs::write(sdk.join(".emscripten"), "NODE_JS = 'node'\n").unwrap();
    for missing in ["em++", "emcmake"] {
        for args in [&["build"][..], &["configure"], &["run", "dev"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
                .current_dir(temp.path())
                .env("PATH", &tools)
                .env("CPPM_EMSDK", &sdk)
                .args(args)
                .output()
                .unwrap();
            assert!(!output.status.success());
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(error.contains(missing), "{error}");
            assert!(error.contains("Emscripten SDK"), "{error}");
            assert!(error.contains("emsdk_env.sh"), "{error}");
            assert!(error.contains("emsdk_env.fish"), "{error}");
            assert!(error.contains("cppm doctor"), "{error}");
        }
        let compiler = sdk_tools.join("em++");
        fs::write(&compiler, "#!/bin/sh\nprintf 'emcc 7.0.0\\n'\n").unwrap();
        fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(before, fs::read(temp.path().join("cppm.toml")).unwrap());
    for path in ["cppm.lock", ".cppm/dependencies.cmake", ".cppm/build"] {
        assert!(!temp.path().join(path).exists(), "{path}");
    }
}

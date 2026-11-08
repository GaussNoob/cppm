use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cppm(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .env_remove("CPPM_PROFILE")
        .env_remove("CPPM_STORE_DIR")
        .env_remove("CPPM_OFFLINE")
        .env_remove("CPPM_FROZEN_LOCKFILE")
        .env_remove("CPPM_VERBOSE")
        .env_remove("CPPM_SCRIPT_STACK")
        .env_remove("CPPM_WATCH_ACTIVE")
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    let log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{log}");
    log
}

fn project() -> tempfile::TempDir {
    let temp = tempfile::Builder::new()
        .prefix("cppm workflow space ")
        .tempdir()
        .unwrap();
    success(cppm(temp.path(), &["init", "--name", "workflow-app"]));
    temp
}

#[test]
fn incremental_builds_reuse_configuration_and_observe_sources_headers_and_flags() {
    let temp = project();
    let root = temp.path();
    fs::create_dir(root.join("include")).unwrap();
    fs::write(root.join("include/value.h"), "#define VALUE 1\n").unwrap();
    let source = "#include <iostream>\n#include <value.h>\n#ifndef CUSTOM\n#define CUSTOM 0\n#endif\n#ifndef OTHER\n#define OTHER 0\n#endif\nint main() { std::cout << \"value=\" << VALUE << \" custom=\" << CUSTOM << \" other=\" << OTHER << '\\n'; }\n";
    fs::write(root.join("src/main.cpp"), source).unwrap();
    assert!(success(cppm(root, &["exec"])).contains("value=1 custom=0 other=0"));
    let cache = root.join(".cppm/build/debug/CMakeCache.txt");
    let modified = fs::metadata(&cache).unwrap().modified().unwrap();
    let binary = root.join(format!(
        ".cppm/build/debug/bin/workflow-app{}",
        std::env::consts::EXE_SUFFIX
    ));
    let binary_modified = fs::metadata(&binary).unwrap().modified().unwrap();
    let log = success(cppm(root, &["build", "--verbose"]));
    assert!(log.contains("Reusing CMake configuration"), "{log}");
    assert_eq!(modified, fs::metadata(&cache).unwrap().modified().unwrap());
    assert_eq!(
        binary_modified,
        fs::metadata(&binary).unwrap().modified().unwrap()
    );

    fs::write(root.join("include/value.h"), "#define VALUE 2\n").unwrap();
    assert!(success(cppm(root, &["exec"])).contains("value=2"));
    fs::write(root.join("src/extra.cpp"), "int extra() { return 3; }\n").unwrap();
    fs::write(
        root.join("src/main.cpp"),
        source
            .replace("int main()", "int extra();\nint main()")
            .replace("<< VALUE", "<< (VALUE + extra())"),
    )
    .unwrap();
    assert!(success(cppm(root, &["exec"])).contains("value=5"));
    let database: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("compile_commands.json")).unwrap()).unwrap();
    assert_eq!(database.as_array().unwrap().len(), 2);
    fs::remove_file(root.join("src/extra.cpp")).unwrap();
    fs::write(root.join("src/main.cpp"), source).unwrap();
    assert!(success(cppm(root, &["exec"])).contains("value=2"));

    let cmake = root.join("CMakeLists.txt");
    fs::write(
        &cmake,
        format!(
            "{}\ntarget_compile_definitions(${{PROJECT_NAME}} PRIVATE CUSTOM=9)\n",
            fs::read_to_string(&cmake).unwrap()
        ),
    )
    .unwrap();
    assert!(success(cppm(root, &["exec"])).contains("custom=9"));
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .env("CXXFLAGS", "-DOTHER=7")
        .args(["exec"])
        .output()
        .unwrap();
    assert!(success(output).contains("other=7"));
    assert!(success(cppm(root, &["exec"])).contains("other=0"));
    let failed = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .env("CXX", root.join("missing-cppm-compiler"))
        .args(["build"])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(success(cppm(root, &["exec"])).contains("value=2 custom=9 other=0"));

    let mut contents = fs::read_to_string(&cache).unwrap();
    contents.push_str("\nCPPM_CACHE_SENTINEL:STRING=keep\n");
    fs::write(&cache, contents).unwrap();
    success(cppm(root, &["build", "--fresh"]));
    assert!(
        !fs::read_to_string(cache)
            .unwrap()
            .contains("CPPM_CACHE_SENTINEL")
    );
}

#[test]
fn clean_previews_artifacts_and_preserves_dependencies_and_custom_editor_files() {
    let temp = project();
    let root = temp.path();
    success(cppm(root, &["build"]));
    success(cppm(root, &["build", "--release"]));
    let manifest = fs::read(root.join("cppm.toml")).unwrap();
    let lock = fs::read(root.join("cppm.lock")).unwrap();
    fs::create_dir_all(root.join(".cppm/conan")).unwrap();
    fs::write(root.join(".cppm/conan/keep.txt"), "dependencies").unwrap();
    fs::create_dir_all(root.join(".cppm/tools")).unwrap();
    fs::write(root.join(".cppm/tools/keep.txt"), "tools").unwrap();
    let database = root.join("compile_commands.json");
    let release_database = fs::read(&database).unwrap();
    assert!(success(cppm(root, &["clean", "--dry-run"])).contains("Would remove"));
    assert!(root.join(".cppm/build/debug").is_dir());
    assert!(root.join(".cppm/build/release").is_dir());
    assert_eq!(release_database, fs::read(&database).unwrap());
    success(cppm(root, &["clean", "debug"]));
    assert!(!root.join(".cppm/build/debug").exists());
    assert!(root.join(".cppm/build/release").exists());
    assert_eq!(release_database, fs::read(&database).unwrap());
    success(cppm(root, &["clean", "release"]));
    assert!(!database.exists());
    success(cppm(root, &["build"]));
    fs::write(&database, "[]\n").unwrap();
    success(cppm(root, &["clean"]));
    assert_eq!(fs::read_to_string(database).unwrap(), "[]\n");
    assert_eq!(fs::read(root.join("cppm.toml")).unwrap(), manifest);
    assert_eq!(fs::read(root.join("cppm.lock")).unwrap(), lock);
    assert_eq!(
        fs::read_to_string(root.join(".cppm/conan/keep.txt")).unwrap(),
        "dependencies"
    );
    assert_eq!(
        fs::read_to_string(root.join(".cppm/tools/keep.txt")).unwrap(),
        "tools"
    );
    assert!(root.join("src/main.cpp").is_file());
    assert!(success(cppm(root, &["clean"])).contains("No build artifacts"));
}

#[cfg(unix)]
#[test]
fn clean_does_not_follow_directory_symlinks() {
    use std::os::unix::fs::symlink;
    let temp = project();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("keep.txt"), "user file").unwrap();
    let build = temp.path().join(".cppm/build");
    symlink(outside.path(), &build).unwrap();
    let output = cppm(temp.path(), &["clean"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symbolic link"));
    fs::remove_file(&build).unwrap();
    fs::create_dir_all(build.join("debug")).unwrap();
    symlink(outside.path(), build.join("debug/external")).unwrap();
    success(cppm(temp.path(), &["clean"]));
    assert_eq!(
        fs::read_to_string(outside.path().join("keep.txt")).unwrap(),
        "user file"
    );
}

#[test]
fn vscode_upgrades_generated_tasks_and_preserves_custom_tasks() {
    let temp = project();
    let root = temp.path();
    let path = root.join(".vscode/tasks.json");
    let mut tasks: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    tasks["tasks"]
        .as_array_mut()
        .unwrap()
        .retain(|task| task["label"] != "cppm: watch");
    fs::write(&path, serde_json::to_vec_pretty(&tasks).unwrap()).unwrap();
    success(cppm(root, &["vscode"]));
    let mut tasks: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(
        tasks["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["label"] == "cppm: watch")
    );
    tasks["tasks"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"label": "custom", "command": "my-tool"}));
    let custom = serde_json::to_vec_pretty(&tasks).unwrap();
    fs::write(&path, &custom).unwrap();
    assert!(!cppm(root, &["vscode"]).status.success());
    assert_eq!(fs::read(path).unwrap(), custom);
}

#[test]
#[ignore = "requires Conan 2, a C++ compiler and fmt/11.0.2 available offline"]
fn conan_cache_revalidates_inherited_profiles_generated_files_and_explicit_refresh() {
    let temp = project();
    let root = temp.path();
    success(cppm(root, &["add", "fmt@11.0.2", "--offline"]));
    fs::write(root.join("src/main.cpp"), "#include <fmt/core.h>\n#ifndef PROFILE_VALUE\n#define PROFILE_VALUE 0\n#endif\nint main() { fmt::print(\"profile={}\\n\", PROFILE_VALUE); }\n").unwrap();
    let log = success(cppm(root, &["exec", "--offline", "--verbose"]));
    assert!(
        log.contains("Reusing installed Conan dependencies"),
        "{log}"
    );
    assert!(log.contains("profile=0"));
    let log = success(cppm(root, &["build", "--offline", "--verbose"]));
    assert!(
        log.contains("Reusing installed Conan dependencies"),
        "{log}"
    );
    assert!(log.contains("Reusing CMake configuration"), "{log}");
    fs::remove_file(root.join(".cppm/conan/debug/conan_toolchain.cmake")).unwrap();
    let log = success(cppm(root, &["build", "--offline", "--verbose"]));
    assert!(!log.contains("Reusing installed Conan dependencies"));
    assert!(
        root.join(".cppm/conan/debug/conan_toolchain.cmake")
            .exists()
    );
    for args in [
        &["install", "--offline", "--verbose"][..],
        &["build", "--fresh", "--offline", "--verbose"],
    ] {
        assert!(!success(cppm(root, args)).contains("Reusing installed Conan dependencies"));
    }
    let base = root.join("base-profile");
    let profile = root.join("custom-profile");
    fs::write(&profile, "include(./base-profile)\n").unwrap();
    for value in [5, 8] {
        fs::write(
            &base,
            format!("include(default)\n[conf]\ntools.build:cxxflags=['-DPROFILE_VALUE={value}']\n"),
        )
        .unwrap();
        let log = success(cppm(
            root,
            &[
                "--profile",
                profile.to_str().unwrap(),
                "exec",
                "--offline",
                "--verbose",
            ],
        ));
        assert!(log.contains(&format!("profile={value}")), "{log}");
        assert!(
            !log.contains("Reusing installed Conan dependencies"),
            "{log}"
        );
        let log = success(cppm(
            root,
            &[
                "--profile",
                profile.to_str().unwrap(),
                "exec",
                "--offline",
                "--verbose",
            ],
        ));
        assert!(
            log.contains("Reusing installed Conan dependencies"),
            "{log}"
        );
    }
}

#[cfg(unix)]
mod watching {
    use super::*;
    use std::{
        thread,
        time::{Duration, Instant},
    };

    struct Watcher(std::process::Child);
    impl Watcher {
        fn stop(&mut self) {
            if self.0.try_wait().unwrap().is_none() {
                unsafe {
                    libc::kill(self.0.id() as i32, libc::SIGTERM);
                }
                let deadline = Instant::now() + Duration::from_secs(5);
                while self.0.try_wait().unwrap().is_none() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(20));
                }
                if self.0.try_wait().unwrap().is_none() {
                    self.0.kill().unwrap();
                }
                self.0.wait().unwrap();
            }
        }
    }
    impl Drop for Watcher {
        fn drop(&mut self) {
            self.stop();
        }
    }

    fn wait(root: &Path, watcher: &mut Watcher, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !condition() {
            assert!(
                watcher.0.try_wait().unwrap().is_none(),
                "watch exited:\n{}",
                fs::read_to_string(root.join("watch.log")).unwrap_or_default()
            );
            assert!(
                Instant::now() < deadline,
                "watch timed out:\n{}",
                fs::read_to_string(root.join("watch.log")).unwrap_or_default()
            );
            thread::sleep(Duration::from_millis(30));
        }
    }

    fn runs(root: &Path) -> Vec<String> {
        fs::read_to_string(root.join("runs.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn stopped(pid: i32) -> bool {
        #[cfg(target_os = "linux")]
        if fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
            stat.split_once(") ")
                .is_some_and(|(_, rest)| rest.starts_with('Z'))
        }) {
            return true;
        }
        unsafe { libc::kill(pid, 0) == -1 }
    }

    #[test]
    fn watch_restarts_on_edits_recovers_after_compile_errors_and_stops_descendants() {
        let temp = project();
        let root = temp.path();
        fs::create_dir(root.join("include")).unwrap();
        fs::write(root.join("include/value.h"), "#define VALUE 1\n").unwrap();
        let source = "#include <value.h>\n#include <unistd.h>\n#include <fstream>\n#include <thread>\n#include <chrono>\nint main(int argc, char** argv) { std::ofstream log(\"runs.log\", std::ios::app); log << getpid() << ':' << VALUE << ':' << (argc > 1 ? argv[1] : \"\") << std::endl; log.close(); for (;;) std::this_thread::sleep_for(std::chrono::seconds(1)); }\n";
        fs::write(root.join("src/main.cpp"), source).unwrap();
        let log = fs::File::create(root.join("watch.log")).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_cppm"))
            .current_dir(root)
            .args([
                "watch",
                "--interval",
                "50",
                "--debounce",
                "150",
                "--",
                "argument with spaces",
            ])
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        let mut watcher = Watcher(child);
        wait(root, &mut watcher, || !runs(root).is_empty());
        assert!(runs(root)[0].ends_with(":1:argument with spaces"));
        let first: i32 = runs(root)[0].split(':').next().unwrap().parse().unwrap();
        for value in [2, 3, 4] {
            fs::write(
                root.join("include/value.h"),
                format!("#define VALUE {value}\n"),
            )
            .unwrap();
            thread::sleep(Duration::from_millis(20));
        }
        wait(root, &mut watcher, || {
            runs(root)
                .iter()
                .any(|run| run.ends_with(":4:argument with spaces"))
        });
        assert!(
            stopped(first),
            "old application remains running after restart"
        );
        thread::sleep(Duration::from_millis(600));
        assert_eq!(runs(root).len(), 2, "generated files caused extra restarts");
        fs::write(root.join("src/main.cpp"), "invalid C++ code\n").unwrap();
        wait(root, &mut watcher, || {
            fs::read_to_string(root.join("watch.log"))
                .unwrap()
                .contains("waiting for changes")
        });
        fs::write(root.join("src/main.cpp"), source).unwrap();
        wait(root, &mut watcher, || runs(root).len() >= 3);
        let last: i32 = runs(root)
            .last()
            .unwrap()
            .split(':')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        watcher.stop();
        assert_eq!(watcher.0.wait().unwrap().code(), Some(130));
        assert!(
            stopped(last),
            "application remains running after watch exits"
        );
    }
}

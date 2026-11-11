//! Dependency inspection and maintenance: `list`, `why`, `licenses`,
//! `outdated`, `upgrade`, `ci`, dev dependencies and Conan-free rebuilds.

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cppm"));
    command
        .current_dir(root)
        .env_remove("CPPM_PROFILE")
        .env_remove("CPPM_STORE_DIR")
        .env_remove("CPPM_OFFLINE")
        .env_remove("CPPM_FROZEN_LOCKFILE")
        .env_remove("CPPM_VERBOSE")
        .env_remove("CPPM_SCRIPT_STACK");
    command
}

fn cppm(root: &Path, args: &[&str]) -> Output {
    command(root).args(args).output().unwrap()
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

fn failure(output: Output) -> String {
    assert!(!output.status.success());
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn inspection_commands_work_before_any_dependency_is_installed() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    success(cppm(root, &["init", "--name", "plain"]));
    let listing = success(cppm(root, &["list"]));
    assert!(listing.starts_with("plain 0.1.0 "), "{listing}");
    assert!(listing.contains("No dependencies"));
    for args in [&["ls", "--json"][..], &["list", "--all", "--json"]] {
        let json: serde_json::Value = serde_json::from_str(&success(cppm(root, args))).unwrap();
        assert_eq!(json["name"], "plain");
        assert_eq!(json["dependencies"], serde_json::json!([]));
        assert_eq!(json["dev-dependencies"], serde_json::json!([]));
    }
    assert_eq!(success(cppm(root, &["licenses", "--json"])).trim(), "{}");
    assert!(failure(cppm(root, &["why", "fmt"])).contains("no dependencies"));
    // Without Conan in PATH a dependency-free project has nothing to query.
    assert!(
        success(
            command(root)
                .env("PATH", "")
                .args(["outdated"])
                .output()
                .unwrap()
        )
        .contains("up to date")
    );
    assert!(
        failure(cppm(root, &["ci"])).contains("cppm.lock is missing"),
        "ci must not create a lockfile"
    );
    assert!(!root.join("cppm.lock").exists());
}

#[test]
fn declared_dependencies_are_listed_but_trees_need_a_current_installation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    success(cppm(root, &["init", "--name", "declared"]));
    let manifest = fs::read_to_string(root.join("cppm.toml")).unwrap();
    fs::write(
        root.join("cppm.toml"),
        format!(
            "{}\n[dev-dependencies]\ncatch2 = \"3.7.1\"\n",
            manifest.replace("[dependencies]", "[dependencies]\nfmt = \"11.0.2\"")
        ),
    )
    .unwrap();
    let listing = success(cppm(root, &["list"]));
    assert!(
        listing.contains("dependencies:\n└── fmt 11.0.2"),
        "{listing}"
    );
    assert!(
        listing.contains("dev-dependencies:\n└── catch2 3.7.1"),
        "{listing}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&success(cppm(root, &["list", "--json"]))).unwrap();
    assert_eq!(json["dev-dependencies"][0]["name"], "catch2");
    // A tree, a reason or a license must come from a real resolution.
    for args in [&["list", "--all"][..], &["why", "fmt"], &["licenses"]] {
        assert!(
            failure(cppm(root, args)).contains("run `cppm install`"),
            "{args:?}"
        );
    }
    // A package cannot be an application and a dev dependency at once.
    fs::write(
        root.join("cppm.toml"),
        fs::read_to_string(root.join("cppm.toml"))
            .unwrap()
            .replace("catch2 = \"3.7.1\"", "fmt = \"11.0.2\""),
    )
    .unwrap();
    assert!(failure(cppm(root, &["list"])).contains("both [dependencies] and [dev-dependencies]"));
}

#[test]
fn mistyped_scripts_and_flags_are_explained() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    success(cppm(root, &["init", "--name", "typos"]));
    let error = failure(cppm(root, &["run", "biuld"]));
    assert!(
        error.contains("unknown script 'biuld' (did you mean 'build'?)"),
        "{error}"
    );
    assert!(!failure(cppm(root, &["run", "deploy"])).contains("did you mean"));
    // --dev names where packages go; it is meaningless for a plain install.
    assert!(!cppm(root, &["install", "--dev"]).status.success());
    assert!(failure(cppm(root, &["upgrade", "--frozen-lockfile"])).contains("--frozen-lockfile"));
}

#[cfg(unix)]
mod with_conan {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    struct Workspace {
        temp: tempfile::TempDir,
    }

    impl Workspace {
        fn store(&self) -> std::path::PathBuf {
            self.temp.path().join("store")
        }

        fn project(&self) -> std::path::PathBuf {
            self.temp.path().join("app")
        }

        fn calls(&self) -> usize {
            fs::read_to_string(self.temp.path().join("conan-calls.log"))
                .map_or(0, |log| log.lines().count())
        }

        /// Runs cppm with a Conan launcher that records every start.
        fn command(&self, args: &[&str]) -> Command {
            let mut command = super::command(&self.project());
            let path = std::env::var_os("PATH").unwrap();
            let mut paths = vec![self.temp.path().join("tools")];
            paths.extend(std::env::split_paths(&path));
            command
                .env("PATH", std::env::join_paths(paths).unwrap())
                .env("CONAN_HOME", self.temp.path().join("unused-cache"))
                .arg("--store-dir")
                .arg(self.store())
                .args(args);
            command
        }

        fn run(&self, args: &[&str]) -> String {
            success(self.command(args).output().unwrap())
        }

        fn export(&self, name: &str, recipe: &str, header: &str, versions: &[&str]) {
            let fixture = self.temp.path().join(name);
            fs::create_dir_all(&fixture).unwrap();
            fs::write(
                fixture.join("conanfile.py"),
                format!(
                    r#"import os
from conan import ConanFile
from conan.tools.files import copy

class Fixture(ConanFile):
    name = "{name}"
    package_type = "header-library"
    exports_sources = "*.h"
{recipe}
    def package(self):
        copy(self, "*.h", self.source_folder, os.path.join(self.package_folder, "include"))

    def package_info(self):
        self.cpp_info.bindirs = []
        self.cpp_info.libdirs = []
"#
                ),
            )
            .unwrap();
            fs::write(fixture.join(format!("{name}.h")), header).unwrap();
            for version in versions {
                success(
                    Command::new("conan")
                        .env("CONAN_HOME", self.store())
                        .args(["export", "--version", version])
                        .arg(&fixture)
                        .output()
                        .unwrap(),
                );
            }
        }
    }

    fn workspace() -> Workspace {
        let workspace = Workspace {
            temp: tempfile::tempdir().unwrap(),
        };
        let root = workspace.temp.path();
        let real = String::from_utf8(
            Command::new("sh")
                .args(["-c", "command -v conan"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        fs::create_dir(root.join("tools")).unwrap();
        let shim = root.join("tools/conan");
        fs::write(
            &shim,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
                root.join("conan-calls.log").display(),
                real.trim()
            ),
        )
        .unwrap();
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        workspace.export(
            "baselib",
            "    license = \"MIT\"\n",
            "inline int base_value() { return 42; }\n",
            &["0.1.0"],
        );
        workspace.export(
            "upperlib",
            "    license = \"BSD-3-Clause\", \"MIT\"\n    requires = \"baselib/0.1.0\"\n",
            "#include <baselib.h>\ninline int upper_value() { return base_value() + 1; }\n",
            &["1.0.0", "1.2.0", "2.0.0"],
        );
        workspace.export(
            "devcheck",
            "    license = \"BSL-1.0\"\n",
            "#define DEVCHECK(condition) ((condition) ? 0 : 1)\n",
            &["3.1.0"],
        );
        // Everything resolves from the local store; no remote is ever asked.
        success(
            Command::new("conan")
                .env("CONAN_HOME", workspace.store())
                .args(["remote", "disable", "*"])
                .output()
                .unwrap(),
        );
        fs::create_dir(workspace.project()).unwrap();
        workspace.run(&["init", "--name", "app"]);
        workspace
    }

    #[test]
    #[ignore = "requires Conan 2, CMake, Ninja and a C++ compiler; no network needed"]
    fn dev_dependencies_inspection_and_upgrades_work_from_a_local_store() {
        let workspace = workspace();
        let root = workspace.project();
        // The user's comments and layout must survive every manifest edit.
        let manifest = fs::read_to_string(root.join("cppm.toml")).unwrap();
        fs::write(
            root.join("cppm.toml"),
            manifest.replace(
                "[dependencies]",
                "# Runtime libraries\n[dependencies]  # reviewed monthly",
            ),
        )
        .unwrap();
        assert!(
            workspace
                .run(&["add", "upperlib@1.0.0"])
                .contains("Added upperlib")
        );
        assert!(
            workspace
                .run(&["add", "--dev", "devcheck"])
                .contains("Added devcheck")
        );
        let manifest = fs::read_to_string(root.join("cppm.toml")).unwrap();
        assert!(
            manifest.contains("# Runtime libraries\n[dependencies]  # reviewed monthly\nupperlib = \"1.0.0\"\n\n[dev-dependencies]\ndevcheck = \"3.1.0\"\n\n[scripts]"),
            "{manifest}"
        );
        let lock: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("cppm.lock")).unwrap()).unwrap();
        assert_eq!(lock["dependencies"]["upperlib"], "1.0.0");
        assert_eq!(lock["dev-dependencies"]["devcheck"], "3.1.0");

        // The application links cppm_dependencies only; tests opt into both.
        fs::write(
            root.join("src/main.cpp"),
            "#include <upperlib.h>\n#if __has_include(<devcheck.h>)\n#error dev dependencies reached the application\n#endif\n#include <iostream>\nint main() { std::cout << upper_value() << '\\n'; }\n",
        )
        .unwrap();
        fs::create_dir(root.join("tests")).unwrap();
        fs::write(
            root.join("tests/unit.cpp"),
            "#include <devcheck.h>\n#include <upperlib.h>\nint main() { return DEVCHECK(upper_value() == 43); }\n",
        )
        .unwrap();
        let cmake = fs::read_to_string(root.join("CMakeLists.txt")).unwrap();
        fs::write(
            root.join("CMakeLists.txt"),
            format!("{cmake}add_executable(unit tests/unit.cpp)\ntarget_link_libraries(unit PRIVATE cppm_dependencies cppm_dev_dependencies)\nadd_test(NAME unit COMMAND unit)\n"),
        )
        .unwrap();
        assert!(workspace.run(&["exec"]).lines().any(|line| line == "43"));
        assert!(workspace.run(&["test"]).contains("100% tests passed"));

        let tree = workspace.run(&["list", "--all"]);
        assert!(
            tree.contains("dependencies:\n└── upperlib 1.0.0\n    └── baselib 0.1.0\n\ndev-dependencies:\n└── devcheck 3.1.0"),
            "{tree}"
        );
        assert!(!workspace.run(&["list"]).contains("baselib"));
        let json: serde_json::Value =
            serde_json::from_str(&workspace.run(&["ls", "--depth", "1", "--json"])).unwrap();
        assert_eq!(
            json["dependencies"][0]["dependencies"][0]["name"],
            "baselib"
        );
        assert_eq!(
            workspace.run(&["why", "baselib"]).trim(),
            "app > upperlib 1.0.0 > baselib 0.1.0"
        );
        assert!(
            workspace
                .run(&["why", "devcheck"])
                .contains("(direct dependency)")
        );
        let error = failure(workspace.command(&["why", "baselb"]).output().unwrap());
        assert!(error.contains("did you mean 'baselib'?"), "{error}");
        let licenses: serde_json::Value =
            serde_json::from_str(&workspace.run(&["licenses", "--json"])).unwrap();
        assert_eq!(
            licenses["MIT"],
            serde_json::json!(["baselib 0.1.0", "upperlib 1.0.0"])
        );
        assert_eq!(
            licenses["BSD-3-Clause"],
            serde_json::json!(["upperlib 1.0.0"])
        );
        assert_eq!(licenses["BSL-1.0"], serde_json::json!(["devcheck 3.1.0"]));

        let outdated: serde_json::Value =
            serde_json::from_str(&workspace.run(&["outdated", "--json"])).unwrap();
        assert_eq!(
            outdated,
            serde_json::json!([{"name": "upperlib", "current": "1.0.0", "wanted": "1.2.0", "latest": "2.0.0", "dev": false}])
        );
        assert!(
            workspace
                .run(&["outdated", "--offline"])
                .contains("upperlib  1.0.0    1.2.0   2.0.0")
        );
        // A plain upgrade never crosses a major version.
        assert!(
            workspace
                .run(&["upgrade"])
                .contains("Upgraded upperlib 1.0.0 -> 1.2.0")
        );
        assert!(workspace.run(&["upgrade"]).contains("Nothing to upgrade"));
        assert!(
            workspace
                .run(&["up", "upperlib", "--latest"])
                .contains("1.2.0 -> 2.0.0")
        );
        assert!(
            failure(workspace.command(&["upgrade", "uperlib"]).output().unwrap())
                .contains("did you mean 'upperlib'?")
        );
        let manifest = fs::read_to_string(root.join("cppm.toml")).unwrap();
        assert!(
            manifest.contains("[dependencies]  # reviewed monthly\nupperlib = \"2.0.0\""),
            "{manifest}"
        );
        assert!(workspace.run(&["outdated"]).contains("up to date"));
        assert!(
            workspace
                .run(&["list", "--all"])
                .contains("upperlib 2.0.0\n    └── baselib")
        );

        // ci installs the locked graph and refuses a manifest that moved on.
        let lock = fs::read(root.join("cppm.lock")).unwrap();
        assert!(workspace.run(&["ci"]).contains("cppm.lock unchanged"));
        assert_eq!(lock, fs::read(root.join("cppm.lock")).unwrap());
        fs::write(root.join("cppm.toml"), manifest.replace("2.0.0", "1.2.0")).unwrap();
        assert!(failure(workspace.command(&["ci"]).output().unwrap()).contains("disagree"));
        assert_eq!(lock, fs::read(root.join("cppm.lock")).unwrap());
        fs::write(root.join("cppm.toml"), &manifest).unwrap();

        // Moving a package between sections keeps exactly one declaration.
        workspace.run(&["add", "-D", "upperlib@2.0.0"]);
        let manifest = fs::read_to_string(root.join("cppm.toml")).unwrap();
        assert_eq!(manifest.matches("upperlib").count(), 1, "{manifest}");
        workspace.run(&["remove", "upperlib"]);
        workspace.run(&["remove", "devcheck"]);
        let manifest = fs::read_to_string(root.join("cppm.toml")).unwrap();
        assert!(!manifest.contains("dev-dependencies"), "{manifest}");
        assert!(manifest.contains("# Runtime libraries\n[dependencies]  # reviewed monthly\n"));
    }

    #[test]
    #[ignore = "requires Conan 2, CMake, Ninja and a C++ compiler; no network needed"]
    fn unchanged_builds_do_not_start_conan_and_every_input_change_is_noticed() {
        let workspace = workspace();
        let root = workspace.project();
        workspace.run(&["add", "upperlib@1.0.0"]);
        fs::write(
            root.join("src/main.cpp"),
            "#include <upperlib.h>\n#include <iostream>\n#ifndef PROFILE_VALUE\n#define PROFILE_VALUE 0\n#endif\nint main() { std::cout << \"profile=\" << PROFILE_VALUE + upper_value() - 43 << '\\n'; }\n",
        )
        .unwrap();
        let reused = |log: &str| log.contains("Reusing installed Conan dependencies");
        // `add` already validated this installation: nothing asks Conan again,
        // not even the store lookup made for the execution lease.
        let before = workspace.calls();
        for args in [
            &["exec", "--offline", "--verbose"][..],
            &["build", "--verbose"],
            &["test", "--frozen-lockfile", "--verbose"],
        ] {
            let output = workspace.command(args).output().unwrap();
            let log = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success() && reused(&log), "{args:?}\n{log}");
        }
        assert_eq!(
            workspace.calls(),
            before,
            "an unchanged build started Conan"
        );

        let verbose = |args: &[&str]| {
            let output = workspace.command(args).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        };
        // Missing generated files, an explicit refresh and `install` reinstall.
        fs::remove_file(root.join(".cppm/conan/debug/conan_toolchain.cmake")).unwrap();
        assert!(!reused(&verbose(&["build", "--verbose"])));
        assert!(
            root.join(".cppm/conan/debug/conan_toolchain.cmake")
                .exists()
        );
        assert!(!reused(&verbose(&["build", "--fresh", "--verbose"])));
        assert!(!reused(&verbose(&["install", "--verbose"])));
        let before = workspace.calls();
        assert!(reused(&verbose(&["build", "--verbose"])));
        assert_eq!(workspace.calls(), before);

        // An edit two includes away from the selected profile is noticed, then
        // the new installation is reused without Conan again.
        let base = root.join("base-profile");
        let profile = root.join("custom-profile");
        fs::write(&profile, "include(./base-profile)\n").unwrap();
        let profile = profile.to_str().unwrap();
        for value in [5, 8] {
            fs::write(
                &base,
                format!(
                    "include(default)\n[conf]\ntools.build:cxxflags=['-DPROFILE_VALUE={value}']\n"
                ),
            )
            .unwrap();
            let log = verbose(&["--profile", profile, "exec", "--verbose"]);
            assert!(
                log.contains(&format!("profile={value}")) && !reused(&log),
                "{log}"
            );
            let before = workspace.calls();
            let log = verbose(&["--profile", profile, "exec", "--verbose"]);
            assert!(
                log.contains(&format!("profile={value}")) && reused(&log),
                "{log}"
            );
            assert_eq!(workspace.calls(), before);
        }
        // A Conan setting changed behind cppm's back invalidates the reuse.
        fs::write(
            workspace.store().join("global.conf"),
            "tools.build:jobs=1\n",
        )
        .unwrap();
        let before = workspace.calls();
        assert!(!reused(&verbose(&[
            "--profile",
            profile,
            "build",
            "--verbose"
        ])));
        assert!(workspace.calls() > before);

        // A templated profile can read anything, so Conan resolves it on every
        // build; its answer still allows reusing the installation.
        fs::write(
            &base,
            "include(default)\n[conf]\ntools.build:cxxflags=['-DPROFILE_VALUE={{ 3 * 3 }}']\n",
        )
        .unwrap();
        assert!(verbose(&["--profile", profile, "exec", "--verbose"]).contains("profile=9"));
        let before = workspace.calls();
        let log = verbose(&["--profile", profile, "exec", "--verbose"]);
        assert!(log.contains("profile=9") && reused(&log), "{log}");
        assert!(
            workspace.calls() > before,
            "a templated profile was not re-resolved"
        );

        // Replacing the Conan launcher (an upgrade) is noticed as well.
        verbose(&["build", "--verbose"]);
        let before = workspace.calls();
        assert!(reused(&verbose(&["build", "--verbose"])));
        assert_eq!(workspace.calls(), before);
        let shim = workspace.temp.path().join("tools/conan");
        let script = fs::read_to_string(&shim).unwrap();
        fs::write(&shim, format!("{script}# upgraded\n")).unwrap();
        assert!(reused(&verbose(&["build", "--verbose"])));
        assert!(
            workspace.calls() > before,
            "a replaced Conan was not consulted"
        );
    }
}

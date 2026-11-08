use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn cppm(cwd: &Path, store: &Path, log: &Path, args: &[&str]) -> String {
    success(
        Command::new(env!("CARGO_BIN_EXE_cppm"))
            .current_dir(cwd)
            .env_remove("CPPM_PROFILE")
            .env_remove("CPPM_OFFLINE")
            .env_remove("CPPM_FROZEN_LOCKFILE")
            .env_remove("CPPM_SCRIPT_STACK")
            .env_remove("CPPM_STORE_DIR")
            // An explicit store must override an inherited Conan home.
            .env("CONAN_HOME", cwd.join("unused-cache"))
            .env("CPPM_STORE_TEST_BUILD_LOG", log)
            .arg("--store-dir")
            .arg(store)
            .args(args)
            .output()
            .unwrap(),
    )
}

#[test]
#[ignore = "requires Conan 2, CMake, Ninja and a C++ compiler; no network needed"]
fn independent_projects_share_one_binary_and_keep_working_after_pruning() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("shared store");
    let log = temp.path().join("build-count.txt");
    let fixture = temp.path().join("fixture");
    fs::create_dir(&fixture).unwrap();
    fs::write(
        fixture.join("conanfile.py"),
        r#"import os
from conan import ConanFile
from conan.tools.cmake import CMake, CMakeToolchain, cmake_layout

class StoreFixture(ConanFile):
    name = "cppm-store-fixture"
    version = "0.1.0"
    package_type = "static-library"
    settings = "os", "compiler", "arch", "build_type"
    exports_sources = "CMakeLists.txt", "value.cpp", "value.h"

    def layout(self):
        cmake_layout(self)

    def generate(self):
        CMakeToolchain(self).generate()

    def build(self):
        with open(os.environ["CPPM_STORE_TEST_BUILD_LOG"], "a") as log:
            log.write(self.build_folder + "\n")
        cmake = CMake(self)
        cmake.configure()
        cmake.build()

    def package(self):
        CMake(self).install()

    def package_info(self):
        self.cpp_info.libs = ["cppm_store_fixture"]
        self.cpp_info.set_property("cmake_file_name", "CppmStoreFixture")
        self.cpp_info.set_property("cmake_target_name", "CppmStoreFixture::CppmStoreFixture")
"#,
    )
    .unwrap();
    fs::write(fixture.join("CMakeLists.txt"), "cmake_minimum_required(VERSION 3.24)\nproject(fixture LANGUAGES CXX)\nadd_library(cppm_store_fixture STATIC value.cpp)\ninstall(TARGETS cppm_store_fixture ARCHIVE DESTINATION lib)\ninstall(FILES value.h DESTINATION include)\n").unwrap();
    fs::write(fixture.join("value.h"), "int cppm_store_value();\n").unwrap();
    fs::write(
        fixture.join("value.cpp"),
        "int cppm_store_value() { return 42; }\n",
    )
    .unwrap();
    success(
        Command::new("conan")
            .env("CONAN_HOME", &store)
            .args(["export"])
            .arg(&fixture)
            .output()
            .unwrap(),
    );
    // Both installs run in normal mode with all remotes disabled. The second
    // project resolves a versionless recipe entirely from the shared store.
    success(
        Command::new("conan")
            .env("CONAN_HOME", &store)
            .args(["remote", "disable", "*"])
            .output()
            .unwrap(),
    );
    let projects = [temp.path().join("one"), temp.path().join("two")];
    for project in &projects {
        fs::create_dir(project).unwrap();
        cppm(project, &store, &log, &["init", "--name", "store-check"]);
        cppm(project, &store, &log, &["install", "cppm-store-fixture"]);
        fs::write(
            project.join("src/main.cpp"),
            "#include <value.h>\n#include <iostream>\nint main() { std::cout << cppm_store_value() << '\\n'; return cppm_store_value() == 42 ? 0 : 1; }\n",
        )
        .unwrap();
        assert!(
            cppm(
                project,
                &store,
                &log,
                &["exec", "--offline", "--frozen-lockfile"]
            )
            .lines()
            .any(|line| line == "42")
        );
        assert!(!project.join("unused-cache").exists());
    }
    let builds = fs::read_to_string(&log).unwrap();
    assert_eq!(
        builds.lines().count(),
        1,
        "library was rebuilt for a second project"
    );
    assert!(
        !Path::new(builds.trim()).exists(),
        "successful installs should clean their temporary build directory"
    );
    let status: serde_json::Value = serde_json::from_str(&cppm(
        temp.path(),
        &store,
        &log,
        &["store", "status", "--json"],
    ))
    .unwrap();
    assert_eq!(status["recipes"], 1);
    assert_eq!(status["binaries"], 1);
    assert!(status["home_bytes"].as_u64().unwrap() > 0);

    // Both CMake integrations point to the very same library directory. There
    // is no copied or hard-linked dependency tree inside either project.
    let package_directory = |project: &Path| {
        let generated = project.join(".cppm/conan/debug");
        let file = fs::read_dir(generated)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with("-data.cmake")
            })
            .unwrap();
        fs::read_to_string(file)
            .unwrap()
            .lines()
            .find(|line| line.contains("PACKAGE_FOLDER_DEBUG"))
            .unwrap()
            .to_owned()
    };
    assert_eq!(
        package_directory(&projects[0]),
        package_directory(&projects[1])
    );
    let locks: Vec<_> = projects
        .iter()
        .map(|project| fs::read(project.join("cppm.lock")).unwrap())
        .collect();
    for args in [vec!["store", "prune"], vec!["store", "prune", "--sources"]] {
        cppm(temp.path(), &store, &log, &args);
        for (project, lock) in projects.iter().zip(&locks) {
            cppm(
                project,
                &store,
                &log,
                &["test", "--offline", "--frozen-lockfile"],
            );
            assert_eq!(*lock, fs::read(project.join("cppm.lock")).unwrap());
        }
    }
    assert_eq!(fs::read_to_string(&log).unwrap(), builds);
    // A relative CLI store remains absolute in a nested script, even when the
    // outer command discovers the project from a subdirectory.
    assert!(
        cppm(
            &projects[0].join("src"),
            Path::new("../../shared store"),
            &log,
            &["run", "dev", "--offline", "--frozen-lockfile"]
        )
        .lines()
        .any(|line| line == "42")
    );

    let package_path = |project: &Path, mode: &str| {
        let generated = project.join(format!(".cppm/conan/{mode}"));
        let data = fs::read_dir(generated)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with("-data.cmake")
            })
            .unwrap();
        let data = fs::read_to_string(data).unwrap();
        let line = data
            .lines()
            .find(|line| line.contains(&format!("PACKAGE_FOLDER_{}", mode.to_uppercase())))
            .unwrap();
        std::path::PathBuf::from(line.split('"').nth(1).unwrap())
    };
    let old_debug = package_path(&projects[0], "debug");
    cppm(
        &projects[0],
        &store,
        &log,
        &["build", "--release", "--offline", "--frozen-lockfile"],
    );
    let old_release = package_path(&projects[0], "release");
    assert_ne!(old_debug, old_release);
    assert!(
        same_file::is_same_file(
            old_debug.join("include/value.h"),
            old_release.join("include/value.h")
        )
        .unwrap()
    );

    // A second version shares its unchanged header with both configurations.
    let recipe = fixture.join("conanfile.py");
    fs::write(
        &recipe,
        fs::read_to_string(&recipe)
            .unwrap()
            .replace("version = \"0.1.0\"", "version = \"0.2.0\""),
    )
    .unwrap();
    success(
        Command::new("conan")
            .env("CONAN_HOME", &store)
            .arg("export")
            .arg(&fixture)
            .output()
            .unwrap(),
    );
    cppm(
        &projects[0],
        &store,
        &log,
        &["install", "cppm-store-fixture@0.2.0", "--offline"],
    );
    let new_debug = package_path(&projects[0], "debug");
    assert!(
        same_file::is_same_file(
            old_debug.join("include/value.h"),
            new_debug.join("include/value.h")
        )
        .unwrap()
    );
    let status: serde_json::Value = serde_json::from_str(&cppm(
        temp.path(),
        &store,
        &log,
        &["store", "status", "--json"],
    ))
    .unwrap();
    assert_eq!(status["projects"], 2);
    assert!(status["deduplicated_bytes"].as_u64().unwrap() > 0);
    assert!(status["unique_bytes"].as_u64().unwrap() < status["logical_bytes"].as_u64().unwrap());
    success(
        Command::new("conan")
            .env("CONAN_HOME", &store)
            .args(["cache", "check-integrity", "*"])
            .output()
            .unwrap(),
    );

    // Previewing cannot change the registry or any package. Only the obsolete
    // Release configuration is orphaned; another project still pins old Debug.
    let index = store.join("cppm-store/index.json");
    let before = fs::read(&index).unwrap();
    assert!(
        cppm(temp.path(), &store, &log, &["store", "prune", "--dry-run"])
            .contains("Would remove 1 unused binaries")
    );
    assert_eq!(before, fs::read(&index).unwrap());
    assert!(old_release.is_dir());
    cppm(temp.path(), &store, &log, &["store", "prune"]);
    assert!(!old_release.exists());
    assert!(old_debug.is_dir());
    for project in &projects {
        cppm(
            project,
            &store,
            &log,
            &["exec", "--offline", "--frozen-lockfile"],
        );
    }
    cppm(
        &projects[1],
        &store,
        &log,
        &["remove", "cppm-store-fixture", "--offline"],
    );
    cppm(temp.path(), &store, &log, &["store", "prune"]);
    assert!(!old_debug.exists());
    assert_eq!(
        fs::read_to_string(new_debug.join("include/value.h")).unwrap(),
        "int cppm_store_value();\n"
    );
    cppm(
        &projects[0],
        &store,
        &log,
        &["exec", "--offline", "--frozen-lockfile"],
    );

    // Missing projects stop pinning packages. Unregistered Conan recipes are
    // outside cppm's ownership and must survive its garbage collection.
    fs::write(
        &recipe,
        fs::read_to_string(&recipe)
            .unwrap()
            .replace("version = \"0.2.0\"", "version = \"0.3.0\""),
    )
    .unwrap();
    success(
        Command::new("conan")
            .env("CONAN_HOME", &store)
            .arg("export")
            .arg(&fixture)
            .output()
            .unwrap(),
    );
    fs::rename(&projects[0], temp.path().join("moved-project")).unwrap();
    cppm(temp.path(), &store, &log, &["store", "prune"]);
    assert!(!new_debug.exists());
    let status: serde_json::Value = serde_json::from_str(&cppm(
        temp.path(),
        &store,
        &log,
        &["store", "status", "--json"],
    ))
    .unwrap();
    assert_eq!(status["managed_binaries"], 0);
    assert_eq!(status["projects"], 0);
    assert_eq!(status["recipes"], 1);
    assert!(
        fs::read_dir(store.join("cppm-store/files"))
            .unwrap()
            .all(|shard| fs::read_dir(shard.unwrap().path())
                .unwrap()
                .next()
                .is_none())
    );
}

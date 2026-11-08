use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cppm(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires Conan 2, CMake, Ninja and a C++ compiler; builds a local shared-library fixture"]
fn shared_libraries_and_recipe_environment_work_in_debug_release_and_ctest() {
    let temporary = tempfile::Builder::new()
        .prefix("cppm runtime á & space ")
        .tempdir()
        .unwrap();
    let fixture = temporary.path().join("fixture");
    fs::create_dir(&fixture).unwrap();
    fs::write(
        fixture.join("conanfile.py"),
        r#"from conan import ConanFile
from conan.tools.cmake import CMake, CMakeToolchain, cmake_layout

class RuntimeFixture(ConanFile):
    name = "cppm-runtime-fixture"
    version = "0.1.0"
    package_type = "shared-library"
    settings = "os", "compiler", "arch", "build_type"
    exports_sources = "CMakeLists.txt", "value.cpp", "value.h"

    def layout(self):
        cmake_layout(self)

    def generate(self):
        CMakeToolchain(self).generate()

    def build(self):
        cmake = CMake(self)
        cmake.configure()
        cmake.build()

    def package(self):
        CMake(self).install()

    def package_info(self):
        self.cpp_info.libs = ["cppm_fixture"]
        self.cpp_info.set_property("cmake_file_name", "CppmFixture")
        self.cpp_info.set_property("cmake_target_name", "CppmFixture::CppmFixture")
        self.runenv_info.define("CPPM_FIXTURE_MESSAGE", "literal & spaces / á")
"#,
    )
    .unwrap();
    fs::write(fixture.join("CMakeLists.txt"), "cmake_minimum_required(VERSION 3.24)\nproject(fixture LANGUAGES CXX)\nadd_library(cppm_fixture SHARED value.cpp)\ninstall(TARGETS cppm_fixture RUNTIME DESTINATION bin LIBRARY DESTINATION lib ARCHIVE DESTINATION lib)\ninstall(FILES value.h DESTINATION include)\n").unwrap();
    fs::write(
        fixture.join("value.h"),
        "#pragma once\nint cppm_fixture_value();\n",
    )
    .unwrap();
    fs::write(fixture.join("value.cpp"), "#include \"value.h\"\n#ifdef _WIN32\n__declspec(dllexport)\n#endif\nint cppm_fixture_value() { return 42; }\n").unwrap();
    success(
        Command::new("conan")
            .arg("export")
            .arg(&fixture)
            .output()
            .unwrap(),
    );
    let root = temporary.path().join("project");
    fs::create_dir(&root).unwrap();
    success(cppm(&root, &["init", "--name", "runtime-check"]));
    success(cppm(
        &root,
        &["install", "--offline", "cppm-runtime-fixture@0.1.0"],
    ));
    let cmake = fs::read_to_string(root.join("CMakeLists.txt")).unwrap();
    fs::write(root.join("CMakeLists.txt"), format!("{cmake}\nset_target_properties(${{PROJECT_NAME}} PROPERTIES SKIP_BUILD_RPATH TRUE INSTALL_RPATH \"\")\n")).unwrap();
    fs::write(
        root.join("src/main.cpp"),
        r#"#include <value.h>
#include <cstdlib>
#include <cstring>
#include <iostream>
#ifdef _WIN32
#include <windows.h>
#include <cwchar>
#endif
int main() {
#ifdef _WIN32
    wchar_t message[128];
    if (!GetEnvironmentVariableW(L"CPPM_FIXTURE_MESSAGE", message, 128) ||
        std::wcscmp(message, L"literal & spaces / \u00e1") != 0) return 2;
#else
    const char* message = std::getenv("CPPM_FIXTURE_MESSAGE");
    if (!message || std::strcmp(message, "literal & spaces / \xc3\xa1") != 0) return 2;
#endif
    if (cppm_fixture_value() != 42) return 3;
    std::cout << "runtime: 42\n";
}
"#,
    )
    .unwrap();
    for release in [false, true] {
        let mut args = vec!["exec", "--offline", "--frozen-lockfile"];
        if release {
            args.push("--release");
        }
        assert!(success(cppm(&root, &args)).contains("runtime: 42"));
        args[0] = "test";
        success(cppm(&root, &args));
    }
    let mut protected = vec!["exec", "obf", "--offline", "--frozen-lockfile"];
    // The portable CI path needs no LLVM SDK. Enable the managed plugin when
    // exercising its real Conan compiler/profile/runtime integration locally.
    if std::env::var_os("CPPM_TEST_BUNDLED_OBF").is_none() {
        protected.push("--obf-basic");
    }
    assert!(success(cppm(&root, &protected)).contains("runtime: 42"));
    protected[0] = "test";
    success(cppm(&root, &protected));
    if std::env::var_os("CPPM_TEST_BUNDLED_OBF").is_some() {
        let sdk = fs::read_dir(root.join(".cppm/tools/obfuscator"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let profile = sdk.join("clang-profile");
        assert!(
            success(cppm(
                &root,
                &[
                    "exec",
                    "obf",
                    "--offline",
                    "--frozen-lockfile",
                    "--profile",
                    profile.to_str().unwrap()
                ]
            ))
            .contains("runtime: 42")
        );
        let incompatible = root.join("gcc-profile");
        fs::write(&incompatible, "include(default)\n[settings]\ncompiler=gcc\ncompiler.version=13\ncompiler.libcxx=libstdc++11\n").unwrap();
        let failure = cppm(
            &root,
            &[
                "build",
                "obf",
                "--offline",
                "--profile",
                incompatible.to_str().unwrap(),
            ],
        );
        assert!(!failure.status.success());
        assert!(String::from_utf8_lossy(&failure.stderr).contains("must select clang"));
    }
    // Conan settings and recipe metadata are local to the child processes.
    assert!(std::env::var_os("CPPM_FIXTURE_MESSAGE").is_none());
}

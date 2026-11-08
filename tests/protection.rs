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

fn project() -> TempDir {
    let temp = tempfile::Builder::new()
        .prefix("cppm protected space ")
        .tempdir()
        .unwrap();
    success(cppm(temp.path(), &["init", "--name", "protected-app"]));
    temp
}

#[test]
fn protected_build_preserves_behavior_hides_metadata_and_isolates_outputs() {
    let temp = project();
    let root = temp.path();
    let source = r#"#include <iostream>
#include <string>
#ifdef _MSC_VER
__declspec(noinline)
#else
__attribute__((noinline))
#endif
int proprietary_calculation(int value) { return value * 7 + 3; }
int main(int argc, char** argv) {
    if (argc > 1 && std::string(argv[1]) == "fail") return 23;
    std::cout << __FILE__ << '\n' << proprietary_calculation(argc) << '\n';
    if (argc > 1) std::cout << argv[1] << '\n';
}
"#;
    fs::write(root.join("src/main.cpp"), source).unwrap();
    let cmake = fs::read(root.join("CMakeLists.txt")).unwrap();
    let manifest = fs::read(root.join("cppm.toml")).unwrap();
    let log = success(cppm(
        &root.join("src"),
        &["build", "obf", "--obf-basic", "-j", "2", "--offline"],
    ));
    assert!(log.contains("No LLVM obfuscation plugin"));
    let binary = root.join(format!(
        ".cppm/build/obf/bin/protected-app{}",
        std::env::consts::EXE_SUFFIX
    ));
    let output = success(Command::new(&binary).output().unwrap());
    assert!(output.contains("10"));
    assert!(
        !output
            .replace('\\', "/")
            .contains(&root.to_string_lossy().replace('\\', "/"))
    );
    let database = fs::read_to_string(root.join(".cppm/build/obf/compile_commands.json")).unwrap();
    if cfg!(windows) && database.contains("/pathmap:") {
        assert!(database.contains("/GL") || database.contains("-flto"));
    } else {
        for flag in ["-g0", "-ffile-prefix-map=", "-fvisibility=hidden", "-flto"] {
            assert!(database.contains(flag), "missing {flag}");
        }
    }
    #[cfg(target_os = "linux")]
    {
        let symbols = Command::new("nm")
            .env("LC_ALL", "C")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&symbols.stderr).contains("no symbols"));
        let bytes = fs::read(&binary).unwrap();
        assert!(
            !bytes
                .windows(b"proprietary_calculation".len())
                .any(|part| part == b"proprietary_calculation")
        );
    }
    assert!(
        success(cppm(
            root,
            &["exec", "obf", "--obf-basic", "--", "argument with spaces"]
        ))
        .contains("argument with spaces")
    );
    assert_eq!(
        cppm(root, &["exec", "obf", "--obf-basic", "--", "fail"])
            .status
            .code(),
        Some(23)
    );
    assert!(success(cppm(root, &["test", "obf", "--obf-basic"])).contains("100% tests passed"));
    success(cppm(root, &["build", "--release"]));
    let ordinary = root.join(format!(
        ".cppm/build/release/bin/protected-app{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(ordinary.is_file());
    let database =
        fs::read_to_string(root.join(".cppm/build/release/compile_commands.json")).unwrap();
    assert!(!database.contains("-ffile-prefix-map="));
    assert!(!database.contains("/pathmap:"));
    assert_eq!(
        fs::read(root.join("src/main.cpp")).unwrap(),
        source.as_bytes()
    );
    assert_eq!(fs::read(root.join("CMakeLists.txt")).unwrap(), cmake);
    assert_eq!(fs::read(root.join("cppm.toml")).unwrap(), manifest);
}

#[test]
fn bad_plugin_and_wasm_fail_before_preparing_dependencies() {
    let temp = project();
    let root = temp.path();
    let output = cppm(root, &["build", "--obf-plugin", "missing.so"]);
    assert!(!output.status.success());
    assert!(!root.join("cppm.lock").exists());
    let output = cppm(root, &["build", "obf", "--obf-plugin", "missing.so"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("is not a file"));
    assert!(!root.join(".cppm/build/obf").exists());
    assert!(!root.join("cppm.lock").exists());
    let wasm = tempfile::tempdir().unwrap();
    success(cppm(
        wasm.path(),
        &["init", "--name", "web", "--template", "wasm"],
    ));
    let output = cppm(wasm.path(), &["build", "obf"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("WebAssembly protection is not supported")
    );
    assert!(!wasm.path().join("cppm.lock").exists());
}

#[test]
fn missing_llvm_sdk_does_not_silently_fall_back_to_basic() {
    let temp = project();
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(temp.path())
        .env("CPPM_LLVM_CONFIG", temp.path().join("missing-llvm-config"))
        .args(["build", "obf"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.contains(if cfg!(any(target_os = "linux", target_os = "macos")) {
            "matching LLVM development SDK"
        } else {
            "currently supports Linux/macOS hosts"
        })
    );
    assert!(!temp.path().join("cppm.lock").exists());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires matching Clang and LLVM 18–23 development SDK"]
fn bundled_plugin_bootstraps_offline_hides_literals_and_preserves_cpp_semantics() {
    let temp = project();
    let root = temp.path();
    fs::write(
        root.join("src/shared.h"),
        r#"#pragma once
#include <string>
inline const char shared_literal[] = "shared-odr-value";
const std::string& startup_value();
"#,
    )
    .unwrap();
    fs::write(
        root.join("src/second.cpp"),
        r#"#include "shared.h"
static std::string startup("cppm-startup-sensitive-marker-b7451970");
const std::string& startup_value() { return startup; }
"#,
    )
    .unwrap();
    fs::write(root.join("src/main.cpp"), r#"#include "shared.h"
#include <cstdio>
#include <cstdlib>
#include <stdexcept>
#include <thread>
__attribute__((noinline)) int calculate(int x) {
    int result = 0;
    for (int i = 0; i < x; ++i) result = (result * 3 + i) % 101;
    if (x < 10) return result + 7;
    if (x < 20) return result - 13;
    return result ^ 42;
}
int main(int argc, char** argv) {
    int input = argc > 1 ? std::atoi(argv[1]) : 3;
    if (input < 0) { try { throw std::runtime_error("expected-exception"); } catch(const std::exception&) { return 23; } }
    std::thread thread([] { if (std::string(shared_literal) != "shared-odr-value") std::abort(); });
    thread.join();
    std::puts("cppm-bundled-sensitive-marker-890a31f4");
    std::puts(startup_value().c_str());
    std::printf("%d\n", calculate(input));
}
"#).unwrap();
    success(cppm(root, &["build", "--release", "--offline"]));
    let ordinary = root.join(".cppm/build/release/bin/protected-app");
    let first = success(cppm(root, &["build", "obf", "--offline"]));
    assert!(first.contains("Preparing bundled Obfuscator-LLVM"));
    assert!(first.contains("Bundled Obfuscator-LLVM"));
    let binary = root.join(".cppm/build/obf/bin/protected-app");
    for input in ["0", "1", "5", "11", "19", "20", "50", "-1"] {
        let expected = Command::new(&ordinary).arg(input).output().unwrap();
        let actual = Command::new(&binary).arg(input).output().unwrap();
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "input {input}"
        );
        assert_eq!(actual.stdout, expected.stdout, "input {input}");
    }
    let bytes = fs::read(&binary).unwrap();
    let marker = b"cppm-bundled-sensitive-marker-890a31f4";
    assert!(!bytes.windows(marker.len()).any(|part| part == marker));
    let second = success(cppm(root, &["test", "obf", "--offline"]));
    assert!(second.contains("100% tests passed"));
    assert!(!second.contains("Preparing bundled Obfuscator-LLVM"));
    let plugins = fs::read_dir(root.join(".cppm/tools/obfuscator"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(plugins.len(), 1);
    let plugin = plugins[0].join("build/lib/CppmObfuscator.so");
    let ir = root.join("transformed.ll");
    success(
        Command::new("clang++")
            .args(["-O3", "-S", "-emit-llvm"])
            .arg(format!("-fpass-plugin={}", plugin.display()))
            .env(
                "LLVM_OBF_SCALAROPTIMIZERLATE_PASSES",
                "flattening,substitution,split-basic-blocks",
            )
            .env("LLVM_OBF_OPTIMIZERLASTEP_PASSES", "string-encryption")
            .arg(root.join("src/main.cpp"))
            .arg("-o")
            .arg(&ir)
            .output()
            .unwrap(),
    );
    assert!(fs::read_to_string(ir).unwrap().contains("switch i32"));
    // A corrupt cached plugin is rebuilt instead of being passed to Clang.
    fs::write(&plugin, b"corrupted").unwrap();
    assert!(
        success(cppm(root, &["build", "obf", "--offline"]))
            .contains("Preparing bundled Obfuscator-LLVM")
    );
}

#[test]
fn disabling_the_cmake_hook_cannot_silently_skip_protection() {
    let temp = project();
    let root = temp.path();
    let cmake = fs::read_to_string(root.join("CMakeLists.txt")).unwrap();
    fs::write(
        root.join("CMakeLists.txt"),
        cmake.replace("project(", "set(CMAKE_PROJECT_INCLUDE \"\")\nproject("),
    )
    .unwrap();
    let output = cppm(root, &["build", "obf", "--obf-basic"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not apply the obf preset"));
    assert!(
        !root
            .join(format!(
                ".cppm/build/obf/bin/protected-app{}",
                std::env::consts::EXE_SUFFIX
            ))
            .exists()
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Clang, matching LLVM development headers/libraries and LTO linker support"]
fn llvm_plugin_transforms_real_code_and_changed_plugins_invalidate_objects() {
    let temp = project();
    let root = temp.path();
    let plugin_source = root.join("pass fixture.cpp");
    fs::write(
        &plugin_source,
        include_str!("fixtures/protection-plugin.cpp"),
    )
    .unwrap();
    let plugin = root.join("pass fixture.so");
    let flags = success(
        Command::new("llvm-config")
            .args([
                "--cxxflags",
                "--ldflags",
                "--libs",
                "core",
                "passes",
                "--system-libs",
            ])
            .output()
            .unwrap(),
    );
    let compile_plugin = |replacement: &str| {
        success(
            Command::new("clang++")
                .args(flags.split_whitespace())
                .args(["-shared", "-fPIC", "-O2"])
                .arg(format!("-DFIXTURE_REPLACEMENT=\"{replacement}\""))
                .arg(&plugin_source)
                .arg("-o")
                .arg(&plugin)
                .output()
                .unwrap(),
        );
    };
    fs::write(
        root.join("src/main.cpp"),
        "#include <iostream>\nint main() { std::cout << \"plugin-before\" << '\\n'; }\n",
    )
    .unwrap();
    let build = || {
        success(
            Command::new(env!("CARGO_BIN_EXE_cppm"))
                .current_dir(root)
                .env("CXX", "clang++")
                .env("CC", "clang")
                .env_remove("CPPM_FROZEN_LOCKFILE")
                .env_remove("CPPM_SCRIPT_STACK")
                .args([
                    "build",
                    "obf",
                    "--offline",
                    "--obf-plugin",
                    "pass fixture.so",
                ])
                .output()
                .unwrap(),
        )
    };
    compile_plugin("plugin-after!");
    assert!(build().contains("LLVM pass plugin:"));
    let binary = root.join(".cppm/build/obf/bin/protected-app");
    assert_eq!(
        success(Command::new(&binary).output().unwrap()),
        "plugin-after!\n"
    );
    compile_plugin("plugin-again!");
    build();
    assert_eq!(
        success(Command::new(&binary).output().unwrap()),
        "plugin-again!\n"
    );
    // Changing back to the builtin preset must drop the plugin flags and output.
    success(
        Command::new(env!("CARGO_BIN_EXE_cppm"))
            .current_dir(root)
            .env("CXX", "clang++")
            .env("CC", "clang")
            .env_remove("CPPM_FROZEN_LOCKFILE")
            .args(["build", "obf", "--obf-basic", "--offline"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        success(Command::new(&binary).output().unwrap()),
        "plugin-before\n"
    );
    // A compiler that cannot load LLVM plugins must fail clearly.
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .env("CXX", "g++")
        .env("CC", "gcc")
        .env_remove("CPPM_FROZEN_LOCKFILE")
        .args([
            "build",
            "obf",
            "--offline",
            "--obf-plugin",
            "pass fixture.so",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires Clang"));
}

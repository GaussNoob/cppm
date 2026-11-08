use crate::{emscripten, manifest::Target, platform, project::Project};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::{ffi::OsStr, fs, path::Path, process::Command};

#[derive(Serialize)]
struct Check {
    name: &'static str,
    status: &'static str,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'static str>,
}

#[derive(Serialize)]
struct Report {
    format: u32,
    cppm_version: &'static str,
    platform: &'static str,
    architecture: &'static str,
    cpp_standard: u8,
    ok: bool,
    checks: Vec<Check>,
}

pub fn run(cwd: &Path, json: bool) -> Result<()> {
    let project = Project::discover(cwd).ok();
    let standard = project
        .as_ref()
        .map_or(20, |project| project.manifest.package.cpp_standard);
    let wasm = project.is_some_and(|project| project.manifest.package.target == Target::Wasm);
    let mut checks = vec![
        tool(
            "cmake",
            "cmake",
            cwd,
            (3, 24),
            None,
            "Install CMake 3.24+ and add it to PATH.",
        ),
        tool(
            "ninja",
            "ninja",
            cwd,
            (1, 0),
            None,
            "Install Ninja and add it to PATH.",
        ),
    ];
    if wasm {
        checks.push(check(
            "Emscripten",
            emscripten::check(cwd),
            emscripten::SETUP_HINT,
        ));
        checks.push(tool(
            "Node.js",
            emscripten::node(),
            cwd,
            (0, 0),
            None,
            "Activate the SDK's Node.js with emsdk_env, or install Node.js and add it to PATH. Check EMSDK_NODE if it is set.",
        ));
        checks.push(tool(
            "Python 3",
            emscripten::python(),
            cwd,
            (3, 0),
            Some(3),
            "Install Python 3 and add it to PATH; `cppm run dev` uses it to serve WebAssembly.",
        ));
    } else {
        checks.push(tool(
            "conan",
            "conan",
            cwd,
            (2, 34),
            Some(2),
            "Install Conan 2.34+ with `uv tool install 'conan>=2.34,<3'` or pipx.",
        ));
    }
    let compiler_name = if wasm { "WebAssembly" } else { "C++" };
    let hint = if wasm {
        emscripten::SETUP_HINT
    } else {
        compiler_hint()
    };
    let prerequisites = if wasm { 4 } else { 2 };
    let compiler = if checks[..prerequisites]
        .iter()
        .all(|check| check.status == "ok")
    {
        match compiler(standard, wasm) {
            Ok(detail) => Check {
                name: compiler_name,
                status: "ok",
                detail,
                hint: None,
            },
            Err(error) => Check {
                name: compiler_name,
                status: "error",
                detail: format!("{error:#}"),
                hint: Some(hint),
            },
        }
    } else {
        Check {
            name: compiler_name,
            status: "skipped",
            detail: if wasm {
                "WebAssembly probe needs working CMake, Ninja, Emscripten and Node.js."
            } else {
                "Compiler probe needs working CMake and Ninja."
            }
            .into(),
            hint: Some(hint),
        }
    };
    checks.push(compiler);
    let report = Report {
        format: 1,
        cppm_version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        cpp_standard: standard,
        ok: checks.iter().all(|check| check.status == "ok"),
        checks,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "cppm {} / {} {} / C++{}",
            report.cppm_version, report.platform, report.architecture, standard
        );
        for check in &report.checks {
            println!("[{}] {}: {}", check.status, check.name, check.detail);
            if let Some(hint) = check.hint {
                println!("  {hint}");
            }
        }
        if report.ok {
            if wasm {
                println!("Ready: Emscripten built a WebAssembly module and Node.js ran it.");
            } else {
                println!("Ready: the C++ compiler built and ran a test program.");
            }
        }
    }
    ensure!(report.ok, "environment checks failed; see the report above");
    Ok(())
}

fn compiler_hint() -> &'static str {
    if cfg!(windows) {
        "Install Visual Studio Build Tools with Desktop development with C++ (MSVC and Windows SDK). cppm detects it automatically; an existing Developer PowerShell or CC/CXX is respected."
    } else {
        "Install GCC or Clang with its C++ standard library. Check CC/CXX if you selected a custom compiler."
    }
}

fn tool(
    name: &'static str,
    program: impl AsRef<OsStr>,
    cwd: &Path,
    minimum: (u64, u64),
    major: Option<u64>,
    hint: &'static str,
) -> Check {
    let result = (|| -> Result<String> {
        let program = program.as_ref();
        let label = program.to_string_lossy();
        let output = platform::command(program)
            .arg("--version")
            .current_dir(cwd)
            .output()
            .with_context(|| format!("cannot start {label}"))?;
        ensure!(
            output.status.success(),
            "{label} --version failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8_lossy(&output.stdout);
        let version = parse_version(&text).context("cannot determine tool version")?;
        ensure!(
            (version.0, version.1) >= minimum && major.is_none_or(|major| version.0 == major),
            "unsupported version: {}",
            text.lines().next().unwrap_or_default()
        );
        Ok(text.lines().next().unwrap_or_default().to_owned())
    })();
    check(name, result, hint)
}

fn check(name: &'static str, result: Result<String>, hint: &'static str) -> Check {
    match result {
        Ok(detail) => Check {
            name,
            status: "ok",
            detail,
            hint: None,
        },
        Err(error) => Check {
            name,
            status: "error",
            detail: format!("{error:#}"),
            hint: Some(hint),
        },
    }
}

fn parse_version(text: &str) -> Option<(u64, u64)> {
    text.split_whitespace().find_map(|token| {
        let mut parts = token.trim_start_matches('v').split('.');
        Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
    })
}

fn capture(command: &mut Command) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("cannot start {}", command.get_program().to_string_lossy()))?;
    if !output.status.success() {
        let log = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let lines: Vec<_> = log.lines().collect();
        bail!(
            "{} failed ({}):\n{}",
            command.get_program().to_string_lossy(),
            output.status,
            lines[lines.len().saturating_sub(24)..].join("\n")
        );
    }
    Ok(())
}

fn compiler(standard: u8, wasm: bool) -> Result<String> {
    let temporary = tempfile::Builder::new().prefix("cppm doctor ").tempdir()?;
    let root = temporary.path();
    fs::write(
        root.join("CMakeLists.txt"),
        format!(
            r#"cmake_minimum_required(VERSION 3.24)
project(cppm_doctor LANGUAGES CXX)
add_executable(cppm_probe main.cpp)
set_target_properties(cppm_probe PROPERTIES CXX_STANDARD {standard} CXX_STANDARD_REQUIRED ON CXX_EXTENSIONS OFF)
if(EMSCRIPTEN)
    set_target_properties(cppm_probe PROPERTIES SUFFIX ".js")
    target_link_options(cppm_probe PRIVATE "-sENVIRONMENT=node")
endif()
file(WRITE "${{CMAKE_BINARY_DIR}}/compiler.txt" "${{CMAKE_CXX_COMPILER_ID}} ${{CMAKE_CXX_COMPILER_VERSION}} (${{CMAKE_CXX_COMPILER}})")
enable_testing()
add_test(NAME cppm_probe COMMAND cppm_probe)
"#
        ),
    )?;
    fs::write(
        root.join("main.cpp"),
        "#include <iostream>\n#include <string>\nint main() { const std::string value = \"cppm\"; std::cout << value; return value.size() == 4 ? 0 : 1; }\n",
    )?;
    let build = root.join("build");
    let mut configure = if wasm {
        emscripten::cmake()?
    } else {
        platform::build_command("cmake")?
    };
    capture(configure.arg("-S").arg(root).arg("-B").arg(&build).args([
        "-G",
        "Ninja",
        "-DCMAKE_BUILD_TYPE=Release",
    ]))?;
    let mut compile = if wasm {
        emscripten::command("cmake")?
    } else {
        platform::build_command("cmake")?
    };
    capture(compile.arg("--build").arg(&build).args(["--parallel", "2"]))?;
    if wasm {
        capture(emscripten::command("node")?.arg(build.join("cppm_probe.js")))?;
    } else {
        capture(
            platform::build_command("ctest")?
                .arg("--test-dir")
                .arg(&build)
                .args(["--output-on-failure", "--no-tests=error"]),
        )?;
    }
    let compiler = fs::read_to_string(build.join("compiler.txt"))?;
    Ok(if wasm {
        format!("{compiler}; compiled C++{standard} to WebAssembly and executed it with Node.js")
    } else {
        format!("{compiler}; compiled and executed C++{standard}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_checks_handle_real_tool_output() {
        assert_eq!(parse_version("cmake version 3.31.6"), Some((3, 31)));
        assert_eq!(parse_version("Conan version 2.34.0"), Some((2, 34)));
        assert_eq!(parse_version("1.13.2.git"), Some((1, 13)));
        assert_eq!(parse_version("not installed"), None);
    }
}

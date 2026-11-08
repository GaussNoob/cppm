use std::{fs, path::Path, process::Command};

#[test]
#[ignore = "requires Conan 2, a C++ toolchain and network access on first run"]
fn json_alias_serializes_and_parses() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "with-json"]);
    run(root, &["install", "json"]);
    assert!(
        fs::read_to_string(root.join("cppm.toml"))
            .unwrap()
            .contains("nlohmann_json =")
    );
    fs::write(
        root.join("src/main.cpp"),
        r#"#include <nlohmann/json.hpp>
int main() {
    nlohmann::json value = {{"answer", 42}};
    auto parsed = nlohmann::json::parse(value.dump());
    return parsed.at("answer").get<int>() == 42 ? 0 : 1;
}
"#,
    )
    .unwrap();
    run(root, &["exec", "--offline", "--frozen-lockfile"]);
}

fn run(root: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .env_remove("CPPM_SCRIPT_STACK")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "command {args:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires Conan 2, a C++ toolchain and network access on first run"]
fn fmt_installs_links_and_reuses_the_locked_cache_offline() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "with-fmt"]);
    run(root, &["install", "fmt"]);
    fs::write(
        root.join("src/main.cpp"),
        "#include <fmt/core.h>\nint main() { fmt::print(\"fmt works: {}\\n\", 42); }\n",
    )
    .unwrap();
    let locked = fs::read(root.join("cppm.lock")).unwrap();
    assert!(run(root, &["exec", "--offline", "--frozen-lockfile"]).contains("fmt works: 42"));
    assert_eq!(locked, fs::read(root.join("cppm.lock")).unwrap());
    run(root, &["build", "--release"]);
    assert!(
        run(
            root,
            &["exec", "--release", "--offline", "--frozen-lockfile"]
        )
        .contains("fmt works: 42")
    );
    run(root, &["test", "--offline", "--frozen-lockfile"]);
    // Removing the final dependency also clears its CMake toolchain on the next build.
    run(root, &["remove", "fmt"]);
    fs::write(root.join("src/main.cpp"), "int main() {}\n").unwrap();
    run(root, &["build", "--offline", "--frozen-lockfile"]);
}

#[test]
#[ignore = "requires Conan 2 and a C++ toolchain"]
fn batch_install_deduplicates_aliases_and_rolls_back_the_entire_failed_request() {
    let temp = tempfile::Builder::new()
        .prefix("cppm batch ")
        .tempdir()
        .unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "batch-check"]);
    let output = run(root, &["install", "fmt", "json", "nlohmann_json"]);
    assert_eq!(output.matches("Added ").count(), 2);
    let manifest = fs::read(root.join("cppm.toml")).unwrap();
    let parsed: toml::Value = toml::from_str(std::str::from_utf8(&manifest).unwrap()).unwrap();
    assert_eq!(parsed["dependencies"].as_table().unwrap().len(), 2);
    fs::write(root.join("src/main.cpp"), "#include <fmt/core.h>\n#include <nlohmann/json.hpp>\nint main() { nlohmann::json v = {{\"answer\", 42}}; fmt::print(\"batch: {}\\n\", v.at(\"answer\").get<int>()); }\n").unwrap();
    assert!(run(root, &["exec", "--offline", "--frozen-lockfile"]).contains("batch: 42"));
    let lock = fs::read(root.join("cppm.lock")).unwrap();
    let failed = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .args(["install", "--offline", "eigen", "fmt@0.0.0-does-not-exist"])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert_eq!(fs::read(root.join("cppm.toml")).unwrap(), manifest);
    assert_eq!(fs::read(root.join("cppm.lock")).unwrap(), lock);
    assert!(run(root, &["exec", "--offline", "--frozen-lockfile"]).contains("batch: 42"));
}

#[test]
#[ignore = "requires Conan 2, a C++ toolchain and network access on first run"]
fn sqlite_alias_installs_and_executes_a_real_query() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "with-sqlite"]);
    run(root, &["install", "sqlite"]);
    let manifest = fs::read_to_string(root.join("cppm.toml")).unwrap();
    assert!(manifest.contains("sqlite3 ="));
    fs::write(root.join("src/main.cpp"), r#"#include <sqlite3.h>
#include <iostream>
int main() {
    sqlite3* database = nullptr;
    if (sqlite3_open(":memory:", &database) != SQLITE_OK) return 1;
    sqlite3_stmt* statement = nullptr;
    if (sqlite3_prepare_v2(database, "SELECT 6 * 7", -1, &statement, nullptr) != SQLITE_OK) {
        sqlite3_close(database);
        return 2;
    }
    const bool valid = sqlite3_step(statement) == SQLITE_ROW && sqlite3_column_int(statement, 0) == 42;
    sqlite3_finalize(statement);
    sqlite3_close(database);
    if (!valid) return 3;
    std::cout << "sqlite works: 42\n";
}

"#).unwrap();
    assert!(run(root, &["exec", "--offline", "--frozen-lockfile"]).contains("sqlite works: 42"));
    run(root, &["install", "sqlite3"]);
    run(root, &["remove", "sqlite"]);
}

#[test]
#[ignore = "requires Conan 2, a C++ toolchain and network access on first run"]
fn package_outside_catalog_resolves_version_and_cmake_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "automatic-package"]);
    run(root, &["install", "utfcpp"]);
    let manifest = fs::read_to_string(root.join("cppm.toml")).unwrap();
    assert!(manifest.contains("utfcpp ="));
    assert!(!manifest.contains("[>=0]"));
    fs::write(root.join("src/main.cpp"), "#include <utf8.h>\nint main() { const char text[] = \"hello\"; return utf8::is_valid(text, text + 5) ? 0 : 1; }\n").unwrap();
    run(root, &["exec", "--offline", "--frozen-lockfile"]);
}

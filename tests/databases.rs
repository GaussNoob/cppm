use std::{fs, path::Path, process::Command};

fn run(root: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cppm"))
        .current_dir(root)
        .env_remove("CPPM_SCRIPT_STACK")
        .env_remove("CPPM_OFFLINE")
        .env_remove("CPPM_FROZEN_LOCKFILE")
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

fn check(alias: &str, canonical: &str, source: &str, alternate_alias: &str) {
    let temp = tempfile::Builder::new()
        .prefix("cppm database ")
        .tempdir()
        .unwrap();
    let root = temp.path();
    run(root, &["init", "--name", "database-check"]);
    run(root, &["install", alias]);
    assert!(
        fs::read_to_string(root.join("cppm.toml"))
            .unwrap()
            .contains(&format!("{canonical} ="))
    );
    fs::write(root.join("src/main.cpp"), source).unwrap();
    let locked = fs::read(root.join("cppm.lock")).unwrap();
    run(root, &["exec", "--offline", "--frozen-lockfile"]);
    run(root, &["test", "--offline", "--frozen-lockfile"]);
    assert_eq!(locked, fs::read(root.join("cppm.lock")).unwrap());
    // Installing another spelling must update the same direct dependency.
    run(root, &["install", alternate_alias, "--offline"]);
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("cppm.toml")).unwrap()).unwrap();
    assert_eq!(manifest["dependencies"].as_table().unwrap().len(), 1);
    run(root, &["remove", alternate_alias, "--offline"]);
    fs::write(root.join("src/main.cpp"), "int main() {}\n").unwrap();
    run(root, &["build", "--offline", "--frozen-lockfile"]);
}

#[test]
#[ignore = "requires Conan 2, a native C++ toolchain and network access on first run"]
fn mysql_and_mariadb_install_the_same_client_and_link() {
    check(
        "MySQL",
        "mariadb-connector-c",
        include_str!("../examples/databases/mysql.cpp"),
        "mariadb",
    );
}

#[test]
#[ignore = "requires Conan 2, a native C++ toolchain and network access on first run"]
fn postgres_alias_links_the_postgresql_client() {
    check(
        "postgres",
        "libpq",
        include_str!("../examples/databases/postgres.cpp"),
        "PostgreSQL",
    );
}

#[test]
#[ignore = "requires Conan 2, a native C++ toolchain and network access on first run"]
fn redis_alias_links_and_parses_a_real_protocol_reply() {
    check(
        "redis",
        "hiredis",
        include_str!("../examples/databases/redis.cpp"),
        "hiredis",
    );
}

#[test]
#[ignore = "requires Conan 2, a native C++ toolchain and network access on first run"]
fn mongodb_alias_links_the_client_and_encodes_bson() {
    check(
        "mongodb",
        "mongo-c-driver",
        include_str!("../examples/databases/mongodb.cpp"),
        "mongo",
    );
}

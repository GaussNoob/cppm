use crate::{
    conan::Graph,
    manifest::{Dependency, DependencyDetails, valid_name, validate_cmake_token, validate_version},
};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub struct PackageInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub cmake_package: &'static str,
    pub target: &'static str,
}

pub const ALIASES: &[(&str, &str)] = &[
    ("sqlite", "sqlite3"),
    ("mysql", "mariadb-connector-c"),
    ("mariadb", "mariadb-connector-c"),
    ("postgres", "libpq"),
    ("postgresql", "libpq"),
    ("postgress", "libpq"),
    ("redis", "hiredis"),
    ("mongo", "mongo-c-driver"),
    ("mongodb", "mongo-c-driver"),
    ("curl", "libcurl"),
    ("sdl2", "sdl"),
    ("sdl3", "sdl"),
    ("json", "nlohmann_json"),
    ("googletest", "gtest"),
    ("google-test", "gtest"),
    ("google_test", "gtest"),
    ("httplib", "cpp-httplib"),
    ("dear-imgui", "imgui"),
    ("dearimgui", "imgui"),
    ("qt6", "qt"),
    ("opencv4", "opencv"),
    ("protocol-buffers", "protobuf"),
    ("tbb", "onetbb"),
    ("one-tbb", "onetbb"),
];

// These are starter defaults, never an assertion that a version is the latest.
pub const CATALOG: &[PackageInfo] = &[
    PackageInfo {
        name: "sqlite3",
        version: "3.54.0",
        cmake_package: "SQLite3",
        target: "SQLite::SQLite3",
    },
    PackageInfo {
        name: "mariadb-connector-c",
        version: "3.4.8",
        cmake_package: "mariadb-connector-c",
        target: "mariadb-connector-c::mariadb-connector-c",
    },
    PackageInfo {
        name: "libpq",
        version: "17.11",
        cmake_package: "PostgreSQL",
        target: "PostgreSQL::PostgreSQL",
    },
    PackageInfo {
        name: "hiredis",
        version: "1.3.0",
        cmake_package: "hiredis",
        target: "hiredis::hiredis_all_unofficial",
    },
    PackageInfo {
        name: "mongo-c-driver",
        version: "2.5.8",
        cmake_package: "mongoc",
        target: "mongo::mongoc_static",
    },
    PackageInfo {
        name: "fmt",
        version: "11.0.2",
        cmake_package: "fmt",
        target: "fmt::fmt",
    },
    PackageInfo {
        name: "spdlog",
        version: "1.15.0",
        cmake_package: "spdlog",
        target: "spdlog::spdlog",
    },
    PackageInfo {
        name: "zlib",
        version: "1.3.1",
        cmake_package: "ZLIB",
        target: "ZLIB::ZLIB",
    },
    PackageInfo {
        name: "nlohmann_json",
        version: "3.11.3",
        cmake_package: "nlohmann_json",
        target: "nlohmann_json::nlohmann_json",
    },
    PackageInfo {
        name: "catch2",
        version: "3.7.1",
        cmake_package: "Catch2",
        target: "Catch2::Catch2",
    },
    PackageInfo {
        name: "boost",
        version: "1.91.0",
        cmake_package: "Boost",
        target: "boost::boost",
    },
    PackageInfo {
        name: "cpp-httplib",
        version: "0.62.0",
        cmake_package: "httplib",
        target: "httplib::httplib",
    },
    PackageInfo {
        name: "asio",
        version: "1.38.2",
        cmake_package: "asio",
        target: "asio::asio",
    },
    PackageInfo {
        name: "imgui",
        version: "1.92.9b",
        cmake_package: "imgui",
        target: "imgui::imgui",
    },
    PackageInfo {
        name: "gtest",
        version: "1.18.0",
        cmake_package: "GTest",
        target: "gtest::gtest",
    },
    PackageInfo {
        name: "qt",
        version: "6.12.0",
        cmake_package: "Qt6",
        target: "qt::qt",
    },
    PackageInfo {
        name: "opencv",
        version: "4.14.0",
        cmake_package: "OpenCV",
        target: "opencv::opencv",
    },
    PackageInfo {
        name: "eigen",
        version: "3.4.0",
        cmake_package: "Eigen3",
        target: "Eigen3::Eigen",
    },
    PackageInfo {
        name: "protobuf",
        version: "7.36.2",
        cmake_package: "protobuf",
        target: "protobuf::protobuf",
    },
    PackageInfo {
        name: "glm",
        version: "1.0.3",
        cmake_package: "glm",
        target: "glm::glm",
    },
    PackageInfo {
        name: "onetbb",
        version: "2023.1.0",
        cmake_package: "TBB",
        target: "TBB::tbb",
    },
];

/// `cppm packages`: the built-in catalog, without contacting Conan.
pub fn print() -> Result<()> {
    for entry in CATALOG {
        println!(
            "{}@{}  {} ({})",
            entry.name, entry.version, entry.target, entry.cmake_package
        );
    }
    println!("Aliases:");
    for (alias, package) in ALIASES {
        let (_, version) = parse_spec(alias)?;
        match version {
            Some(version) => println!("  {alias}={package}@{version}"),
            None => println!("  {alias}={package}"),
        }
    }
    println!("Other Conan recipes are resolved automatically by name.");
    println!("Common examples: sdl, curl, sfml, raylib, glfw, glm, openssl");
    println!("Database packages install client libraries; database servers run separately.");
    Ok(())
}

pub fn canonical_name(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map_or(name, |(_, package)| (*package).into())
}

pub fn parse_spec(spec: &str) -> Result<(String, Option<String>)> {
    let (alias, version) = spec
        .split_once('@')
        .map_or((spec, None), |(n, v)| (n, Some(v)));
    let name = canonical_name(alias);
    ensure!(
        valid_name(&name),
        "invalid package name '{name}'; use name@version"
    );
    let default = match alias.to_ascii_lowercase().as_str() {
        "sdl2" => Some("2.32.10"),
        "sdl3" => Some("3.4.16"),
        _ => CATALOG
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.version),
    };
    let version = version.or(default);
    if let Some(version) = version {
        validate_version(version)?;
    }
    Ok((name, version.map(str::to_owned)))
}

pub fn dependency(
    version: String,
    cmake_package: Option<String>,
    targets: Vec<String>,
) -> Result<Dependency> {
    validate_version(&version)?;
    if cmake_package.is_none() && targets.is_empty() {
        return Ok(Dependency::Version(version));
    }
    ensure!(
        cmake_package.is_some() && !targets.is_empty(),
        "pass --cmake-package and at least one --target together"
    );
    Ok(Dependency::Detailed(DependencyDetails {
        version,
        cmake_package: cmake_package.unwrap(),
        targets,
    }))
}

pub fn mapping(
    name: &str,
    dependency: &Dependency,
    graph: &Value,
) -> Result<(String, Vec<String>)> {
    match dependency {
        Dependency::Detailed(d) => Ok((d.cmake_package.clone(), d.targets.clone())),
        Dependency::Version(_) => {
            let node = Graph::new(graph)
                .host(name)
                .with_context(|| format!("Conan did not return metadata for '{name}'"))?;
            ensure!(
                node.cmake_property("cmake_find_mode") != Some("none"),
                "'{name}' does not expose a CMake package"
            );
            let package = node
                .cmake_property("cmake_file_name")
                .unwrap_or(name)
                .to_owned();
            let target = node
                .cmake_property("cmake_target_name")
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{name}::{name}"));
            validate_cmake_token(&package)?;
            validate_cmake_token(&target)?;
            Ok((package, vec![target]))
        }
    }
}

pub fn cmake_fixes(graph: &Value) -> String {
    let graph = Graph::new(graph);
    let static_host = |name: &str| {
        graph
            .host(name)
            .filter(|node| node.option_is("shared", false))
    };
    let mut fixes = String::new();
    if cfg!(target_os = "linux")
        && static_host("sdl").is_some_and(|sdl| {
            sdl.version()
                .is_some_and(|version| version.starts_with("3.4."))
                && sdl.option_is("wayland", true)
        })
    {
        fixes.push_str(include_str!("../cmake/sdl-wayland.cmake"));
    }
    if cfg!(target_os = "linux")
        && static_host("libpq").is_some_and(|libpq| libpq.version() == Some("17.11"))
    {
        fixes.push_str(include_str!("../cmake/postgresql-gssapi.cmake"));
    }
    if graph.host("protobuf").is_some() {
        fixes.push_str(include_str!("../cmake/protobuf-protoc.cmake"));
    }
    fixes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_aliases_use_exact_defaults_and_accept_explicit_versions() {
        for alias in ["mysql", "MySQL", "MariaDB", "mariadb-connector-c"] {
            assert_eq!(
                parse_spec(alias).unwrap(),
                ("mariadb-connector-c".into(), Some("3.4.8".into()))
            );
        }
        for alias in ["postgres", "PostgreSQL", "postgress", "libpq"] {
            assert_eq!(
                parse_spec(alias).unwrap(),
                ("libpq".into(), Some("17.11".into()))
            );
        }
        assert_eq!(
            parse_spec("redis").unwrap(),
            ("hiredis".into(), Some("1.3.0".into()))
        );
        assert_eq!(
            parse_spec("MongoDB").unwrap(),
            ("mongo-c-driver".into(), Some("2.5.8".into()))
        );
        assert_eq!(
            parse_spec("mongo@1.30.11").unwrap(),
            ("mongo-c-driver".into(), Some("1.30.11".into()))
        );
        assert_eq!(
            parse_spec("postgres@16.15").unwrap(),
            ("libpq".into(), Some("16.15".into()))
        );
        assert!(parse_spec("mysql@").is_err());
        assert!(parse_spec("postgres@>=17").is_err());
    }

    #[test]
    fn existing_aliases_keep_generation_defaults_and_unknown_packages() {
        assert_eq!(
            parse_spec("sqlite").unwrap(),
            ("sqlite3".into(), Some("3.54.0".into()))
        );
        assert_eq!(
            parse_spec("SDL2").unwrap(),
            ("sdl".into(), Some("2.32.10".into()))
        );
        assert_eq!(
            parse_spec("sdl3").unwrap(),
            ("sdl".into(), Some("3.4.16".into()))
        );
        assert_eq!(
            parse_spec("sdl3@3.4.14").unwrap(),
            ("sdl".into(), Some("3.4.14".into()))
        );
        assert_eq!(parse_spec("curl").unwrap(), ("libcurl".into(), None));
        assert_eq!(
            parse_spec("CUSTOM-PACKAGE").unwrap(),
            ("custom-package".into(), None)
        );
    }

    #[test]
    fn common_library_names_and_aliases_have_pinned_defaults() {
        for (spec, package, version) in [
            ("Boost", "boost", "1.91.0"),
            ("cpp-httplib", "cpp-httplib", "0.62.0"),
            ("httplib", "cpp-httplib", "0.62.0"),
            ("Asio", "asio", "1.38.2"),
            ("Dear-ImGui", "imgui", "1.92.9b"),
            ("dearimgui", "imgui", "1.92.9b"),
            ("Catch2", "catch2", "3.7.1"),
            ("googletest", "gtest", "1.18.0"),
            ("google-test", "gtest", "1.18.0"),
            ("Qt", "qt", "6.12.0"),
            ("qt6", "qt", "6.12.0"),
            ("OpenCV", "opencv", "4.14.0"),
            ("Eigen", "eigen", "3.4.0"),
            ("Protobuf", "protobuf", "7.36.2"),
            ("GLM", "glm", "1.0.3"),
            ("oneTBB", "onetbb", "2023.1.0"),
            ("tbb", "onetbb", "2023.1.0"),
        ] {
            assert_eq!(
                parse_spec(spec).unwrap(),
                (package.into(), Some(version.into()))
            );
        }
        assert_eq!(
            parse_spec("Qt6@6.10.1").unwrap(),
            ("qt".into(), Some("6.10.1".into()))
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn native_recipe_fixes_compose_and_skip_unaffected_graphs() {
        let graph = serde_json::json!({"graph": {"nodes": {
            "1": {"name": "sdl", "version": "3.4.16", "context": "host", "options": {"shared": "False", "wayland": "True"}},
            "2": {"name": "libpq", "version": "17.11", "context": "host", "options": {"shared": "False"}}
        }}});
        let fixes = cmake_fixes(&graph);
        assert!(fixes.contains("libdecor"));
        assert!(fixes.contains("krb5-gssapi"));
        for (context, shared) in [("build", false), ("host", true)] {
            let graph = serde_json::json!({"graph": {"nodes": {"1": {
                "name": "libpq", "version": "17.11", "context": context, "options": {"shared": shared}
            }}}});
            assert!(cmake_fixes(&graph).is_empty());
        }
        assert!(cmake_fixes(&Value::Null).is_empty());
    }
}

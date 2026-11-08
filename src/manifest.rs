use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, fs, path::Path};

pub const FILE_NAME: &str = "cppm.toml";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(default)]
    pub scripts: BTreeMap<String, Script>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Script {
    Shell(String),
    Command(Vec<String>),
}

impl fmt::Display for Script {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shell(body) => f.write_str(body),
            Self::Command(args) => f.write_str(&args.join(" ")),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(default = "default_standard", rename = "cpp-standard")]
    pub cpp_standard: u8,
    #[serde(default, skip_serializing_if = "Target::is_native")]
    pub target: Target,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    Native,
    Wasm,
}

impl Target {
    pub fn is_native(&self) -> bool {
        *self == Self::Native
    }
}

fn default_standard() -> u8 {
    20
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Detailed(DependencyDetails),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DependencyDetails {
    pub version: String,
    #[serde(rename = "cmake-package")]
    pub cmake_package: String,
    pub targets: Vec<String>,
}

impl Dependency {
    pub fn version(&self) -> &str {
        match self {
            Self::Version(v) => v,
            Self::Detailed(d) => &d.version,
        }
    }
}

impl Manifest {
    pub fn new(name: String) -> Self {
        Self {
            package: Package {
                name,
                version: "0.1.0".into(),
                cpp_standard: 20,
                target: Target::Native,
            },
            dependencies: BTreeMap::new(),
            scripts: BTreeMap::from([
                (
                    "dev".into(),
                    Script::Command(vec!["cppm".into(), "exec".into(), "--".into()]),
                ),
                (
                    "build".into(),
                    Script::Command(vec!["cppm".into(), "build".into(), "--release".into()]),
                ),
                (
                    "test".into(),
                    Script::Command(vec!["cppm".into(), "test".into(), "--".into()]),
                ),
            ]),
        }
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(FILE_NAME);
        let source =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        let manifest: Self =
            toml::from_str(&source).with_context(|| format!("invalid {}", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.package.target != Target::Wasm || self.dependencies.is_empty(),
            "WASM dependencies need a dedicated Emscripten Conan profile; this version supports the standard library in WASM projects"
        );
        ensure!(
            valid_name(&self.package.name),
            "package.name must start with a letter and contain only ASCII letters, digits, '_' or '-'"
        );
        ensure!(
            !self.package.version.is_empty(),
            "package.version cannot be empty"
        );
        ensure!(
            [11, 14, 17, 20, 23, 26].contains(&self.package.cpp_standard),
            "unsupported cpp-standard; use 11, 14, 17, 20, 23 or 26"
        );
        for (name, dependency) in &self.dependencies {
            ensure!(valid_name(name), "invalid dependency name '{name}'");
            validate_version(dependency.version())?;
            if let Dependency::Detailed(details) = dependency {
                validate_cmake_token(&details.cmake_package)?;
                ensure!(
                    !details.targets.is_empty(),
                    "dependency '{name}' needs at least one CMake target"
                );
                for target in &details.targets {
                    validate_cmake_token(target)?;
                }
            }
        }
        for (name, script) in &self.scripts {
            ensure!(valid_name(name), "invalid script name '{name}'");
            let valid = match script {
                Script::Shell(body) => !body.trim().is_empty(),
                Script::Command(args) => args
                    .first()
                    .is_some_and(|program| !program.trim().is_empty()),
            };
            ensure!(valid, "script '{name}' is empty");
        }
        Ok(())
    }
}

pub fn valid_name(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}

pub fn validate_version(value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._+-".contains(&c))
    {
        bail!("invalid version '{value}'; use an exact version such as 11.0.2");
    }
    Ok(())
}

pub fn validate_cmake_token(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_:+.-".contains(&c)),
        "invalid CMake name '{value}'"
    );
    Ok(())
}

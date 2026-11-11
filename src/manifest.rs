use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, fs, path::Path};

pub const FILE_NAME: &str = "cppm.toml";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    /// Libraries for tests and tools only; never linked into the application.
    #[serde(
        default,
        rename = "dev-dependencies",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub dev_dependencies: BTreeMap<String, Dependency>,
    #[serde(default)]
    pub scripts: BTreeMap<String, Script>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

    pub fn set_version(&mut self, version: String) {
        match self {
            Self::Version(v) => *v = version,
            Self::Detailed(d) => d.version = version,
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
            dev_dependencies: BTreeMap::new(),
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

    /// Serializes the manifest. Given the file's current text, only the
    /// dependency entries that changed are rewritten, so comments, key order
    /// and formatting written by the user survive `add`, `remove` and `upgrade`.
    pub fn render(&self, existing: Option<&str>) -> Result<String> {
        let edited = existing.and_then(|source| self.edit(source));
        match edited {
            Some(text) => Ok(text),
            None => Ok(toml::to_string_pretty(self)?),
        }
    }

    fn edit(&self, source: &str) -> Option<String> {
        let previous: Self = toml::from_str(source).ok()?;
        let mut document = source.parse::<toml_edit::Document>().ok()?;
        for (section, current, before) in [
            ("dependencies", &self.dependencies, &previous.dependencies),
            (
                "dev-dependencies",
                &self.dev_dependencies,
                &previous.dev_dependencies,
            ),
        ] {
            if current == before {
                continue;
            }
            if current.is_empty() && section == "dev-dependencies" {
                document.remove(section);
                continue;
            }
            if !document.contains_key(section) {
                document.insert(section, toml_edit::table());
                place_after_dependencies(&mut document, section);
            }
            let table = document.get_mut(section)?.as_table_like_mut()?;
            let sorted = table.iter().map(|(key, _)| key).is_sorted();
            for name in before.keys().filter(|name| !current.contains_key(*name)) {
                table.remove(name);
            }
            for (name, dependency) in current {
                if before.get(name) == Some(dependency) {
                    continue;
                }
                let mut entry = match dependency {
                    Dependency::Version(version) => toml_edit::value(version.as_str()),
                    Dependency::Detailed(details) => {
                        let mut inline = toml_edit::InlineTable::new();
                        inline.insert("version", details.version.as_str().into());
                        inline.insert("cmake-package", details.cmake_package.as_str().into());
                        inline.insert(
                            "targets",
                            toml_edit::Value::Array(
                                details.targets.iter().map(String::as_str).collect(),
                            ),
                        );
                        toml_edit::value(inline)
                    }
                };
                // A comment after the old value belongs to the dependency.
                if let (Some(old), Some(new)) = (
                    table.get(name).and_then(toml_edit::Item::as_value),
                    entry.as_value_mut(),
                ) {
                    *new.decor_mut() = old.decor().clone();
                }
                table.insert(name, entry);
            }
            if sorted {
                table.sort_values();
            }
        }
        let text = document.to_string();
        // Anything this targeted edit cannot express falls back to a rewrite.
        (toml::from_str::<Self>(&text).ok().as_ref() == Some(self)).then_some(text)
    }

    /// Every package Conan must install: application and dev dependencies.
    pub fn requirements(&self) -> impl Iterator<Item = (&String, &Dependency)> {
        self.dependencies.iter().chain(&self.dev_dependencies)
    }

    pub fn has_requirements(&self) -> bool {
        !self.dependencies.is_empty() || !self.dev_dependencies.is_empty()
    }

    pub fn requires(&self, name: &str) -> bool {
        self.dependencies.contains_key(name) || self.dev_dependencies.contains_key(name)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.package.target != Target::Wasm || !self.has_requirements(),
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
        for name in self.dev_dependencies.keys() {
            ensure!(
                !self.dependencies.contains_key(name),
                "'{name}' is listed in both [dependencies] and [dev-dependencies]"
            );
        }
        for (name, dependency) in self.requirements() {
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

/// A table created by an edit would be printed last. Keep a new
/// `[dev-dependencies]` next to `[dependencies]`, where a reader expects it.
fn place_after_dependencies(document: &mut toml_edit::Document, section: &str) {
    fn last(table: &toml_edit::Table) -> Option<usize> {
        table
            .iter()
            .filter_map(|(_, item)| item.as_table().and_then(last))
            .chain(table.position())
            .max()
    }
    fn shift(table: &mut toml_edit::Table, after: usize) {
        if let Some(position) = table.position().filter(|position| *position > after) {
            table.set_position(position + 1);
        }
        for (_, item) in table.iter_mut() {
            if let Some(table) = item.as_table_mut() {
                shift(table, after);
            }
        }
    }
    let Some(after) = document
        .get("dependencies")
        .and_then(toml_edit::Item::as_table)
        .and_then(last)
    else {
        return;
    };
    for (_, item) in document.iter_mut() {
        if let Some(table) = item.as_table_mut() {
            shift(table, after);
        }
    }
    if let Some(table) = document
        .get_mut(section)
        .and_then(toml_edit::Item::as_table_mut)
    {
        table.set_position(after + 1);
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

#[cfg(test)]
mod tests {
    use super::*;

    const COMMENTED: &str = r#"# Team manifest: keep the comments!
[package]
name = "app"
version = "0.1.0"
cpp-standard = 20   # agreed in the March review

[dependencies]
# logging first
spdlog = "1.15.0"
fmt = "11.0.2"  # pinned: 11.1 changes locale output

[scripts]
dev = ["cppm", "exec", "--"]   # default entry point
"#;

    fn edited(source: &str, change: impl FnOnce(&mut Manifest)) -> String {
        let mut manifest: Manifest = toml::from_str(source).unwrap();
        change(&mut manifest);
        let text = manifest.render(Some(source)).unwrap();
        assert_eq!(toml::from_str::<Manifest>(&text).unwrap(), manifest);
        text
    }

    #[test]
    fn dependency_edits_keep_comments_order_and_unrelated_formatting() {
        let text = edited(COMMENTED, |manifest| {
            manifest
                .dependencies
                .insert("zlib".into(), Dependency::Version("1.3.1".into()));
            manifest
                .dependencies
                .get_mut("fmt")
                .unwrap()
                .set_version("11.2.0".into());
        });
        for kept in [
            "# Team manifest: keep the comments!",
            "cpp-standard = 20   # agreed in the March review",
            "# logging first\nspdlog = \"1.15.0\"",
            "fmt = \"11.2.0\"  # pinned: 11.1 changes locale output",
            "dev = [\"cppm\", \"exec\", \"--\"]   # default entry point",
        ] {
            assert!(text.contains(kept), "missing {kept:?} in:\n{text}");
        }
        // The user's order (spdlog before fmt) is not alphabetical; keep it
        // and append the new package instead of reordering their file.
        let position = |needle: &str| text.find(needle).unwrap();
        assert!(position("spdlog") < position("fmt = ") && position("fmt = ") < position("zlib"));
        let removed = edited(&text, |manifest| {
            manifest.dependencies.remove("spdlog");
        });
        assert!(!removed.contains("spdlog"));
        assert!(removed.contains("# pinned: 11.1 changes locale output"));
    }

    #[test]
    fn sorted_tables_stay_sorted_and_dev_dependencies_follow_dependencies() {
        let source = "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nfmt = \"11.0.2\"\nzlib = \"1.3.1\"\n\n[scripts]\ndev = \"cppm exec\"\n";
        let text = edited(source, |manifest| {
            manifest
                .dependencies
                .insert("boost".into(), Dependency::Version("1.91.0".into()));
            manifest
                .dev_dependencies
                .insert("catch2".into(), Dependency::Version("3.7.1".into()));
        });
        let position = |needle: &str| text.find(needle).unwrap();
        assert!(position("boost") < position("fmt") && position("fmt") < position("zlib"));
        assert!(
            position("[dependencies]") < position("[dev-dependencies]")
                && position("[dev-dependencies]") < position("[scripts]"),
            "{text}"
        );
        // Removing the last dev dependency removes its empty section.
        let text = edited(&text, |manifest| manifest.dev_dependencies.clear());
        assert!(!text.contains("dev-dependencies"));
    }

    #[test]
    fn detailed_dependencies_and_unparseable_files_are_still_written_correctly() {
        let source = "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n";
        let text = edited(source, |manifest| {
            manifest.dependencies.insert(
                "custom".into(),
                Dependency::Detailed(DependencyDetails {
                    version: "1.2.3".into(),
                    cmake_package: "Custom".into(),
                    targets: vec!["Custom::Core".into(), "Custom::Net".into()],
                }),
            );
        });
        assert!(text.contains("custom = {"), "{text}");
        // Without a readable previous file the manifest is serialized whole.
        let manifest: Manifest = toml::from_str(&text).unwrap();
        for existing in [None, Some("not = [valid")] {
            let whole = manifest.render(existing).unwrap();
            assert_eq!(toml::from_str::<Manifest>(&whole).unwrap(), manifest);
        }
    }

    #[test]
    fn a_package_cannot_be_both_an_application_and_a_dev_dependency() {
        let mut manifest = Manifest::new("app".into());
        manifest
            .dependencies
            .insert("fmt".into(), Dependency::Version("11.0.2".into()));
        manifest
            .dev_dependencies
            .insert("fmt".into(), Dependency::Version("11.0.2".into()));
        assert!(manifest.validate().is_err());
        manifest.dependencies.clear();
        assert!(manifest.validate().is_ok());
        assert!(manifest.requires("fmt") && manifest.has_requirements());
    }
}

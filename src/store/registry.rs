use super::{capture, command, content, home, lock};
use crate::{
    conan::InstallOptions,
    manifest::Dependency,
    project::Project,
    storage::{write_atomic, write_if_changed},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, TryLockError},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct References {
    pub recipes: BTreeSet<String>,
    pub binaries: BTreeMap<String, PathBuf>,
}

impl References {
    fn extend(&mut self, other: &Self) {
        self.recipes.extend(other.recipes.iter().cloned());
        self.binaries.extend(other.binaries.clone());
    }

    fn from_graph(graph: &Value) -> Result<Self> {
        let mut refs = Self::default();
        let nodes = graph["graph"]["nodes"]
            .as_object()
            .context("Conan did not return graph nodes for project registration")?;
        for node in nodes.values() {
            if let Some(python) = node["python_requires"].as_object() {
                refs.recipes.extend(python.keys().cloned());
            }
            if matches!(
                node["recipe"].as_str(),
                Some("Editable" | "Consumer" | "Virtual")
            ) {
                continue;
            }
            let Some(reference) = node["ref"].as_str().filter(|r| r.contains('#')) else {
                continue;
            };
            ensure!(
                !reference.contains('*'),
                "invalid recipe reference in Conan graph"
            );
            refs.recipes.insert(reference.to_owned());
            if let (Some(id), Some(revision), Some(folder)) = (
                node["package_id"].as_str(),
                node["prev"].as_str(),
                node["package_folder"].as_str(),
            ) {
                ensure!(
                    !id.contains('*') && !revision.contains('*'),
                    "invalid binary reference in Conan graph"
                );
                refs.binaries.insert(
                    format!("{reference}:{id}#{revision}"),
                    PathBuf::from(folder),
                );
            }
        }
        Ok(refs)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegisteredProject {
    pub root: PathBuf,
    dependencies: BTreeMap<String, Dependency>,
    states: BTreeMap<String, References>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Index {
    format: u32,
    pub projects: BTreeMap<String, RegisteredProject>,
    pub owned: References,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            format: 1,
            projects: BTreeMap::new(),
            owned: References::default(),
        }
    }
}

pub(super) fn read(home: &Path) -> Result<Index> {
    let path = home.join("cppm-store/index.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Index::default()),
        Err(error) => return Err(error.into()),
    };
    let index: Index = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "invalid shared store registry {}; cleanup stopped",
            path.display()
        )
    })?;
    ensure!(
        index.format == 1,
        "unsupported shared store registry; cleanup stopped"
    );
    Ok(index)
}

fn save(home: &Path, index: &Index) -> Result<()> {
    write_if_changed(
        &home.join("cppm-store/index.json"),
        &serde_json::to_vec_pretty(index)?,
    )
}

fn project_key(root: &Path) -> Result<String> {
    Ok(blake3::hash(&serde_json::to_vec(root)?)
        .to_hex()
        .to_string())
}

fn configuration(profile: &str, release: bool) -> String {
    format!("{}:{profile}", if release { "release" } else { "debug" })
}

pub(crate) struct Pending {
    _guard: File,
    home: PathBuf,
    refs: References,
    configuration: String,
}

impl Pending {
    pub(crate) fn new(
        home: PathBuf,
        guard: File,
        graph: &Value,
        options: &InstallOptions,
        release: bool,
    ) -> Result<Self> {
        Ok(Self {
            _guard: guard,
            home,
            refs: References::from_graph(graph)?,
            configuration: configuration(&options.profile, release),
        })
    }

    // The store lock stays held from Conan installation through the manifest
    // and lockfile commit. Prune cannot observe unregistered successful packages.
    pub(crate) fn commit(
        self,
        root: &Path,
        dependencies: BTreeMap<String, Dependency>,
    ) -> Result<()> {
        let root = root.canonicalize()?;
        let mut index = read(&self.home)?;
        let new_packages: Vec<_> = self
            .refs
            .binaries
            .iter()
            .filter(|(reference, _)| !index.owned.binaries.contains_key(*reference))
            .map(|(_, path)| path.clone())
            .collect();
        index.owned.extend(&self.refs);
        let project = index
            .projects
            .entry(project_key(&root)?)
            .or_insert_with(|| RegisteredProject {
                root: root.clone(),
                dependencies: dependencies.clone(),
                states: BTreeMap::new(),
            });
        if project.dependencies != dependencies {
            // An upgrade/removal invalidates the old dependency set in every
            // mode. Unchanged manifests retain Debug, Release and other profiles.
            project.states.clear();
        }
        project.dependencies = dependencies;
        project.states.insert(self.configuration, self.refs);
        let mut homes = project_homes(&root)?;
        homes.insert(self.home.clone());
        write_if_changed(
            &root.join(".cppm/store-homes.json"),
            &serde_json::to_vec_pretty(&homes)?,
        )?;
        save(&self.home, &index)?;
        if let Err(error) = content::dedupe(&self.home, &new_packages) {
            eprintln!(
                "Warning: file deduplication could not finish: {error:#}; run `cppm store dedupe` to retry."
            );
        }
        Ok(())
    }
}

fn project_homes(root: &Path) -> Result<BTreeSet<PathBuf>> {
    match fs::read(root.join(".cppm/store-homes.json")) {
        Ok(bytes) => {
            Ok(serde_json::from_slice(&bytes).context("invalid project store registrations")?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn clear_project(root: &Path) -> Result<()> {
    let root = root.canonicalize()?;
    let key = project_key(&root)?;
    // A dependency-free project still works without Conan installed. Its local
    // record locates earlier stores, including a previously selected custom one.
    for home in project_homes(&root)? {
        if !home.try_exists()? {
            continue;
        }
        let _guard = lock(&home)?;
        let mut index = read(&home)?;
        index.projects.remove(&key);
        save(&home, &index)?;
    }
    Ok(())
}

pub(super) fn project_exists(root: &Path) -> Result<bool> {
    match fs::metadata(root.join("cppm.toml")) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file(),
                "cannot inspect project {}; cleanup stopped",
                root.display()
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error)
            .with_context(|| format!("cannot inspect project {}; cleanup stopped", root.display())),
    }
}

pub(crate) fn lease(
    project: &Project,
    release: bool,
    options: &InstallOptions,
) -> Result<Option<Lease>> {
    if project.manifest.dependencies.is_empty() {
        return Ok(None);
    }
    let home = home(options)?;
    let _guard = lock(&home)?;
    let index = read(&home)?;
    let key = project_key(&project.root.canonicalize()?)?;
    let refs = index
        .projects
        .get(&key)
        .and_then(|project| {
            project
                .states
                .get(&configuration(&options.profile, release))
        })
        .context("project dependency registration is missing; run `cppm install`")?;
    Ok(Some(create_lease(&home, refs)?))
}

pub(crate) struct Lease {
    _guard: tempfile::NamedTempFile,
    metadata: PathBuf,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.metadata);
    }
}

fn create_lease(home: &Path, refs: &References) -> Result<Lease> {
    let directory = home.join("cppm-store/leases");
    fs::create_dir_all(&directory)?;
    let file = tempfile::Builder::new()
        .suffix(".lock")
        .tempfile_in(directory)?;
    let metadata = file.path().with_extension("refs");
    // Keep metadata separate from the lock: Windows byte-range locks also
    // prevent reading the locked file, unlike Unix advisory locks.
    write_atomic(&metadata, &serde_json::to_vec(refs)?)?;
    file.as_file().lock()?;
    Ok(Lease {
        _guard: file,
        metadata,
    })
}

fn active_references(home: &Path, index: &Index) -> Result<(References, BTreeSet<String>)> {
    let mut pins = References::default();
    let mut dead = BTreeSet::new();
    for (key, project) in &index.projects {
        if project_exists(&project.root)? {
            for refs in project.states.values() {
                pins.extend(refs);
            }
        } else {
            dead.insert(key.clone());
        }
    }
    let directory = home.join("cppm-store/leases");
    if directory.try_exists()? {
        for path in content::files(&directory)? {
            if !matches!(
                path.extension().and_then(|v| v.to_str()),
                Some("lock" | "json")
            ) {
                continue;
            }
            let file = match File::options().read(true).write(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            match file.try_lock() {
                Ok(()) => {} // Abandoned process; this lease pins nothing.
                Err(TryLockError::WouldBlock) => {
                    let metadata = if path.extension().is_some_and(|ext| ext == "lock") {
                        path.with_extension("refs")
                    } else {
                        path.clone()
                    };
                    let bytes =
                        match fs::read(metadata) {
                            Ok(bytes) => bytes,
                            // The application can finish between testing its lock
                            // and reading metadata. Ignore only a released lease.
                            Err(error)
                                if error.kind() == std::io::ErrorKind::NotFound
                                    && file.try_lock().is_ok() =>
                            {
                                continue;
                            }
                            Err(error) => return Err(error).context(
                                "cannot read an active application's dependencies; cleanup stopped",
                            ),
                        };
                    let refs: References = serde_json::from_slice(&bytes).context(
                        "cannot read an active application's dependencies; cleanup stopped",
                    )?;
                    pins.extend(&refs);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok((pins, dead))
}

// Read every recipe and binary revision, rather than only Conan's latest ones.
fn cache_references(options: &InstallOptions) -> Result<References> {
    let listing: Value = serde_json::from_slice(&capture(command(options).args([
        "list",
        "*#*:*#*",
        "--format=json",
    ]))?)?;
    let recipes = listing["Local Cache"]
        .as_object()
        .context("invalid Conan cache listing")?;
    let mut refs = References::default();
    for (name, recipe) in recipes {
        {
            let revisions = recipe["revisions"]
                .as_object()
                .context("Conan returned incomplete recipe metadata; cleanup stopped")?;
            for (revision, data) in revisions {
                let reference = format!("{name}#{revision}");
                refs.recipes.insert(reference.clone());
                if let Some(packages) = data.get("packages") {
                    let packages = packages
                        .as_object()
                        .context("Conan returned invalid binary metadata; cleanup stopped")?;
                    for (id, data) in packages {
                        {
                            let revisions = data["revisions"]
                                .as_object()
                                .context("Conan omitted binary revisions; cleanup stopped")?;
                            for revision in revisions.keys() {
                                refs.binaries
                                    .insert(format!("{reference}:{id}#{revision}"), PathBuf::new());
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(refs)
}

#[derive(Default, Serialize)]
pub(super) struct PruneReport {
    pub removed_binaries: usize,
    pub removed_recipes: usize,
    pub forgotten_projects: usize,
    pub removed_shared_files: usize,
}

pub(super) fn prune(home: &Path, options: &InstallOptions, dry_run: bool) -> Result<PruneReport> {
    let mut index = read(home)?;
    // Validate every live project's/lease's metadata before the first deletion.
    let (pins, dead) = active_references(home, &index)?;
    let cache = if index.owned.recipes.is_empty() && index.owned.binaries.is_empty() {
        References::default()
    } else {
        cache_references(options)?
    };
    let binaries: BTreeSet<_> = index
        .owned
        .binaries
        .keys()
        .filter(|r| !pins.binaries.contains_key(*r) && cache.binaries.contains_key(*r))
        .cloned()
        .collect();
    let recipes: BTreeSet<_> = index
        .owned
        .recipes
        .iter()
        .filter(|r| !pins.recipes.contains(*r) && cache.recipes.contains(*r))
        // Unknown binaries may belong to direct Conan users. A recipe is only
        // removable when all its remaining binaries are managed and orphaned.
        .filter(|r| {
            cache
                .binaries
                .keys()
                .filter(|b| b.split(':').next() == Some(r.as_str()))
                .all(|b| binaries.contains(b))
        })
        .cloned()
        .collect();
    for reference in &recipes {
        println!(
            "{} recipe {reference}",
            if dry_run { "Would remove" } else { "Removing" }
        );
        if !dry_run {
            capture(command(options).args(["remove", reference, "--confirm"]))?;
        }
    }
    for reference in &binaries {
        if recipes.contains(reference.split(':').next().unwrap()) {
            continue;
        }
        println!(
            "{} binary {reference}",
            if dry_run { "Would remove" } else { "Removing" }
        );
        if !dry_run {
            capture(command(options).args(["remove", reference, "--confirm"]))?;
        }
    }
    let mut report = PruneReport {
        removed_binaries: binaries.len(),
        removed_recipes: recipes.len(),
        forgotten_projects: dead.len(),
        removed_shared_files: 0,
    };
    if !dry_run {
        index.projects.retain(|key, _| !dead.contains(key));
        index
            .owned
            .binaries
            .retain(|r, _| !binaries.contains(r) && cache.binaries.contains_key(r));
        index
            .owned
            .recipes
            .retain(|r| !recipes.contains(r) && cache.recipes.contains(r));
        // Persist after removal. If a process crashes before this save, another
        // prune reconciles the registry against Conan's actual cache contents.
        if home.join("cppm-store/index.json").try_exists()? {
            save(home, &index)?;
        }
        report.removed_shared_files = content::prune(home, index.owned.binaries.values(), false)?;
        let directory = home.join("cppm-store/leases");
        if directory.try_exists()? {
            for path in content::files(&directory)? {
                if !matches!(
                    path.extension().and_then(|v| v.to_str()),
                    Some("lock" | "json")
                ) {
                    continue;
                }
                let file = match File::options().read(true).write(true).open(&path) {
                    Ok(file) => file,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                if file.try_lock().is_ok() {
                    if path.extension().is_some_and(|ext| ext == "lock") {
                        match fs::remove_file(path.with_extension("refs")) {
                            Ok(()) => {}
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                            Err(error) => return Err(error.into()),
                        }
                    }
                    drop(file);
                    fs::remove_file(path)?;
                }
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn refs(name: &str) -> References {
        References {
            recipes: BTreeSet::from([format!("{name}/1.0#recipe")]),
            binaries: BTreeMap::from([(
                format!("{name}/1.0#recipe:id#binary"),
                PathBuf::from("/unused"),
            )]),
        }
    }

    #[test]
    fn graph_registration_keeps_transitive_build_and_python_requirements() {
        let graph = json!({"graph":{"nodes":{
            "0":{"recipe":"Consumer", "ref":"conanfile.py"},
            "1":{"recipe":"Cache", "ref":"library/1.0#recipe", "package_id":"host", "prev":"rev", "package_folder":"/cache/library", "python_requires":{"helper/1.0#pyrev":{}}},
            "2":{"recipe":"Cache", "ref":"tool/1.0#recipe", "context":"build", "package_id":"build", "prev":"rev", "package_folder":"/cache/tool"},
            "3":{"recipe":"Cache", "ref":"transitive/1.0#recipe", "binary":"Skip"},
            "4":{"recipe":"Editable", "ref":"local/1.0#recipe", "package_id":"id", "prev":"rev", "package_folder":"/do-not-touch"}
        }}});
        let refs = References::from_graph(&graph).unwrap();
        assert_eq!(refs.recipes.len(), 4);
        assert_eq!(refs.binaries.len(), 2);
        assert!(refs.recipes.contains("helper/1.0#pyrev"));
        assert!(
            refs.binaries
                .values()
                .all(|path| path.starts_with("/cache"))
        );
    }

    #[test]
    fn active_application_leases_pin_revisions_after_projects_are_removed() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        fs::create_dir_all(home.join("cppm-store/leases")).unwrap();
        let live = refs("live-app");
        let file = create_lease(home, &live).unwrap();
        fs::write(
            home.join("cppm-store/leases/abandoned.json"),
            "corrupt abandoned lease",
        )
        .unwrap();
        let index = Index::default();
        assert_eq!(
            active_references(home, &index).unwrap().0.binaries,
            live.binaries
        );
        drop(file);
        assert!(
            active_references(home, &index)
                .unwrap()
                .0
                .binaries
                .is_empty()
        );
        let corrupt = create_lease(home, &live).unwrap();
        fs::write(&corrupt.metadata, "invalid live lease").unwrap();
        assert!(active_references(home, &index).is_err());
    }

    #[test]
    fn commits_retain_modes_until_dependencies_change_and_clear_without_conan() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("store");
        let root = temp.path().join("project");
        fs::create_dir_all(root.join(".cppm")).unwrap();
        fs::create_dir(&home).unwrap();
        fs::write(root.join("cppm.toml"), "test project").unwrap();
        let dependencies =
            BTreeMap::from([("library".to_owned(), Dependency::Version("1.0".into()))]);
        let commit = |release, dependencies: BTreeMap<String, Dependency>, name: &str| {
            Pending {
                _guard: lock(&home).unwrap(),
                home: home.clone(),
                refs: refs(name),
                configuration: configuration("default", release),
            }
            .commit(&root, dependencies)
            .unwrap();
        };
        commit(false, dependencies.clone(), "debug");
        commit(true, dependencies.clone(), "release");
        let index = read(&home).unwrap();
        assert_eq!(index.projects.values().next().unwrap().states.len(), 2);
        assert_eq!(
            active_references(&home, &index).unwrap().0.binaries.len(),
            2
        );
        // Preparing an installation without committing it changes no references.
        drop(Pending {
            _guard: lock(&home).unwrap(),
            home: home.clone(),
            refs: refs("failed"),
            configuration: configuration("default", false),
        });
        assert_eq!(read(&home).unwrap().owned.binaries.len(), 2);
        commit(
            false,
            BTreeMap::from([("library".to_owned(), Dependency::Version("2.0".into()))]),
            "upgraded",
        );
        let index = read(&home).unwrap();
        assert_eq!(index.owned.binaries.len(), 3);
        let pins = active_references(&home, &index).unwrap().0;
        assert_eq!(pins.binaries.len(), 1);
        assert!(pins.binaries.keys().all(|key| key.starts_with("upgraded/")));
        clear_project(&root).unwrap();
        assert!(read(&home).unwrap().projects.is_empty());
        assert_eq!(read(&home).unwrap().owned.binaries.len(), 3);
    }

    #[test]
    fn invalid_registries_and_uninspectable_projects_stop_collection() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        fs::create_dir_all(home.join("cppm-store")).unwrap();
        fs::write(home.join("cppm-store/index.json"), "invalid").unwrap();
        let options = InstallOptions {
            profile: "default".into(),
            store_dir: Some(home.to_path_buf()),
            frozen: false,
            offline: true,
            verbose: false,
        };
        assert!(prune(home, &options, false).is_err());
        fs::create_dir(home.join("project")).unwrap();
        fs::create_dir(home.join("project/cppm.toml")).unwrap();
        let index = Index {
            projects: BTreeMap::from([(
                "project".into(),
                RegisteredProject {
                    root: home.join("project"),
                    dependencies: BTreeMap::new(),
                    states: BTreeMap::new(),
                },
            )]),
            ..Index::default()
        };
        save(home, &index).unwrap();
        assert!(prune(home, &options, false).is_err());
    }
}

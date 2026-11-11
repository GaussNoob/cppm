use crate::{
    build,
    conan::{self, versions},
    manifest::{Dependency, Manifest},
    packages,
    project::Project,
    storage::write_if_changed,
    store,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

pub use crate::conan::InstallOptions;

pub fn install(project: &Project, release: bool, options: &InstallOptions) -> Result<()> {
    let (project, _guard) = project.locked()?;
    prepare(&project, release, options)?.commit(&project.root, options.frozen)?;
    build::refresh_editor(&project, release, options);
    Ok(())
}

pub fn add(
    project: &Project,
    specs: &[String],
    cmake_package: Option<String>,
    targets: Vec<String>,
    release: bool,
    dev: bool,
    options: &InstallOptions,
) -> Result<Vec<String>> {
    ensure!(!options.frozen, "add cannot be used with --frozen-lockfile");
    ensure!(!specs.is_empty(), "specify at least one package");
    ensure!(
        specs.len() == 1 || (cmake_package.is_none() && targets.is_empty()),
        "--cmake-package and --target require exactly one package"
    );
    // Validate the whole request before contacting Conan or changing project files.
    let mut requested = BTreeMap::new();
    for spec in specs {
        let (name, version) = packages::parse_spec(spec)?;
        if let Some(previous) = requested.get(&name) {
            ensure!(
                previous == &version,
                "conflicting versions requested for '{name}'"
            );
        }
        requested.insert(name, version);
    }
    let (mut project, _guard) = project.locked()?;
    ensure!(
        project.manifest.package.target.is_native(),
        "Conan dependencies for WebAssembly are not supported yet; native libraries cannot be linked into a WASM build"
    );
    let names: Vec<String> = requested.keys().cloned().collect();
    for (name, version) in requested {
        let version = match version {
            Some(version) => version,
            None => conan::resolve_version(&project, &name, release, options)?,
        };
        let dependency = packages::dependency(version, cmake_package.clone(), targets.clone())?;
        // Like `pnpm add -D`, adding to one section moves the package there.
        let manifest = &mut project.manifest;
        let (section, other) = if dev {
            (&mut manifest.dev_dependencies, &mut manifest.dependencies)
        } else {
            (&mut manifest.dependencies, &mut manifest.dev_dependencies)
        };
        other.remove(&name);
        section.insert(name, dependency);
    }
    let prepared = prepare(&project, release, options)?;
    project.save_manifest()?;
    prepared.commit(&project.root, false)?;
    build::refresh_editor(&project, release, options);
    if names.iter().any(|name| name == "sdl")
        && let Some((_, sdl)) = project
            .manifest
            .requirements()
            .find(|(name, _)| *name == "sdl")
    {
        let version = sdl.version();
        if version.starts_with("2.") {
            println!(
                "SDL {version} ready: use #include <SDL.h> (SDL2). SDL3 examples require `cppm add sdl3` and <SDL3/SDL.h>."
            );
        } else if version.starts_with("3.") {
            println!(
                "SDL {version} ready: use #include <SDL3/SDL.h> (SDL3). For SDL2 code using <SDL.h>, run `cppm add sdl2`."
            );
        }
    }
    Ok(names)
}

pub fn remove(project: &Project, name: &str, options: &InstallOptions) -> Result<()> {
    let name = packages::canonical_name(name);
    ensure!(
        !options.frozen,
        "remove cannot be used with --frozen-lockfile"
    );
    let (mut project, _guard) = project.locked()?;
    let manifest = &mut project.manifest;
    if manifest.dependencies.remove(&name).is_none()
        && manifest.dev_dependencies.remove(&name).is_none()
    {
        let hint = crate::suggest::hint(&name, manifest.requirements().map(|(name, _)| name));
        bail!("'{name}' is not a direct dependency{hint}");
    }
    let prepared = prepare(&project, false, options)?;
    project.save_manifest()?;
    prepared.commit(&project.root, false)?;
    build::refresh_editor(&project, false, options);
    Ok(())
}

pub struct Upgrade {
    pub name: String,
    pub from: String,
    pub to: String,
}

/// Moves direct dependencies to newer versions and reinstalls them together.
/// Without `latest`, a package stays within its current major version.
pub fn upgrade(
    project: &Project,
    names: &[String],
    latest: bool,
    release: bool,
    options: &InstallOptions,
) -> Result<Vec<Upgrade>> {
    ensure!(
        !options.frozen,
        "upgrade cannot be used with --frozen-lockfile"
    );
    let selected: Vec<String> = names
        .iter()
        .map(|name| packages::canonical_name(name))
        .collect();
    let (mut project, _guard) = project.locked()?;
    for name in &selected {
        if !project.manifest.requires(name) {
            let hint =
                crate::suggest::hint(name, project.manifest.requirements().map(|(name, _)| name));
            bail!("'{name}' is not a direct dependency{hint}");
        }
    }
    let upgrades: Vec<Upgrade> = versions::report(&project, options)?
        .into_iter()
        .filter(|entry| selected.is_empty() || selected.contains(&entry.name))
        .filter_map(|entry| {
            let to = if latest { entry.latest } else { entry.wanted }?;
            Some(Upgrade {
                name: entry.name,
                from: entry.current,
                to,
            })
        })
        .collect();
    if upgrades.is_empty() {
        return Ok(upgrades);
    }
    for upgrade in &upgrades {
        let manifest = &mut project.manifest;
        let dependency = manifest
            .dependencies
            .get_mut(&upgrade.name)
            .or_else(|| manifest.dev_dependencies.get_mut(&upgrade.name))
            .expect("reported dependencies come from the manifest");
        dependency.set_version(upgrade.to.clone());
    }
    let prepared = prepare(&project, release, options)?;
    project.save_manifest()?;
    prepared.commit(&project.root, false)?;
    build::refresh_editor(&project, release, options);
    Ok(upgrades)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Lockfile {
    format: u32,
    dependencies: BTreeMap<String, Dependency>,
    #[serde(
        default,
        rename = "dev-dependencies",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    dev_dependencies: BTreeMap<String, Dependency>,
    conan: Value,
}

impl Lockfile {
    fn matches(&self, manifest: &Manifest) -> bool {
        self.dependencies == manifest.dependencies
            && self.dev_dependencies == manifest.dev_dependencies
    }
}

pub struct Installation {
    lockfile: Lockfile,
    integration: String,
    registration: Option<store::Pending>,
}

impl Installation {
    pub fn commit(self, root: &Path, frozen: bool) -> Result<()> {
        write_if_changed(
            &root.join(".cppm/dependencies.cmake"),
            self.integration.as_bytes(),
        )?;
        if !frozen {
            let mut bytes = serde_json::to_vec_pretty(&self.lockfile)?;
            bytes.push(b'\n');
            write_if_changed(&root.join("cppm.lock"), &bytes)?;
        }
        let mut dependencies = self.lockfile.dependencies;
        dependencies.extend(self.lockfile.dev_dependencies);
        if let Some(registration) = self.registration {
            registration.commit(root, dependencies)?;
        } else if dependencies.is_empty() {
            store::clear_project(root)?;
        }
        Ok(())
    }
}

fn read_lock(root: &Path) -> Result<Option<Lockfile>> {
    let path = root.join("cppm.lock");
    let source = match fs::read(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let lock: Lockfile = serde_json::from_slice(&source).context("invalid cppm.lock")?;
    ensure!(
        lock.format == 1,
        "unsupported cppm.lock format {}",
        lock.format
    );
    Ok(Some(lock))
}

fn integration(manifest: &Manifest, graph: &Value) -> Result<String> {
    let mut source = format!(
        "# Generated by cppm. Edit cppm.toml to change dependencies.\nset(CPPM_CXX_STANDARD {})\n",
        manifest.package.cpp_standard
    );
    // Both targets always exist, so a CMakeLists.txt can link them before the
    // first package is added. Tests link cppm_dev_dependencies as well.
    for (target, dependencies) in [
        ("cppm_dependencies", &manifest.dependencies),
        ("cppm_dev_dependencies", &manifest.dev_dependencies),
    ] {
        source.push_str(&format!("add_library({target} INTERFACE)\n"));
        for (name, dependency) in dependencies {
            let (package, targets) = packages::mapping(name, dependency, graph)?;
            source.push_str(&format!(
                "find_package({package} REQUIRED)\ntarget_link_libraries({target} INTERFACE {})\n",
                targets.join(" ")
            ));
        }
    }
    source.push_str(&packages::cmake_fixes(graph));
    Ok(source)
}

pub fn prepare(project: &Project, release: bool, options: &InstallOptions) -> Result<Installation> {
    prepare_with_cache(project, release, options, false)
}

pub fn prepare_build(
    project: &Project,
    release: bool,
    options: &InstallOptions,
    reuse: bool,
) -> Result<Installation> {
    prepare_with_cache(project, release, options, reuse)
}

fn prepare_with_cache(
    project: &Project,
    release: bool,
    options: &InstallOptions,
    reuse: bool,
) -> Result<Installation> {
    project.manifest.validate()?;
    let previous = read_lock(&project.root)?;
    if options.frozen {
        let previous = previous
            .as_ref()
            .context("cppm.lock is missing; run `cppm install` before using --frozen-lockfile")?;
        ensure!(
            previous.matches(&project.manifest),
            "cppm.toml and cppm.lock disagree; run `cppm install` to update the lockfile"
        );
    }
    let (graph, metadata, registration) = if !project.manifest.has_requirements() {
        (Value::Null, Value::Null, None)
    } else {
        let installed = conan::install(
            project,
            release,
            options,
            previous.as_ref().map(|lock| &lock.conan),
            reuse,
        )?;
        if options.frozen
            && previous
                .as_ref()
                .is_some_and(|lock| lock.conan != installed.lock)
        {
            bail!("dependency graph changed; run `cppm install` without --frozen-lockfile");
        }
        (
            installed.lock,
            installed.graph,
            Some(installed.registration),
        )
    };
    Ok(Installation {
        lockfile: Lockfile {
            format: 1,
            dependencies: project.manifest.dependencies.clone(),
            dev_dependencies: project.manifest.dev_dependencies.clone(),
            conan: graph,
        },
        integration: integration(&project.manifest, &metadata)?,
        registration,
    })
}

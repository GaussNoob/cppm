use crate::{
    build::{self, BuildOptions},
    clean,
    dependencies::{self, InstallOptions},
    doctor, editor, inspect, packages, process,
    project::{self, Project},
    store, suggest,
    templates::Template,
    ui, update, watch,
};
use anyhow::{Context, Result, ensure};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Simple C++ projects, powered by CMake, Ninja and Conan 2"
)]
pub struct Cli {
    /// Run from this directory (also searches parent directories for cppm.toml).
    #[arg(short = 'C', long, global = true)]
    directory: Option<PathBuf>,
    /// Native Conan profile, used for both host and build.
    #[arg(long, global = true, default_value = "default", env = "CPPM_PROFILE")]
    profile: String,
    /// Shared dependency store (defaults to CONAN_HOME, then Conan's user cache).
    #[arg(long, global = true, env = "CPPM_STORE_DIR")]
    store_dir: Option<PathBuf>,
    /// Resolve dependencies only from Conan's local cache.
    #[arg(long, global = true, env = "CPPM_OFFLINE")]
    offline: bool,
    /// Require the manifest and resolved graph to match cppm.lock.
    #[arg(long, global = true, env = "CPPM_FROZEN_LOCKFILE")]
    frozen_lockfile: bool,
    /// Show dependency/configuration details and OpenCV informational messages.
    #[arg(short = 'v', long, global = true, env = "CPPM_VERBOSE")]
    verbose: bool,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Update cppm itself from the latest stable GitHub release.
    Update {
        /// Check for a new version without installing it.
        #[arg(long)]
        check: bool,
    },
    #[command(name = "__update-check", hide = true)]
    UpdateCheck,
    /// Open the interactive package explorer, project dashboard and command palette.
    #[command(visible_aliases = ["tui", "interactive"])]
    Ui,
    /// Create a ready-to-run C++ project without overwriting existing files.
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
        /// Project starter: app, Drogon HTTP API, or browser WebAssembly.
        #[arg(long, value_enum, default_value_t = Template::App, ignore_case = true)]
        template: Template,
    },
    #[command(flatten)]
    Project(ProjectAction),
    /// List packages with built-in version and CMake defaults.
    Packages,
    /// Inspect or clean the dependency store shared by all projects.
    Store {
        #[command(subcommand)]
        command: store::Action,
    },
    /// Check project tools and compile and run a C++ or WebAssembly probe.
    Doctor {
        /// Print a machine-readable report, including every failed check.
        #[arg(long)]
        json: bool,
    },
}

/// Commands that operate on the project containing the current directory.
#[derive(Subcommand)]
enum ProjectAction {
    /// Add and install dependencies together (fmt json or fmt@11.0.2).
    Add {
        #[arg(required = true, num_args = 1..)]
        packages: Vec<String>,
        #[arg(long)]
        cmake_package: Option<String>,
        #[arg(long = "target")]
        targets: Vec<String>,
        /// Add to [dev-dependencies]: linked into tests, never into the application.
        #[arg(short = 'D', long)]
        dev: bool,
    },
    /// Remove a dependency and refresh the lockfile.
    #[command(visible_alias = "rm")]
    Remove { package: String },
    /// Install the manifest dependencies, or add several packages together.
    #[command(visible_alias = "i")]
    Install {
        #[arg(num_args = 0..)]
        packages: Vec<String>,
        #[arg(long)]
        release: bool,
        /// Add the given packages to [dev-dependencies].
        #[arg(short = 'D', long, requires = "packages")]
        dev: bool,
    },
    /// Install exactly what cppm.lock records; fails instead of updating it (for CI).
    Ci {
        #[arg(long)]
        release: bool,
    },
    /// Show direct dependencies, or the resolved tree with --depth/--all.
    #[command(visible_alias = "ls")]
    List {
        /// Levels of transitive dependencies to show below the direct ones.
        #[arg(long, default_value_t = 0, conflicts_with = "all")]
        depth: usize,
        /// Show the whole resolved tree.
        #[arg(long)]
        all: bool,
        /// Inspect the Release resolution instead of Debug.
        #[arg(long)]
        release: bool,
        #[arg(long)]
        json: bool,
    },
    /// Explain which dependencies bring a package into the project.
    Why {
        package: String,
        #[arg(long)]
        release: bool,
        #[arg(long)]
        json: bool,
    },
    /// List the licenses declared by every library in the dependency graph.
    Licenses {
        #[arg(long)]
        release: bool,
        #[arg(long)]
        json: bool,
    },
    /// Check the direct dependencies for newer versions.
    Outdated {
        #[arg(long)]
        json: bool,
    },
    /// Upgrade dependencies within their major version (all, or the ones named).
    #[command(visible_alias = "up")]
    Upgrade {
        packages: Vec<String>,
        /// Also cross major versions, which may break the application's code.
        #[arg(long)]
        latest: bool,
        #[arg(long)]
        release: bool,
    },
    /// Run a manifest script; without a name, list scripts.
    Run {
        script: Option<String>,
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Compile the project.
    Build(BuildArgs),
    /// Remove generated build artifacts while keeping sources, lockfile and dependencies.
    Clean {
        /// Which build configurations to remove.
        #[arg(value_enum, default_value_t = clean::Mode::All)]
        mode: clean::Mode,
        /// Show build directories, file counts and bytes without deleting them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Rebuild/restart a script when source, configuration or web assets change.
    Watch {
        #[arg(default_value = "dev")]
        script: String,
        /// Polling interval in milliseconds.
        #[arg(long, default_value_t = 250, value_parser = clap::value_parser!(u64).range(50..=5000))]
        interval: u64,
        /// Wait for this many milliseconds without edits before restarting.
        #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(50..=5000))]
        debounce: u64,
        /// Restrict watching to these project-relative files/directories (repeatable).
        #[arg(long = "path")]
        paths: Vec<PathBuf>,
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Configure CMake and editor include paths without compiling the application.
    Configure {
        #[arg(long)]
        release: bool,
        /// Resolve dependencies again and discard CMake's configuration cache.
        #[arg(long)]
        fresh: bool,
    },
    /// Create VS Code build/run tasks that use cppm and its dependencies.
    Vscode {
        /// Folder opened in VS Code (defaults to the project root).
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Compile and execute the application.
    Exec {
        #[command(flatten)]
        build: BuildArgs,
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Compile and run CTest.
    Test {
        #[command(flatten)]
        build: BuildArgs,
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Build and serve a WebAssembly project on localhost.
    Serve {
        #[command(flatten)]
        build: BuildArgs,
        #[arg(long, default_value_t = 8080, value_parser = clap::value_parser!(u16).range(1..))]
        port: u16,
    },
}

#[derive(Args)]
struct BuildArgs {
    /// Protected native Release preset with the bundled LLVM obfuscator.
    #[arg(value_enum)]
    mode: Option<BuildMode>,
    #[arg(long)]
    release: bool,
    /// Resolve dependencies again and discard CMake's configuration cache.
    #[arg(long)]
    fresh: bool,
    /// Load an LLVM pass plugin with Clang's GNU frontend (requires obf).
    #[arg(long, requires = "mode", value_name = "PATH")]
    obf_plugin: Option<PathBuf>,
    /// Reduce metadata only, without the bundled LLVM plugin or Clang SDK.
    #[arg(long, requires = "mode", conflicts_with = "obf_plugin")]
    obf_basic: bool,
    #[arg(short = 'j', long, value_parser = clap::value_parser!(u16).range(1..))]
    jobs: Option<u16>,
}

#[derive(Clone, Copy, ValueEnum)]
enum BuildMode {
    Obf,
}

impl From<BuildArgs> for BuildOptions {
    fn from(args: BuildArgs) -> Self {
        Self {
            release: args.release || args.mode.is_some(),
            jobs: args.jobs.map(usize::from),
            obfuscate: args.mode.is_some(),
            obf_plugin: args.obf_plugin,
            obf_basic: args.obf_basic,
            fresh: args.fresh,
        }
    }
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        if let Some(directory) = self.directory {
            std::env::set_current_dir(&directory)
                .with_context(|| format!("cannot enter {}", directory.display()))?;
        }
        let cwd = std::env::current_dir()?;
        match &self.command {
            Action::Update { check } => {
                ensure!(
                    !self.offline,
                    "cppm update needs Internet access; omit --offline"
                );
                return update::run(*check);
            }
            Action::UpdateCheck => {
                if !self.offline {
                    update::refresh_cache();
                }
                return Ok(());
            }
            _ => {}
        }
        update::start(self.offline, matches!(self.command, Action::Ui));
        let install = InstallOptions {
            profile: self.profile,
            // Scripts start from the project root; preserve the caller's location.
            store_dir: self
                .store_dir
                .or_else(|| std::env::var_os("CONAN_HOME").map(PathBuf::from))
                .map(|path| cwd.join(path)),
            offline: self.offline,
            frozen: self.frozen_lockfile,
            verbose: self.verbose,
        };
        match self.command {
            Action::Update { .. } | Action::UpdateCheck => Ok(()),
            Action::Ui => ui::run(&cwd, &install),
            Action::Init {
                path,
                name,
                template,
            } => project::init(&path, name, template),
            Action::Packages => packages::print(),
            Action::Doctor { json } => doctor::run(&cwd, json),
            Action::Store { command } => store::run(command, &install),
            Action::Project(action) => action.execute(&Project::discover(&cwd)?, install),
        }
    }
}

fn report_added(names: Vec<String>) {
    for name in names {
        println!("Added {name}");
    }
}

impl ProjectAction {
    fn execute(self, project: &Project, mut install: InstallOptions) -> Result<()> {
        match self {
            Self::Add {
                packages,
                cmake_package,
                targets,
                dev,
            } => {
                report_added(dependencies::add(
                    project,
                    &packages,
                    cmake_package,
                    targets,
                    false,
                    dev,
                    &install,
                )?);
                Ok(())
            }
            Self::Remove { package } => {
                dependencies::remove(project, &package, &install)?;
                println!("Removed {package}");
                Ok(())
            }
            Self::Install {
                packages,
                release,
                dev,
            } => {
                if packages.is_empty() {
                    dependencies::install(project, release, &install)?;
                    println!("Dependencies ready");
                } else {
                    report_added(dependencies::add(
                        project,
                        &packages,
                        None,
                        vec![],
                        release,
                        dev,
                        &install,
                    )?);
                }
                Ok(())
            }
            Self::Ci { release } => {
                install.frozen = true;
                dependencies::install(project, release, &install)?;
                println!("Dependencies ready (cppm.lock unchanged)");
                Ok(())
            }
            Self::List {
                depth,
                all,
                release,
                json,
            } => inspect::list(project, if all { usize::MAX } else { depth }, release, json),
            Self::Why {
                package,
                release,
                json,
            } => inspect::why(project, &package, release, json),
            Self::Licenses { release, json } => inspect::licenses(project, release, json),
            Self::Outdated { json } => inspect::outdated(project, &install, json),
            Self::Upgrade {
                packages,
                latest,
                release,
            } => {
                let upgrades =
                    dependencies::upgrade(project, &packages, latest, release, &install)?;
                if upgrades.is_empty() {
                    println!(
                        "Nothing to upgrade{}.",
                        if latest {
                            ""
                        } else {
                            " within the current major versions; see `cppm outdated`"
                        }
                    );
                }
                for upgrade in upgrades {
                    println!(
                        "Upgraded {} {} -> {}",
                        upgrade.name, upgrade.from, upgrade.to
                    );
                }
                Ok(())
            }
            Self::Run { script, args } => {
                let scripts = &project.manifest.scripts;
                let Some(name) = script else {
                    ensure!(args.is_empty(), "specify a script before passing arguments");
                    for (name, body) in scripts {
                        println!("{name}: {body}");
                    }
                    return Ok(());
                };
                let body = scripts.get(&name).with_context(|| {
                    format!(
                        "unknown script '{name}'{}; use `cppm run` to list scripts",
                        suggest::hint(&name, scripts.keys())
                    )
                })?;
                process::script(&project.root, &name, body, &args, &install)
            }
            Self::Build(args) => build::build(project, &args.into(), &install),
            Self::Clean { mode, dry_run } => clean::run(project, mode, dry_run),
            Self::Watch {
                script,
                interval,
                debounce,
                paths,
                args,
            } => watch::run(
                project,
                &install,
                &watch::Options {
                    script,
                    interval: std::time::Duration::from_millis(interval),
                    debounce: std::time::Duration::from_millis(debounce),
                    paths,
                    args,
                },
            ),
            Self::Configure { release, fresh } => {
                build::configure(project, release, fresh, &install)
            }
            Self::Vscode { workspace } => {
                editor::vscode(project, workspace.as_deref().unwrap_or(&project.root))
            }
            Self::Exec {
                build: args,
                args: forwarded,
            } => build::exec(project, &args.into(), &install, &forwarded),
            Self::Test {
                build: args,
                args: forwarded,
            } => build::test(project, &args.into(), &install, &forwarded),
            Self::Serve { build: args, port } => {
                build::serve(project, &args.into(), &install, port)
            }
        }
    }
}

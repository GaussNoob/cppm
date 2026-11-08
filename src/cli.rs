use crate::{
    build::{self, BuildOptions},
    clean,
    dependencies::{self, InstallOptions},
    doctor, editor, packages, process,
    project::{self, Project},
    store,
    templates::Template,
    ui, watch,
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
    /// Add and install dependencies together (fmt json or fmt@11.0.2).
    Add {
        #[arg(required = true, num_args = 1..)]
        packages: Vec<String>,
        #[arg(long)]
        cmake_package: Option<String>,
        #[arg(long = "target")]
        targets: Vec<String>,
    },
    /// Remove a dependency and refresh the lockfile.
    Remove { package: String },
    /// Install the manifest dependencies, or add several packages together.
    Install {
        #[arg(num_args = 0..)]
        packages: Vec<String>,
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
            Action::Ui => ui::run(&cwd, &install),
            Action::Init {
                path,
                name,
                template,
            } => project::init(&path, name, template),
            Action::Packages => {
                for entry in packages::CATALOG {
                    println!(
                        "{}@{}  {} ({})",
                        entry.name, entry.version, entry.target, entry.cmake_package
                    );
                }
                println!("Aliases:");
                for (alias, package) in packages::ALIASES {
                    let (_, version) = packages::parse_spec(alias)?;
                    match version {
                        Some(version) => println!("  {alias}={package}@{version}"),
                        None => println!("  {alias}={package}"),
                    }
                }
                println!("Other Conan recipes are resolved automatically by name.");
                println!("Common examples: sdl, curl, sfml, raylib, glfw, glm, openssl");
                println!(
                    "Database packages install client libraries; database servers run separately."
                );
                Ok(())
            }
            Action::Doctor { json } => doctor::run(&cwd, json),
            Action::Store { command } => store::run(command, &install),
            action => {
                let project = Project::discover(&cwd)?;
                match action {
                    Action::Add {
                        packages,
                        cmake_package,
                        targets,
                    } => {
                        let names = dependencies::add(
                            &project,
                            &packages,
                            cmake_package,
                            targets,
                            false,
                            &install,
                        )?;
                        for name in names {
                            println!("Added {name}");
                        }
                        Ok(())
                    }
                    Action::Remove { package } => {
                        dependencies::remove(&project, &package, &install)?;
                        println!("Removed {package}");
                        Ok(())
                    }
                    Action::Install { packages, release } => {
                        if packages.is_empty() {
                            dependencies::install(&project, release, &install)?;
                            println!("Dependencies ready");
                        } else {
                            for name in dependencies::add(
                                &project,
                                &packages,
                                None,
                                vec![],
                                release,
                                &install,
                            )? {
                                println!("Added {name}");
                            }
                        }
                        Ok(())
                    }
                    Action::Run { script, args } => {
                        if let Some(name) = script {
                            let body = project.manifest.scripts.get(&name).with_context(|| {
                                format!("unknown script '{name}'; use `cppm run` to list scripts")
                            })?;
                            process::script(&project.root, &name, body, &args, &install)
                        } else {
                            ensure!(args.is_empty(), "specify a script before passing arguments");
                            for (name, body) in project.manifest.scripts {
                                println!("{name}: {body}");
                            }
                            Ok(())
                        }
                    }
                    Action::Build(args) => build::build(&project, &args.into(), &install),
                    Action::Clean { mode, dry_run } => clean::run(&project, mode, dry_run),
                    Action::Watch {
                        script,
                        interval,
                        debounce,
                        paths,
                        args,
                    } => watch::run(
                        &project,
                        &install,
                        &watch::Options {
                            script,
                            interval: std::time::Duration::from_millis(interval),
                            debounce: std::time::Duration::from_millis(debounce),
                            paths,
                            args,
                        },
                    ),
                    Action::Configure { release, fresh } => {
                        build::configure(&project, release, fresh, &install)
                    }
                    Action::Vscode { workspace } => {
                        editor::vscode(&project, workspace.as_deref().unwrap_or(&project.root))
                    }
                    Action::Exec {
                        build: args,
                        args: forwarded,
                    } => build::exec(&project, &args.into(), &install, &forwarded),
                    Action::Test {
                        build: args,
                        args: forwarded,
                    } => build::test(&project, &args.into(), &install, &forwarded),
                    Action::Serve { build: args, port } => {
                        build::serve(&project, &args.into(), &install, port)
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

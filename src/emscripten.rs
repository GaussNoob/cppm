use crate::{fingerprint::Fingerprint, platform};
use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::Deserialize;
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

#[cfg(not(windows))]
pub const SETUP_HINT: &str = "Install the Emscripten SDK: https://emscripten.org/docs/getting_started/downloads.html\ncppm detects installed SDKs automatically. Set CPPM_EMSDK to select a custom SDK directory.\nIf the SDK has not been configured, run its `emsdk activate latest` once.\nManual activation is also supported: source /path/to/emsdk/emsdk_env.sh (Bash/zsh), source /path/to/emsdk/emsdk_env.fish (fish), or emsdk_env.bat (Windows).\nRun `cppm doctor` from the WebAssembly project to check the environment.";
#[cfg(windows)]
pub const SETUP_HINT: &str = concat!(
    "Rerun the current cppm Windows setup to install Emscripten and Node.js automatically: ",
    "https://github.com/GaussNoob/cppm/releases/latest\n",
    "Existing projects are preserved. Open a new terminal and retry cppm doctor / cppm run dev.\n",
    "For a separately installed SDK, set CPPM_EMSDK to its directory. Shell activation is not required."
);

static TOOLCHAIN: OnceLock<Result<Toolchain, String>> = OnceLock::new();

struct Toolchain {
    compiler: PathBuf,
    wrapper: PathBuf,
    node: OsString,
    python: Option<OsString>,
    sdk: Option<PathBuf>,
    config: Option<PathBuf>,
    environment: Vec<(OsString, OsString)>,
}

fn current() -> Result<&'static Toolchain> {
    TOOLCHAIN
        .get_or_init(|| Toolchain::discover().map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|error| anyhow!(error.clone()))
}

pub fn command(program: impl AsRef<OsStr>) -> Result<Command> {
    let toolchain = current()?;
    let program = program.as_ref();
    let program = if program == "em++" {
        toolchain.compiler.as_os_str()
    } else if program == "emcmake" {
        toolchain.wrapper.as_os_str()
    } else if program == "node" {
        &toolchain.node
    } else if matches!(program.to_str(), Some("python" | "python3")) {
        toolchain.python.as_deref().unwrap_or(program)
    } else {
        program
    };
    let mut command = platform::command(program);
    #[cfg(windows)]
    if matches!(
        Path::new(program).extension().and_then(OsStr::to_str),
        Some("bat" | "cmd")
    ) {
        let script = Path::new(program).with_extension("py");
        if script.is_file()
            && let Some(python) = &toolchain.python
        {
            // Run the SDK's Python entrypoint directly so cmd.exe never
            // interprets SDK or project paths containing spaces and '&'.
            command = platform::command(python);
            command.arg(script);
        }
    }
    command.envs(toolchain.environment.iter().cloned());
    Ok(command)
}

pub fn cmake() -> Result<Command> {
    let mut command = command("emcmake")?;
    command.arg("cmake");
    Ok(command)
}

pub fn node() -> OsString {
    current()
        .map(|toolchain| toolchain.node.clone())
        .unwrap_or_else(|_| inherited_node())
}

pub fn python() -> OsString {
    current()
        .ok()
        .and_then(|toolchain| toolchain.python.clone())
        .unwrap_or_else(|| if cfg!(windows) { "python" } else { "python3" }.into())
}

pub fn fingerprint(value: &mut Fingerprint) -> Result<()> {
    if let Some(config) = &current()?.config {
        value.file(config)?;
    }
    Ok(())
}

fn inherited_node() -> OsString {
    std::env::var_os("EMSDK_NODE")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "node".into())
}

pub fn check(cwd: &Path) -> Result<String> {
    let compiler = capture(command("em++")?.arg("--version").current_dir(cwd))?;
    // emcmake --version prints usage and exits with status 1. Run CMake's
    // version command through it to check that the wrapper also works.
    capture(cmake()?.arg("--version").current_dir(cwd))?;
    let version = compiler.lines().next().unwrap_or_default();
    Ok(match &current()?.sdk {
        Some(sdk) => format!("{version}; SDK: {}", sdk.display()),
        None => version.to_owned(),
    })
}

impl Toolchain {
    fn discover() -> Result<Self> {
        for name in ["CPPM_EMSDK", "EMSDK"] {
            if let Some(directory) = std::env::var_os(name).filter(|value| !value.is_empty()) {
                return Self::from_sdk(&std::path::absolute(directory)?)
                    .with_context(|| format!("cannot use the SDK selected by {name}"));
            }
        }
        if let (Some(compiler), Some(wrapper)) = (
            platform::find_executable("em++"),
            platform::find_executable("emcmake"),
        ) {
            return Ok(Self {
                compiler,
                wrapper,
                node: inherited_node(),
                python: std::env::var_os("EMSDK_PYTHON").filter(|value| !value.is_empty()),
                sdk: None,
                config: std::env::var_os("EM_CONFIG").map(PathBuf::from),
                environment: Vec::new(),
            });
        }
        for directory in candidates() {
            if directory.join(".emscripten").is_file() && directory.join("emsdk.py").is_file() {
                return Self::from_sdk(&directory);
            }
        }
        bail!(
            "cannot find em++/emcmake in PATH or an installed Emscripten SDK; set CPPM_EMSDK to your SDK directory"
        )
    }

    fn from_sdk(directory: &Path) -> Result<Self> {
        let directory = directory
            .canonicalize()
            .with_context(|| format!("SDK directory {} does not exist", directory.display()))?;
        #[cfg(windows)]
        let directory = {
            let path = directory
                .to_str()
                .context("the Emscripten SDK requires a UTF-8 path")?;
            if let Some(network) = path.strip_prefix(r"\\?\UNC\") {
                PathBuf::from(format!(r"\\{network}"))
            } else {
                PathBuf::from(path.strip_prefix(r"\\?\").unwrap_or(path))
            }
        };
        let tools = directory.join("upstream/emscripten");
        let compiler = platform::find_executable(tools.join("em++"))
            .with_context(|| format!("em++ is missing from SDK {}", directory.display()))?;
        let wrapper = platform::find_executable(tools.join("emcmake"))
            .with_context(|| format!("emcmake is missing from SDK {}", directory.display()))?;
        let config = std::env::var_os("EM_CONFIG")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(std::path::absolute)
            .transpose()?
            .unwrap_or_else(|| directory.join(".emscripten"));
        ensure!(
            config.is_file(),
            "{} is missing; configure this SDK once with its `emsdk activate latest` command",
            config.display()
        );
        let python = bootstrap_python(&directory)?;
        let json = capture(
            platform::command(&python)
                .args(["-E", "-c", CONFIGURATION])
                .arg(&config)
                .env("EM_CONFIG", &config),
        )?;
        let configuration: Configuration =
            serde_json::from_str(&json).context("invalid Emscripten SDK configuration")?;
        let node = std::env::var_os("EMSDK_NODE")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| configuration.node.into());
        let node = platform::find_executable(&node)
            .map(PathBuf::into_os_string)
            .unwrap_or(node);
        let mut paths = vec![
            tools,
            directory.clone(),
            PathBuf::from(&node)
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf(),
        ];
        if let Some(llvm) = configuration.llvm {
            paths.push(llvm);
        }
        if let Some(binaryen) = configuration.binaryen {
            paths.push(binaryen.join("bin"));
        }
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let mut unique = Vec::new();
        for path in paths {
            if !path.as_os_str().is_empty() && !unique.contains(&path) {
                unique.push(path);
            }
        }
        let environment = vec![
            ("PATH".into(), std::env::join_paths(unique)?),
            ("EMSDK".into(), directory.clone().into_os_string()),
            ("EM_CONFIG".into(), config.clone().into_os_string()),
            ("EMSDK_NODE".into(), node.clone()),
            ("EMSDK_PYTHON".into(), python.clone()),
        ];
        Ok(Self {
            compiler,
            wrapper,
            node,
            python: Some(python),
            sdk: Some(directory),
            config: Some(config),
            environment,
        })
    }
}

fn candidates() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
        && let Ok(receipt) = std::fs::read(parent.join("cppm.install.json"))
        && let Ok(receipt) = serde_json::from_slice::<serde_json::Value>(
            receipt.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&receipt),
        )
        && let Some(tools) = receipt.get("toolsDir").and_then(serde_json::Value::as_str)
    {
        directories.push(PathBuf::from(tools).join("emsdk"));
    }
    if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        directories.push(PathBuf::from(data).join("cppm/emsdk"));
    }
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        let home = PathBuf::from(home);
        for path in [
            "emsdk",
            ".local/share/cppm/emsdk",
            ".local/share/emsdk",
            ".emsdk",
        ] {
            directories.push(home.join(path));
        }
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        directories.push(parent.join("emsdk"));
        directories.push(parent.join("../share/cppm/emsdk"));
        // A locally built cppm can use the SDK installed with this checkout.
        // current_exe resolves the user's ~/.local/bin/cppm symlink as well.
        for root in parent.ancestors().take(5) {
            if root.join("Cargo.toml").is_file() {
                directories.push(root.join(".qa/emsdk"));
                directories.push(root.join("emsdk"));
            }
        }
    }
    if cfg!(unix) {
        directories.extend([
            PathBuf::from("/opt/emsdk"),
            PathBuf::from("/usr/local/emsdk"),
        ]);
    }
    directories
}

fn bootstrap_python(directory: &Path) -> Result<OsString> {
    if let Some(python) = std::env::var_os("EMSDK_PYTHON").filter(|value| !value.is_empty()) {
        return platform::find_executable(&python)
            .map(PathBuf::into_os_string)
            .with_context(|| {
                format!(
                    "EMSDK_PYTHON points to an unavailable interpreter: {}",
                    python.to_string_lossy()
                )
            });
    }
    let bundled = bundled_python(directory);
    if cfg!(windows)
        && let Some(python) = &bundled
    {
        return Ok(python.clone());
    }
    let names = if cfg!(windows) {
        ["python", "python3"]
    } else {
        ["python3", "python"]
    };
    for name in names {
        if let Some(python) = platform::find_executable(name) {
            return Ok(python.into_os_string());
        }
    }
    if let Some(python) = bundled {
        return Ok(python);
    }
    bail!("Python 3 is required to use the Emscripten SDK; install it and add it to PATH")
}

fn bundled_python(directory: &Path) -> Option<OsString> {
    if let Ok(versions) = std::fs::read_dir(directory.join("python")) {
        let mut versions: Vec<_> = versions.filter_map(Result::ok).collect();
        versions.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
        for version in versions {
            for relative in ["python.exe", "bin/python3", "python"] {
                if let Some(python) = platform::find_executable(version.path().join(relative)) {
                    return Some(python.into_os_string());
                }
            }
        }
    }
    None
}

#[derive(Deserialize)]
struct Configuration {
    node: String,
    llvm: Option<PathBuf>,
    binaryen: Option<PathBuf>,
}

// Read the SDK's own Python configuration, just as Emscripten does. Paths and
// custom expressions stay valid, and no shell output is parsed or evaluated.
const CONFIGURATION: &str = r#"import contextlib, json, os, pathlib, sys
config = pathlib.Path(sys.argv[1])
os.environ['CFGDIR'] = str(config.parent)
values = {'__file__': str(config)}
with contextlib.redirect_stdout(sys.stderr):
    exec(compile(config.read_bytes(), str(config), 'exec'), values)
def expand(value):
    return os.path.normpath(os.path.expandvars(os.path.expanduser(str(value)))) if value else None
node = os.environ.get('EM_NODE_JS') or values.get('NODE_JS') or 'node'
if isinstance(node, (list, tuple)):
    node = node[0]
print(json.dumps({'node': expand(node), 'llvm': expand(os.environ.get('EM_LLVM_ROOT') or values.get('LLVM_ROOT')),
                  'binaryen': expand(os.environ.get('EM_BINARYEN_ROOT') or values.get('BINARYEN_ROOT'))}))
"#;

fn capture(command: &mut Command) -> Result<String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .with_context(|| format!("cannot start {program}; it must be available in PATH"))?;
    ensure!(
        output.status.success(),
        "{program} failed ({}):\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

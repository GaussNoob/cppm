use anyhow::Result;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
};

pub fn find_executable(program: impl AsRef<OsStr>) -> Option<PathBuf> {
    let path = Path::new(program.as_ref());
    let candidates = if path.components().count() > 1 || path.is_absolute() {
        vec![path.to_path_buf()]
    } else {
        let paths = std::env::var_os("PATH").unwrap_or_default();
        let directories = std::env::split_paths(&paths);
        #[cfg(windows)]
        let directories = managed_tools().into_iter().cloned().chain(directories);
        directories.map(|directory| directory.join(path)).collect()
    };
    for candidate in candidates {
        #[cfg(windows)]
        if candidate.extension().is_none() {
            for extension in ["exe", "bat", "cmd"] {
                let file = candidate.with_extension(extension);
                if file.is_file() {
                    return Some(file);
                }
            }
        }
        if let Ok(metadata) = std::fs::metadata(&candidate) {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }
            if metadata.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn build_command(program: impl AsRef<OsStr>) -> Result<Command> {
    let mut command = command(program);
    configure(&mut command)?;
    Ok(command)
}

pub fn command(program: impl AsRef<OsStr>) -> Command {
    Command::new(executable(program.as_ref()))
}

#[cfg(not(windows))]
fn executable(program: &OsStr) -> std::ffi::OsString {
    program.to_owned()
}

#[cfg(windows)]
fn executable(program: &OsStr) -> std::ffi::OsString {
    use std::path::Path;
    let path = Path::new(program);
    if path.extension().is_none() {
        let paths = std::env::var_os("PATH").unwrap_or_default();
        let candidates = if path.components().count() > 1 {
            vec![path.to_path_buf()]
        } else {
            let mut directories: Vec<_> = std::env::split_paths(&paths).collect();
            if let Some(managed) = managed_tools() {
                directories.insert(0, managed.clone());
            }
            directories
                .into_iter()
                .map(|directory| directory.join(path))
                .collect()
        };
        for candidate in candidates {
            for extension in ["exe", "bat", "cmd"] {
                let file = candidate.with_extension(extension);
                if file.is_file() {
                    return file.into_os_string();
                }
            }
        }
    }
    program.to_owned()
}

#[cfg(windows)]
fn managed_tools() -> Option<&'static PathBuf> {
    use std::sync::OnceLock;
    static TOOLS: OnceLock<Option<PathBuf>> = OnceLock::new();
    TOOLS
        .get_or_init(|| {
            let executable = std::env::current_exe().ok()?;
            let receipt = std::fs::read(executable.parent()?.join("cppm.install.json")).ok()?;
            // Windows PowerShell 5.1 writes a UTF-8 BOM with -Encoding UTF8.
            let source = receipt.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&receipt);
            let receipt: serde_json::Value = serde_json::from_slice(source).ok()?;
            let directory = PathBuf::from(receipt.get("toolsDir")?.as_str()?).join("venv/Scripts");
            directory.is_dir().then_some(directory)
        })
        .as_ref()
}

#[cfg(not(windows))]
fn configure(_command: &mut Command) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
fn configure(command: &mut Command) -> Result<()> {
    windows::configure(command)
}

#[cfg(windows)]
mod windows {
    use anyhow::{Context, Result, anyhow, bail, ensure};
    use std::{
        ffi::OsString,
        os::windows::process::CommandExt,
        path::{Path, PathBuf},
        process::Command,
        sync::OnceLock,
    };

    type Environment = Vec<(OsString, OsString)>;
    static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();

    pub fn configure(command: &mut Command) -> Result<()> {
        let environment = ENVIRONMENT
            .get_or_init(|| detect().map_err(|error| format!("{error:#}")))
            .as_ref()
            .map_err(|error| anyhow!(error.clone()))?;
        command.envs(environment.iter().cloned());
        if let Some(managed) = super::managed_tools() {
            let inherited = command
                .get_envs()
                .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("PATH"))
                .and_then(|(_, value)| value.map(OsString::from))
                .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default());
            let mut paths = vec![managed.clone()];
            paths.extend(std::env::split_paths(&inherited));
            command.env("PATH", std::env::join_paths(paths)?);
        }
        if std::env::var("CPPM_VERBOSE").as_deref() == Ok("true") {
            for (key, value) in command.get_envs() {
                if key.to_string_lossy().eq_ignore_ascii_case("PATH")
                    && let Some(value) = value
                {
                    eprintln!("MSVC subprocess PATH: {}", value.to_string_lossy());
                }
            }
        }
        Ok(())
    }

    fn detect() -> Result<Environment> {
        // Respect a configured developer prompt or an explicitly chosen compiler.
        if ["VCINSTALLDIR", "CXX", "CC"]
            .iter()
            .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
        {
            return Ok(Vec::new());
        }
        let program_files =
            std::env::var_os("ProgramFiles(x86)").context("cannot find ProgramFiles(x86)")?;
        let vswhere =
            PathBuf::from(program_files).join("Microsoft Visual Studio/Installer/vswhere.exe");
        let output = Command::new(vswhere).args([
            "-latest", "-products", "*", "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property", "installationPath", "-utf8",
        ]).output().context("MSVC Build Tools not found; install Visual Studio Build Tools with Desktop development with C++")?;
        ensure!(
            output.status.success(),
            "vswhere could not locate Visual Studio Build Tools"
        );
        let installation = String::from_utf8(output.stdout)?;
        let installation = installation.trim().trim_start_matches('\u{feff}');
        ensure!(
            !installation.is_empty(),
            "install the Desktop development with C++ workload in Visual Studio Build Tools"
        );
        let script = PathBuf::from(installation).join("Common7/Tools/VsDevCmd.bat");
        ensure!(
            script.is_file(),
            "VsDevCmd.bat is missing from the Visual Studio installation"
        );
        // Explicit selection sets VSCMD_ARG_VCVARS_VER. Conan's PowerShell
        // activator can then reuse this environment instead of calling vcvars
        // again through batch files that misinterpret metacharacters in PATH.
        let version_file = PathBuf::from(installation)
            .join("VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt");
        let toolset = std::fs::read_to_string(&version_file)
            .context("cannot read the Visual Studio default C++ toolset version")?;
        let toolset = toolset.trim().trim_start_matches('\u{feff}').trim();
        ensure!(
            toolset.split('.').count() >= 2
                && toolset
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit())),
            "invalid Visual Studio C++ toolset version"
        );
        let architecture = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "x86" => "x86",
            "aarch64" => "arm64",
            other => bail!("unsupported Windows host architecture '{other}'"),
        };
        // VsDevCmd.bat expands PATH unquoted, so a directory containing `&`,
        // `|` or `)` truncates it and later tools (Ninja, Python) disappear.
        // Run it with only the Windows directories, then put the user's PATH
        // back behind the entries Visual Studio added.
        let system_root = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        let minimal = [
            system_root.join("System32"),
            system_root.clone(),
            system_root.join(r"System32\Wbem"),
            system_root.join(r"System32\WindowsPowerShell\v1.0"),
        ];
        // The discovered path travels in an environment variable; the version
        // contains only validated numbers and dots, never command syntax.
        let output = Command::new("cmd.exe").args(["/D", "/U", "/V:OFF", "/S", "/C"])
            .raw_arg(format!(r#""call "%CPPM_VSDEVCMD%" -no_logo -arch={architecture} -host_arch={architecture} -vcvars_ver={toolset} >nul && set""#))
            .env("CPPM_VSDEVCMD", script)
            .env("PATH", std::env::join_paths(&minimal)?)
            .output().context("cannot initialize the MSVC build environment")?;
        ensure!(
            output.status.success(),
            "VsDevCmd.bat failed; check the MSVC toolchain and Windows SDK installation"
        );
        let utf16: Vec<u16> = output
            .stdout
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let source =
            String::from_utf16(&utf16).context("invalid environment returned by VsDevCmd.bat")?;
        let original = std::env::var_os("PATH").unwrap_or_default();
        let environment: Environment = source
            .lines()
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                if key.is_empty() {
                    return None;
                }
                let value = if key.eq_ignore_ascii_case("PATH") {
                    merge_path(value, &minimal, &original)
                } else {
                    OsString::from(value)
                };
                Some((OsString::from(key), value))
            })
            .collect();
        ensure!(
            environment
                .iter()
                .any(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("VCINSTALLDIR")),
            "MSVC environment did not define VCINSTALLDIR"
        );
        Ok(environment)
    }

    /// Visual Studio's PATH entries first, then the user's original PATH. The
    /// minimal Windows directories used to run VsDevCmd are dropped here: the
    /// original PATH already lists them where the user put them.
    fn merge_path(captured: &str, minimal: &[PathBuf], original: &std::ffi::OsStr) -> OsString {
        let same = |a: &Path, b: &Path| {
            a.as_os_str()
                .to_string_lossy()
                .trim_end_matches('\\')
                .eq_ignore_ascii_case(b.as_os_str().to_string_lossy().trim_end_matches('\\'))
        };
        let added = std::env::split_paths(captured)
            .filter(|entry| !entry.as_os_str().is_empty())
            .filter(|entry| !minimal.iter().any(|minimal| same(entry, minimal)));
        let entries: Vec<PathBuf> = added.chain(std::env::split_paths(original)).collect();
        // join_paths only fails for entries containing `"`, which split_paths
        // never yields; keep the original PATH if it ever does.
        std::env::join_paths(entries).unwrap_or_else(|_| original.to_os_string())
    }
}

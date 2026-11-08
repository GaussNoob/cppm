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
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|directory| directory.join(path))
            .collect()
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
            std::env::split_paths(&paths)
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
        ffi::OsString, os::windows::process::CommandExt, path::PathBuf, process::Command,
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
        if std::env::var("CPPM_VERBOSE").as_deref() == Ok("true") {
            for (key, value) in environment {
                if key.to_string_lossy().eq_ignore_ascii_case("PATH") {
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
        let architecture = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "x86" => "x86",
            "aarch64" => "arm64",
            other => bail!("unsupported Windows host architecture '{other}'"),
        };
        // The command text is fixed. The discovered path travels in an environment
        // variable so spaces and cmd metacharacters never become command syntax.
        let output = Command::new("cmd.exe").args(["/D", "/U", "/V:OFF", "/S", "/C"])
            .raw_arg(format!(r#""call "%CPPM_VSDEVCMD%" -no_logo -arch={architecture} -host_arch={architecture} >nul && set""#))
            .env("CPPM_VSDEVCMD", script).output().context("cannot initialize the MSVC build environment")?;
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
        let environment: Environment = source
            .lines()
            .filter_map(|line| {
                let (key, value) = line.split_once('=')?;
                if key.is_empty() {
                    return None;
                }
                Some((OsString::from(key), OsString::from(value)))
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
}

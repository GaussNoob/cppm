use crate::{manifest::Script, platform};
use anyhow::{Context, Result, bail};
use std::{fmt, path::Path, process::Command};

#[derive(Debug)]
pub struct CommandFailure(pub i32);

impl fmt::Display for CommandFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "command exited with status {}", self.0)
    }
}

impl std::error::Error for CommandFailure {}

pub fn run(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command.status().with_context(|| {
        format!("cannot start {program}; check that it is installed and available in PATH")
    })?;
    if !status.success() {
        // Preserve signal termination as a nonzero result too.
        return Err(CommandFailure(status.code().unwrap_or(130)).into());
    }
    Ok(())
}

pub fn check_tool(program: &str, cwd: &Path) -> Result<()> {
    let output = platform::command(program)
        .arg("--version")
        .current_dir(cwd)
        .output()
        .with_context(|| format!("{program} is required; install it and add it to PATH"))?;
    if !output.status.success() {
        bail!("{program} --version failed");
    }
    Ok(())
}

pub fn script(
    cwd: &Path,
    name: &str,
    body: &Script,
    args: &[String],
    options: &crate::dependencies::InstallOptions,
) -> Result<()> {
    let stack = std::env::var("CPPM_SCRIPT_STACK").unwrap_or_default();
    if stack.split(':').any(|entry| entry == name) {
        bail!("recursive script '{name}' ({stack}); call a build/exec command instead");
    }
    let executable = std::env::current_exe()?;
    let mut paths = vec![
        executable
            .parent()
            .context("cannot locate cppm executable")?
            .to_path_buf(),
    ];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let (mut command, _temporary) = match body {
        Script::Command(argv) => {
            let mut command = platform::command(argv.first().context("script has no executable")?);
            command.args(&argv[1..]).args(args);
            (command, None)
        }
        Script::Shell(body) => shell(body, args)?,
    };
    command
        .current_dir(cwd)
        .env("PATH", std::env::join_paths(paths)?)
        .env("CPPM_PROFILE", &options.profile)
        .env("CPPM_OFFLINE", options.offline.to_string())
        .env("CPPM_FROZEN_LOCKFILE", options.frozen.to_string())
        .env("CPPM_VERBOSE", options.verbose.to_string())
        .env("CPPM_SCRIPT_STACK", format!("{stack}:{name}"));
    if let Some(directory) = &options.store_dir {
        command.env("CPPM_STORE_DIR", directory);
    }
    run(&mut command)
}

#[cfg(not(windows))]
fn shell(body: &str, args: &[String]) -> Result<(Command, Option<tempfile::NamedTempFile>)> {
    let mut command = Command::new("sh");
    command
        .args(["-c", &format!("{body} \"$@\""), "cppm-script"])
        .args(args);
    Ok((command, None))
}

#[cfg(windows)]
fn shell(body: &str, args: &[String]) -> Result<(Command, Option<tempfile::NamedTempFile>)> {
    use std::io::Write;
    let mut script = tempfile::Builder::new()
        .prefix("cppm-")
        .suffix(".ps1")
        .tempfile()?;
    // A BOM keeps Windows PowerShell 5.1 from treating UTF-8 scripts as ANSI.
    script.write_all(b"\xef\xbb\xbf")?;
    let body = if args.is_empty() {
        body.to_owned()
    } else {
        format!("{body} @cppmScriptArgs")
    };
    writeln!(
        script,
        "$ErrorActionPreference = 'Stop'\n$cppmScriptArgs = @((ConvertFrom-Json -InputObject $env:CPPM_SCRIPT_ARGS))\n{body}\nif ($null -ne $LASTEXITCODE) {{ exit $LASTEXITCODE }}"
    )?;
    script.flush()?;
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script.path())
        .env("CPPM_SCRIPT_ARGS", serde_json::to_string(args)?);
    Ok((command, Some(script)))
}

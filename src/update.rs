use crate::storage;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, IsTerminal},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const VERSION_URL: &str = "https://github.com/GaussNoob/cppm/releases/latest/download/VERSION";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const CHECK_INTERVAL: u64 = 6 * 60 * 60;
const RETRY_INTERVAL: u64 = 60 * 60;

#[derive(Default, Deserialize, Serialize)]
struct Cache {
    checked_at: u64,
    latest: Option<String>,
    #[serde(default)]
    failed: bool,
}

// Releases are stable X.Y.Z versions. Numeric comparison avoids 0.9 > 0.10.
fn version(value: &str) -> Option<[u64; 3]> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let mut result = [0; 3];
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty()
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        result[index] = part.parse().ok()?;
    }
    Some(result)
}

fn newer(latest: &str) -> bool {
    version(latest)
        .zip(version(CURRENT))
        .is_some_and(|(latest, current)| latest > current)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cache_path() -> Option<PathBuf> {
    let root = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
    }?;
    Some(root.join("cppm").join("update.json"))
}

fn read_cache() -> Cache {
    cache_path()
        .and_then(|path| fs::read(path).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &Cache) {
    if let Some(path) = cache_path()
        && let Ok(data) = serde_json::to_vec(cache)
    {
        let _ = storage::write_atomic(&path, &data);
    }
}

fn fresh(cache: &Cache, time: u64) -> bool {
    let interval = if cache.failed {
        RETRY_INTERVAL
    } else {
        CHECK_INTERVAL
    };
    cache.checked_at != 0 && time >= cache.checked_at && time - cache.checked_at < interval
}

fn disabled() -> bool {
    std::env::var_os("CPPM_NO_UPDATE_CHECK").is_some_and(|v| v != "0" && !v.is_empty())
        || std::env::var_os("CI").is_some_and(|v| v != "false" && v != "0" && !v.is_empty())
}

pub fn notice() -> Option<String> {
    if disabled() {
        return None;
    }
    let latest = read_cache().latest?;
    newer(&latest)
        .then(|| format!("Atualização disponível: {CURRENT} → {latest}. Execute `cppm update`."))
}

/// A separate, silent process refreshes the cache even when the command is short.
/// No network operation is allowed to delay a normal cppm command.
pub fn start(offline: bool, interactive: bool) {
    if offline || disabled() || !io::stderr().is_terminal() || !io::stdout().is_terminal() {
        return;
    }
    if !interactive && let Some(message) = notice() {
        eprintln!("cppm: {message}");
    }
    if fresh(&read_cache(), now()) {
        return;
    }
    if let Ok(executable) = std::env::current_exe() {
        let mut command = Command::new(executable);
        command
            .arg("__update-check")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        if let Ok(mut child) = command.spawn() {
            // Reap the worker during long builds; short commands can exit immediately.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

fn fetch_latest() -> Result<String> {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("powershell.exe");
        command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]);
        command.arg(format!(
            "$ErrorActionPreference='Stop'; \
             [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; \
             $r=Invoke-WebRequest -UseBasicParsing -TimeoutSec 3 -Uri '{VERSION_URL}'; \
             if ($r.Content -is [byte[]]) {{ [Text.Encoding]::UTF8.GetString($r.Content) }} else {{ $r.Content }}"
        ));
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("curl");
        command.args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "2",
            "--max-time",
            "3",
            "--max-filesize",
            "1024",
            VERSION_URL,
        ]);
        command
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .context("cannot check GitHub releases; install curl on Linux or use Windows PowerShell")?;
    let deadline = std::time::Instant::now() + Duration::from_secs(6);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("GitHub release check timed out");
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "cannot check GitHub releases: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let latest = String::from_utf8(output.stdout).context("invalid release response")?;
    let latest = latest.trim();
    ensure!(
        version(latest).is_some(),
        "invalid release version from GitHub"
    );
    Ok(latest.into())
}

pub fn refresh_cache() {
    let mut cache = read_cache();
    // Concurrent terminal sessions can reuse the first completed check.
    if fresh(&cache, now()) {
        return;
    }
    cache.checked_at = now();
    match fetch_latest() {
        Ok(latest) => {
            cache.latest = Some(latest);
            cache.failed = false;
        }
        Err(_) => {
            cache.failed = true;
        }
    }
    save_cache(&cache);
}

pub fn run(check_only: bool) -> Result<()> {
    println!("Consultando a última versão do cppm no GitHub...");
    let latest = fetch_latest()?;
    save_cache(&Cache {
        checked_at: now(),
        latest: Some(latest.clone()),
        failed: false,
    });
    if !newer(&latest) {
        println!("cppm {CURRENT} já está atualizado (release: {latest}).");
        return Ok(());
    }
    if check_only {
        println!("Atualização disponível: {CURRENT} → {latest}. Execute `cppm update`.");
        return Ok(());
    }
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .context("cannot find the cppm installation directory")?;
    let expected_name = if cfg!(windows) { "cppm.exe" } else { "cppm" };
    ensure!(
        executable
            .file_name()
            .is_some_and(|name| name == expected_name),
        "self-update requires the executable to be named {expected_name}"
    );
    let temporary = tempfile::tempdir()?;
    #[cfg(windows)]
    let mut command = {
        let script = temporary.path().join("bootstrap.ps1");
        fs::write(&script, include_str!("../scripts/bootstrap.ps1"))?;
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
            .arg(script)
            .arg("-Version")
            .arg(&latest)
            .arg("-InstallDir")
            .arg(directory)
            .arg("-NoPath");
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let script = temporary.path().join("bootstrap.sh");
        fs::write(&script, include_str!("../scripts/bootstrap.sh"))?;
        let mut command = Command::new("sh");
        command
            .arg(script)
            .arg("--version")
            .arg(&latest)
            .arg("--bin-dir")
            .arg(directory);
        command
    };
    let status = command
        .status()
        .context("cannot start the release installer")?;
    ensure!(
        status.success(),
        "update failed; the installer exited with {status}"
    );
    println!("cppm atualizado para {latest}. Os próximos comandos usarão a nova versão.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_versions_are_compared_numerically_and_reject_unsafe_input() {
        assert!(version("0.10.0") > version("0.9.9"));
        assert!(newer("999.0.0"));
        assert!(!newer(CURRENT));
        assert!(!newer("0.0.1"));
        for invalid in [
            "v1.2.3",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.3-rc.1",
            "1.2.3\n",
            "1.2.$(id)",
            "../1.2.3",
        ] {
            assert_eq!(version(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn cache_expiration_retries_failures_and_handles_future_timestamps() {
        let mut cache = Cache {
            checked_at: 100,
            latest: None,
            failed: false,
        };
        assert!(fresh(&cache, 100));
        assert!(!fresh(&cache, 99));
        assert!(!fresh(&cache, 100 + CHECK_INTERVAL));
        cache.failed = true;
        assert!(!fresh(&cache, 100 + RETRY_INTERVAL));
        assert!(fresh(&cache, 100 + RETRY_INTERVAL - 1));
        assert!(!fresh(&Cache::default(), 1));
    }
}

use anyhow::{Context, Result};
use std::{ffi::OsStr, fs, io::ErrorKind, path::Path};

pub struct Fingerprint(blake3::Hasher);

impl Fingerprint {
    pub fn new(domain: &str) -> Self {
        let mut value = Self(blake3::Hasher::new());
        value.add(domain.as_bytes());
        value.add(env!("CARGO_PKG_VERSION").as_bytes());
        value
    }

    pub fn add(&mut self, bytes: &[u8]) {
        self.0.update(&(bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }

    pub fn os(&mut self, value: &OsStr) {
        self.add(value.as_encoded_bytes());
    }

    pub fn file(&mut self, path: &Path) -> Result<()> {
        self.os(path.as_os_str());
        match fs::read(path) {
            Ok(bytes) => {
                self.add(b"file");
                self.add(&bytes);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => self.add(b"missing"),
            Err(error) => {
                return Err(error).with_context(|| format!("cannot read {}", path.display()));
            }
        }
        Ok(())
    }

    pub fn directory(&mut self, path: &Path, excluded: &str) -> Result<()> {
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                self.add(b"missing directory");
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let mut entries = entries.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if entry.file_name() == excluded {
                continue;
            }
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                self.os(path.as_os_str());
                self.directory(&path, excluded)?;
            } else {
                self.file(&path)?;
            }
        }
        Ok(())
    }

    pub fn environment(&mut self) -> Result<()> {
        let executable = std::env::current_exe()?;
        let mut environment: Vec<_> = std::env::vars_os().collect();
        environment.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, value) in environment {
            // These only describe the calling shell/CLI. Profile data and the
            // selected Conan home are included separately by callers.
            if matches!(
                name.to_str(),
                Some(
                    "PWD"
                        | "OLDPWD"
                        | "SHLVL"
                        | "_"
                        | "CONAN_HOME"
                        | "CPPM_SCRIPT_STACK"
                        | "CPPM_PROFILE"
                        | "CPPM_STORE_DIR"
                        | "CPPM_OFFLINE"
                        | "CPPM_FROZEN_LOCKFILE"
                        | "CPPM_VERBOSE"
                        | "CPPM_WATCH_ACTIVE"
                        | "EMSDK_QUIET"
                )
            ) {
                continue;
            }
            self.os(&name);
            if name.to_string_lossy().eq_ignore_ascii_case("PATH") {
                // `cppm run` prepends this directory so scripts use the same
                // executable. It must not invalidate an otherwise identical build.
                for path in std::env::split_paths(&value) {
                    if Some(path.as_path()) != executable.parent() {
                        self.os(path.as_os_str());
                    }
                }
            } else {
                self.os(&value);
            }
        }
        Ok(())
    }

    pub fn finish(self) -> String {
        self.0.finalize().to_hex().to_string()
    }
}

use crate::project::Project;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, ffi::OsString, fs, process::Command};

#[derive(Deserialize)]
struct Environment {
    format: u32,
    variables: BTreeMap<String, Option<String>>,
}

pub fn configure(
    command: &mut Command,
    project: &Project,
    release: bool,
    verbose: bool,
) -> Result<()> {
    if project.manifest.dependencies.is_empty() {
        return Ok(());
    }
    let path = project
        .root
        .join(".cppm/conan")
        .join(if release { "release" } else { "debug" })
        .join("cppm-runtime.json");
    let environment: Environment = serde_json::from_slice(
        &fs::read(&path)
            .with_context(|| format!("cannot read {}; run `cppm install`", path.display()))?,
    )?;
    ensure!(
        environment.format == 1,
        "unsupported runtime environment format"
    );
    apply(command, &environment.variables);
    if project.manifest.dependencies.contains_key("opencv") {
        opencv_logging(command, std::env::var_os("OPENCV_LOG_LEVEL"), verbose);
    }
    Ok(())
}

fn opencv_logging(command: &mut Command, inherited: Option<OsString>, verbose: bool) {
    // Explicit choices from the user, recipe or profile take precedence. This
    // only changes the child process; application stdout and diagnostics remain
    // untouched, and the default still includes OpenCV warnings and errors.
    let explicit = command.get_envs().any(|(key, _)| {
        if cfg!(windows) {
            key.to_string_lossy()
                .eq_ignore_ascii_case("OPENCV_LOG_LEVEL")
        } else {
            key == "OPENCV_LOG_LEVEL"
        }
    });
    if !explicit && inherited.is_none() {
        command.env("OPENCV_LOG_LEVEL", if verbose { "INFO" } else { "WARNING" });
    }
}

fn apply(command: &mut Command, variables: &BTreeMap<String, Option<String>>) {
    // Expand only Conan's explicit inheritance marker. No shell evaluation:
    // spaces, Unicode and metacharacters in paths remain literal arguments.
    for (name, value) in variables {
        let Some(value) = value else {
            command.env_remove(name);
            continue;
        };
        let inherited = command
            .get_envs()
            .find(|(key, _)| {
                if cfg!(windows) {
                    key.to_string_lossy().eq_ignore_ascii_case(name)
                } else {
                    *key == name.as_str()
                }
            })
            .map(|(_, value)| value.map(OsString::from))
            .unwrap_or_else(|| std::env::var_os(name));
        let marker = format!("__CPPM_INHERIT_{name}__");
        let mut resolved = OsString::new();
        for (index, part) in value.split(&marker).enumerate() {
            if index > 0
                && let Some(inherited) = &inherited
            {
                resolved.push(inherited);
            }
            resolved.push(part);
        }
        command.env(name, resolved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_environment_respects_the_command_and_preserves_literal_values() {
        let mut command = Command::new("unused");
        command.env("PATH", "compiler path").env("REMOVE_ME", "old");
        apply(
            &mut command,
            &BTreeMap::from([
                (
                    "PATH".into(),
                    Some("C:/Qt & á/bin;__CPPM_INHERIT_PATH__".into()),
                ),
                (
                    "QT_PLUGIN_PATH".into(),
                    Some("C:/Qt & á/plugins/$literal".into()),
                ),
                ("REMOVE_ME".into(), None),
            ]),
        );
        let values: BTreeMap<_, _> = command.get_envs().collect();
        assert_eq!(
            values[std::ffi::OsStr::new("PATH")].unwrap(),
            "C:/Qt & á/bin;compiler path"
        );
        assert_eq!(
            values[std::ffi::OsStr::new("QT_PLUGIN_PATH")].unwrap(),
            "C:/Qt & á/plugins/$literal"
        );
        assert_eq!(values[std::ffi::OsStr::new("REMOVE_ME")], None);
    }

    #[test]
    fn opencv_logging_defaults_keep_diagnostics_and_respect_explicit_choices() {
        let value = |command: &Command| {
            command
                .get_envs()
                .find(|(key, _)| *key == "OPENCV_LOG_LEVEL")
                .map(|(_, value)| value.map(OsString::from))
        };
        let mut command = Command::new("unused");
        opencv_logging(&mut command, None, false);
        assert_eq!(value(&command), Some(Some("WARNING".into())));
        let mut command = Command::new("unused");
        opencv_logging(&mut command, None, true);
        assert_eq!(value(&command), Some(Some("INFO".into())));
        let mut command = Command::new("unused");
        command.env("OPENCV_LOG_LEVEL", "DEBUG");
        opencv_logging(&mut command, None, false);
        assert_eq!(value(&command), Some(Some("DEBUG".into())));
        let mut command = Command::new("unused");
        opencv_logging(&mut command, Some("DEBUG".into()), false);
        assert_eq!(value(&command), None);
        command.env_remove("OPENCV_LOG_LEVEL");
        opencv_logging(&mut command, None, false);
        assert_eq!(value(&command), Some(None));
    }
}

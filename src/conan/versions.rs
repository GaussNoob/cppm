//! Which versions of the direct dependencies exist, for `outdated`/`upgrade`.

use super::InstallOptions;
use crate::{manifest::validate_version, project::Project, store};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;

/// Splits a version into its release identifiers and pre-release suffix.
/// Build metadata after `+` does not take part in ordering.
fn parts(version: &str) -> (Vec<&str>, Option<&str>) {
    let version = version.split_once('+').map_or(version, |(main, _)| main);
    let (main, pre) = version
        .split_once('-')
        .map_or((version, None), |(main, pre)| (main, Some(pre)));
    (main.split('.').collect(), pre)
}

fn compare_identifier(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        // "9b" sorts after "9": compare numeric prefixes before the suffix.
        _ => {
            let split = |value: &str| {
                let digits = value.bytes().take_while(u8::is_ascii_digit).count();
                (
                    value[..digits].parse::<u64>().ok(),
                    value[digits..].to_owned(),
                )
            };
            split(a).cmp(&split(b))
        }
    }
}

/// Orders versions the way Conan does: numeric identifiers compare as
/// numbers, missing identifiers count as zero and a pre-release sorts before
/// its release.
pub fn compare(a: &str, b: &str) -> Ordering {
    let ((a_main, a_pre), (b_main, b_pre)) = (parts(a), parts(b));
    for index in 0..a_main.len().max(b_main.len()) {
        let ordering = compare_identifier(
            a_main.get(index).copied().unwrap_or("0"),
            b_main.get(index).copied().unwrap_or("0"),
        );
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    match (a_pre, b_pre) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(a), Some(b)) => {
            let (mut a, mut b) = (a.split('.'), b.split('.'));
            loop {
                match (a.next(), b.next()) {
                    (None, None) => return Ordering::Equal,
                    (None, Some(_)) => return Ordering::Less,
                    (Some(_), None) => return Ordering::Greater,
                    (Some(a), Some(b)) => match compare_identifier(a, b) {
                        Ordering::Equal => {}
                        ordering => return ordering,
                    },
                }
            }
        }
    }
}

fn numeric(version: &str, index: usize) -> Option<u64> {
    parts(version).0.get(index)?.parse().ok()
}

/// Caret compatibility, as in `^1.2.3`: the leftmost nonzero identifier of
/// the current version must not change.
fn compatible(current: &str, candidate: &str) -> bool {
    let Some(major) = numeric(current, 0) else {
        return false;
    };
    if numeric(candidate, 0) != Some(major) {
        return false;
    }
    if major > 0 {
        return true;
    }
    let minor = numeric(current, 1);
    numeric(candidate, 1) == minor
        && (minor != Some(0) || numeric(candidate, 2) == numeric(current, 2))
}

/// The newest candidate above `current`, optionally within its major version.
/// Pre-releases are only offered to projects already using one, and recipes
/// with date-like names (`cci.20240101`) never replace numbered releases.
fn newest<'a>(current: &str, candidates: &'a [String], same_major: bool) -> Option<&'a str> {
    let prerelease = parts(current).1.is_some();
    candidates
        .iter()
        .map(String::as_str)
        .filter(|candidate| numeric(candidate, 0).is_some() == numeric(current, 0).is_some())
        .filter(|candidate| prerelease || parts(candidate).1.is_none())
        .filter(|candidate| !same_major || compatible(current, candidate))
        .filter(|candidate| compare(candidate, current) == Ordering::Greater)
        .max_by(|a, b| compare(a, b))
}

/// Versions of one recipe in the local cache and, unless offline, the remotes.
fn available(name: &str, options: &InstallOptions) -> Result<Vec<String>> {
    let query = |remotes: bool| -> Result<Option<Value>> {
        let mut command = crate::platform::command("conan");
        store::configure(&mut command, options);
        command
            .arg("list")
            .arg(format!("{name}/*"))
            .arg("--format=json");
        if remotes {
            command.args(["--cache", "--remote=*"]);
        }
        let output = command.output().context(
            "cannot start Conan 2; install it with `pipx install conan` or `uv tool install conan`",
        )?;
        Ok(output
            .status
            .success()
            .then(|| serde_json::from_slice(&output.stdout).ok())
            .flatten())
    };
    // Conan rejects a remote query when every remote is disabled; the local
    // cache still answers, exactly as it does with --offline.
    let listing = match if options.offline { None } else { query(true)? } {
        Some(listing) => listing,
        None => query(false)?
            .with_context(|| format!("Conan could not list the versions of '{name}'"))?,
    };
    let mut versions: Vec<String> = listing
        .as_object()
        .into_iter()
        .flatten()
        // An unreachable remote reports {"error": ...}; other sources still count.
        .filter_map(|(_, recipes)| recipes.as_object())
        .flat_map(|recipes| recipes.keys())
        .filter_map(|reference| reference.strip_prefix(name)?.strip_prefix('/'))
        // name/version@user/channel is a different package identity.
        .filter(|version| !version.contains('@') && validate_version(version).is_ok())
        .map(str::to_owned)
        .collect();
    versions.sort_by(|a, b| compare(a, b));
    versions.dedup();
    Ok(versions)
}

#[derive(Serialize)]
pub struct Entry {
    pub name: String,
    pub current: String,
    /// Newest version within the current major version, if any is newer.
    pub wanted: Option<String>,
    /// Newest version overall, if any is newer.
    pub latest: Option<String>,
    pub dev: bool,
}

/// Compares every direct dependency with the versions Conan can provide.
pub fn report(project: &Project, options: &InstallOptions) -> Result<Vec<Entry>> {
    let manifest = &project.manifest;
    let direct: Vec<_> = manifest
        .dependencies
        .iter()
        .map(|entry| (entry, false))
        .chain(manifest.dev_dependencies.iter().map(|entry| (entry, true)))
        .collect();
    // Each query starts Conan and may wait for a remote; ask a few at a time.
    let mut entries = Vec::with_capacity(direct.len());
    for batch in direct.chunks(8) {
        let results: Vec<Result<Vec<String>>> = std::thread::scope(|scope| {
            let queries: Vec<_> = batch
                .iter()
                .map(|((name, _), _)| scope.spawn(move || available(name, options)))
                .collect();
            queries
                .into_iter()
                .map(|query| query.join().expect("version query panicked"))
                .collect()
        });
        for (((name, dependency), dev), versions) in batch.iter().zip(results) {
            let versions = versions?;
            let current = dependency.version();
            entries.push(Entry {
                name: (*name).clone(),
                current: current.to_owned(),
                wanted: newest(current, &versions, true).map(str::to_owned),
                latest: newest(current, &versions, false).map(str::to_owned),
                dev: *dev,
            });
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_order_numerically_with_suffixes_and_prereleases() {
        let mut versions = vec![
            "1.10.0",
            "1.9.0",
            "1.92.9b",
            "1.92.9",
            "2.0.0-rc.1",
            "2.0.0",
            "1.2",
            "1.2.1",
            "2.0.0-rc.2",
            "2.0.0-beta",
        ];
        versions.sort_by(|a, b| compare(a, b));
        assert_eq!(
            versions,
            [
                "1.2",
                "1.2.1",
                "1.9.0",
                "1.10.0",
                "1.92.9",
                "1.92.9b",
                "2.0.0-beta",
                "2.0.0-rc.1",
                "2.0.0-rc.2",
                "2.0.0"
            ]
        );
        assert_eq!(compare("1.2", "1.2.0"), Ordering::Equal);
        assert_eq!(compare("3.4.16+build", "3.4.16"), Ordering::Equal);
        assert_eq!(compare("2023.1.0", "2022.12.0"), Ordering::Greater);
    }

    #[test]
    fn upgrades_stay_in_the_major_version_unless_latest_is_requested() {
        let candidates: Vec<String> = [
            "2.30.0",
            "2.32.10",
            "3.4.14",
            "3.4.16",
            "3.5.0-rc.1",
            "cci.20240101",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(newest("2.30.0", &candidates, true), Some("2.32.10"));
        assert_eq!(newest("2.30.0", &candidates, false), Some("3.4.16"));
        assert_eq!(newest("2.32.10", &candidates, true), None);
        assert_eq!(newest("3.4.16", &candidates, false), None);
        // A project already on a pre-release may move to a newer one.
        assert_eq!(newest("3.5.0-beta", &candidates, false), Some("3.5.0-rc.1"));
        // Zero-major versions treat the minor version as breaking.
        let zero: Vec<String> = ["0.62.0", "0.62.3", "0.63.0"].map(String::from).to_vec();
        assert_eq!(newest("0.62.0", &zero, true), Some("0.62.3"));
        assert_eq!(newest("0.62.0", &zero, false), Some("0.63.0"));
        // Date-named recipes are only compared with each other.
        let dated: Vec<String> = ["cci.20230101", "cci.20240101", "1.0"]
            .map(String::from)
            .to_vec();
        assert_eq!(newest("1.0", &dated, false), None);
    }
}

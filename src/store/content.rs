use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs::{self, File, Metadata},
    path::{Path, PathBuf},
};

// Package payloads are immutable to consumers, as required by Conan. The
// executable/read permission bits are part of the identity; symlinks retain
// their original targets and are never followed or pooled.
fn mode(metadata: &Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.mode() & 0o7555
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        0
    }
}

fn hash(path: &Path) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(path)?)?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn object(pool: &Path, digest: &str, mode: u32) -> PathBuf {
    pool.join(&digest[..2]).join(format!("{digest}-{mode:03o}"))
}

pub(super) fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            paths.extend(files(&path)?);
        } else if metadata.is_file() {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

#[derive(Default, Serialize)]
pub(super) struct DedupeReport {
    pub files: usize,
    pub linked_files: usize,
    pub skipped_files: usize,
    pub saved_bytes: u64,
}

pub(super) fn dedupe<'a>(
    home: &Path,
    packages: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<DedupeReport> {
    let pool = home.join("cppm-store/files");
    let mut report = DedupeReport::default();
    let mut verified = BTreeSet::new();
    for package in packages {
        if !package.try_exists()? {
            continue;
        }
        for path in files(package)? {
            let metadata = fs::metadata(&path)?;
            report.files += 1;
            let digest = hash(&path)?;
            let object = object(&pool, &digest, mode(&metadata));
            fs::create_dir_all(object.parent().unwrap())?;
            let created = match fs::hard_link(&path, &object) {
                Ok(()) => true,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
                // Some filesystems do not support hard links, or the package
                // storage is on a different volume. Keep the original file.
                Err(_) => {
                    report.skipped_files += 1;
                    continue;
                }
            };
            if verified.insert(object.clone()) {
                let object_metadata = fs::symlink_metadata(&object)?;
                ensure!(
                    object_metadata.is_file()
                        && mode(&object_metadata) == mode(&metadata)
                        && (created || hash(&object)? == digest),
                    "shared file {} failed its content check; reinstall the affected package",
                    object.display()
                );
                let mut permissions = object_metadata.permissions();
                permissions.set_readonly(true);
                fs::set_permissions(&object, permissions)?;
            }
            if created || same_file::is_same_file(&path, &object)? {
                continue;
            }
            // Prepare the link beside its destination and atomically replace
            // the old name. A failed rename leaves the original intact.
            let staging = tempfile::tempdir_in(path.parent().unwrap())?;
            let link = staging.path().join("file");
            if fs::hard_link(&object, &link).is_err() {
                report.skipped_files += 1;
                continue;
            }
            if fs::rename(&link, &path).is_err() {
                remove_file(&link)?;
                // Windows needs a writable attribute to unlink a read-only
                // file, and that attribute is shared by all hard links.
                let mut permissions = fs::metadata(&object)?.permissions();
                permissions.set_readonly(true);
                fs::set_permissions(&object, permissions)?;
                report.skipped_files += 1;
                continue;
            }
            report.linked_files += 1;
            report.saved_bytes += metadata.len();
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_files_share_storage_and_survive_other_package_removal() {
        let temp = tempfile::tempdir().unwrap();
        let packages = [temp.path().join("v1"), temp.path().join("v2")];
        for path in &packages {
            fs::create_dir(path).unwrap();
            fs::write(path.join("common.h"), "common header\n").unwrap();
        }
        fs::write(packages[0].join("changed.h"), "old header\n").unwrap();
        fs::write(packages[1].join("changed.h"), "new header\n").unwrap();
        let report = dedupe(temp.path(), &packages).unwrap();
        assert_eq!(report.linked_files, 1);
        assert_eq!(report.saved_bytes, 14);
        assert!(
            same_file::is_same_file(packages[0].join("common.h"), packages[1].join("common.h"))
                .unwrap()
        );
        assert!(
            !same_file::is_same_file(packages[0].join("changed.h"), packages[1].join("changed.h"))
                .unwrap()
        );
        assert!(
            fs::metadata(packages[0].join("common.h"))
                .unwrap()
                .permissions()
                .readonly()
        );
        assert_eq!(
            stats(temp.path(), &packages).unwrap().deduplicated_bytes,
            14
        );
        assert_eq!(dedupe(temp.path(), &packages).unwrap().saved_bytes, 0);
        // On Windows Conan makes files writable before removing their names.
        // That must not alter the shared bytes in the surviving package.
        for file in files(&packages[0]).unwrap() {
            remove_file(&file).unwrap();
        }
        fs::remove_dir(&packages[0]).unwrap();
        assert_eq!(prune(temp.path(), [&packages[1]], false).unwrap(), 1);
        assert_eq!(
            fs::read_to_string(packages[1].join("common.h")).unwrap(),
            "common header\n"
        );
        assert_eq!(prune(temp.path(), [&packages[1]], false).unwrap(), 0);
        for file in files(&packages[1]).unwrap() {
            remove_file(&file).unwrap();
        }
        assert_eq!(
            prune(temp.path(), std::iter::empty::<&PathBuf>(), true).unwrap(),
            2
        );
    }

    #[test]
    #[cfg(unix)]
    fn executable_permissions_and_symlinks_keep_their_meaning() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("package");
        fs::create_dir(&package).unwrap();
        for (file, mode) in [("tool", 0o755), ("data", 0o644)] {
            fs::write(package.join(file), "identical bytes").unwrap();
            fs::set_permissions(package.join(file), fs::Permissions::from_mode(mode)).unwrap();
        }
        let external = temp.path().join("outside");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("secret"), "outside package").unwrap();
        symlink(&external, package.join("external")).unwrap();
        symlink("tool", package.join("alias")).unwrap();
        dedupe(temp.path(), [&package]).unwrap();
        assert!(!same_file::is_same_file(package.join("tool"), package.join("data")).unwrap());
        assert_eq!(
            fs::metadata(package.join("tool"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o555
        );
        assert_eq!(
            fs::read_link(package.join("alias")).unwrap(),
            PathBuf::from("tool")
        );
        assert_eq!(
            fs::metadata(external.join("secret"))
                .unwrap()
                .permissions()
                .mode()
                & 0o200,
            0o200
        );
    }

    #[test]
    fn a_corrupt_shared_object_is_rejected_before_linking_another_package() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("package");
        fs::create_dir(&package).unwrap();
        let path = package.join("header");
        fs::write(&path, "expected").unwrap();
        let object = object(
            &temp.path().join("cppm-store/files"),
            &hash(&path).unwrap(),
            mode(&fs::metadata(&path).unwrap()),
        );
        fs::create_dir_all(object.parent().unwrap()).unwrap();
        fs::write(object, "corrupt!").unwrap();
        assert!(dedupe(temp.path(), [&package]).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "expected");
    }
}

#[derive(Default, Serialize)]
pub(super) struct ContentStats {
    pub logical_bytes: u64,
    pub unique_bytes: u64,
    pub deduplicated_bytes: u64,
    pub shared_files: usize,
}

#[cfg(not(unix))]
fn used_objects<'a>(
    home: &Path,
    packages: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<(BTreeSet<PathBuf>, ContentStats)> {
    let pool = home.join("cppm-store/files");
    let mut used = BTreeSet::new();
    let mut stats = ContentStats::default();
    for package in packages {
        if !package.try_exists()? {
            continue;
        }
        for path in files(package)? {
            let metadata = fs::metadata(&path)?;
            stats.logical_bytes += metadata.len();
            let object = object(&pool, &hash(&path)?, mode(&metadata));
            if object.try_exists()? && same_file::is_same_file(&path, &object)? {
                stats.shared_files += 1;
                if used.insert(object) {
                    stats.unique_bytes += metadata.len();
                }
            } else {
                stats.unique_bytes += metadata.len();
            }
        }
    }
    stats.deduplicated_bytes = stats.logical_bytes - stats.unique_bytes;
    Ok((used, stats))
}

pub(super) fn stats<'a>(
    home: &Path,
    packages: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<ContentStats> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let _ = home;
        let mut inodes = BTreeSet::new();
        let mut stats = ContentStats::default();
        for package in packages {
            if !package.try_exists()? {
                continue;
            }
            for file in files(package)? {
                let metadata = fs::metadata(file)?;
                stats.logical_bytes += metadata.len();
                if inodes.insert((metadata.dev(), metadata.ino())) {
                    stats.unique_bytes += metadata.len();
                }
                if metadata.nlink() > 1 {
                    stats.shared_files += 1;
                }
            }
        }
        stats.deduplicated_bytes = stats.logical_bytes - stats.unique_bytes;
        Ok(stats)
    }
    #[cfg(not(unix))]
    {
        Ok(used_objects(home, packages)?.1)
    }
}

pub(super) fn prune<'a>(
    home: &Path,
    packages: impl IntoIterator<Item = &'a PathBuf>,
    dry_run: bool,
) -> Result<usize> {
    let pool = home.join("cppm-store/files");
    if !pool.try_exists()? {
        return Ok(0);
    }
    #[cfg(not(unix))]
    let used = used_objects(home, packages)?.0;
    #[cfg(unix)]
    let _ = packages;
    let mut removed = 0;
    for path in files(&pool)? {
        #[cfg(unix)]
        let unused = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&path)?.nlink() == 1
        };
        #[cfg(not(unix))]
        let unused = !used.contains(&path);
        if unused {
            if !dry_run {
                remove_file(&path).with_context(|| {
                    format!("cannot remove unused shared file {}", path.display())
                })?;
            }
            removed += 1;
        } else if !dry_run {
            let mut permissions = fs::metadata(&path)?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(path, permissions)?;
        }
    }
    Ok(removed)
}

fn remove_file(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    fs::remove_file(path)?;
    Ok(())
}

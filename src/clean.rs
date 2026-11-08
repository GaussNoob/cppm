use crate::{build, editor, project::Project};
use anyhow::{Result, ensure};
use clap::ValueEnum;
use std::{fs, io::ErrorKind, path::Path};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Mode {
    #[default]
    All,
    Debug,
    Release,
    Obf,
}

pub fn run(project: &Project, mode: Mode, dry_run: bool) -> Result<()> {
    for path in [project.root.join(".cppm"), project.root.join(".cppm/build")] {
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "cannot clean through a symbolic link: {}",
                path.display()
            );
        }
    }
    let (project, _guard) = project.locked()?;
    let directory = match mode {
        Mode::All => project.root.join(".cppm/build"),
        Mode::Debug => build::directory(&project, false),
        Mode::Release => build::directory(&project, true),
        Mode::Obf => project.root.join(".cppm/build/obf"),
    };
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            println!("No build artifacts to clean.");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let (files, bytes) = size(&directory)?;
    if dry_run {
        println!(
            "Would remove {} ({files} files, {bytes} bytes).",
            directory.display()
        );
    } else {
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(&directory)?;
        } else {
            fs::remove_file(&directory)?;
        }
        editor::unpublish(&project.root, &directory)?;
        println!(
            "Removed {} ({files} files, {bytes} bytes).",
            directory.display()
        );
    }
    Ok(())
}

fn size(path: &Path) -> Result<(u64, u64)> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Ok((1, metadata.len()));
    }
    let mut result = (0, 0);
    for entry in fs::read_dir(path)? {
        let (files, bytes) = size(&entry?.path())?;
        result.0 += files;
        result.1 += bytes;
    }
    Ok(result)
}

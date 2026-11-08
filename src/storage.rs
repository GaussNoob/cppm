use anyhow::{Context, Result};
use std::{fs, io::Write, path::Path};
use tempfile::NamedTempFile;

pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("file has no parent directory")?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

pub fn write_if_changed(path: &Path, contents: &[u8]) -> Result<()> {
    if fs::read(path).ok().as_deref() != Some(contents) {
        write_atomic(path, contents)?;
    }
    Ok(())
}

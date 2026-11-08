use crate::{
    manifest::{FILE_NAME, Manifest, valid_name},
    storage::write_atomic,
    templates::{self, Template},
};
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
}

impl Project {
    pub fn discover(start: &Path) -> Result<Self> {
        let root = start
            .ancestors()
            .find(|p| p.join(FILE_NAME).is_file())
            .context("no cppm.toml found; create a project with `cppm init`")?
            .to_path_buf();
        Ok(Self {
            manifest: Manifest::load(&root)?,
            root,
        })
    }

    pub fn lock(&self) -> Result<File> {
        fs::create_dir_all(self.root.join(".cppm"))?;
        let file = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join(".cppm/project.lock"))?;
        file.try_lock()
            .context("another cppm command is modifying this project; wait for it to finish")?;
        Ok(file)
    }

    pub fn locked(&self) -> Result<(Self, File)> {
        let guard = self.lock()?;
        // Reload after acquiring the lock so a concurrent completed command
        // cannot leave this operation with a stale manifest.
        let project = Self {
            root: self.root.clone(),
            manifest: Manifest::load(&self.root)?,
        };
        Ok((project, guard))
    }

    pub fn save_manifest(&self) -> Result<()> {
        self.manifest.validate()?;
        write_atomic(
            &self.root.join(FILE_NAME),
            toml::to_string_pretty(&self.manifest)?.as_bytes(),
        )
    }
}

pub fn init(path: &Path, name: Option<String>, template: Template) -> Result<()> {
    let root = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    // Canonicalize the directory before deriving a name so `init .` works.
    fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let name = name
        .or_else(|| root.file_name().map(|n| n.to_string_lossy().into_owned()))
        .context("cannot infer project name; use --name")?;
    ensure!(
        valid_name(&name),
        "invalid project name '{name}'; use --name with letters, digits, '_' or '-'"
    );
    let scaffold = templates::scaffold(name.clone(), template);
    let project = Project {
        root,
        manifest: scaffold.manifest,
    };
    let _guard = project.lock()?;
    for file in std::iter::once(FILE_NAME).chain(scaffold.files.keys().copied()) {
        ensure!(
            !project.root.join(file).exists(),
            "{file} already exists; init will not overwrite it"
        );
    }
    for (file, source) in scaffold.files {
        write_atomic(&project.root.join(file), source.as_bytes())?;
    }
    let ignore = project.root.join(".gitignore");
    let mut contents = match fs::read_to_string(&ignore) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    if !contents.lines().any(|line| line == "/.cppm/") {
        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push_str("/.cppm/\n");
        write_atomic(&ignore, contents.as_bytes())?;
    }
    project.save_manifest()?;
    println!("Created {name} in {}", project.root.display());
    if matches!(template, Template::Wasm) {
        println!(
            "Installed Emscripten SDKs are detected automatically; check yours with `cppm doctor`."
        );
    }
    println!("Run `cppm run dev` from that directory.");
    Ok(())
}

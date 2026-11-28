//! An export on disk: written only into a directory that holds nothing, and read back whole.
use super::manifest::ExportFile;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

/// Whether `dir` is absent or an empty directory, so that an export never mixes with other files or replaces them.
fn is_free(dir: &Path) -> Result<bool> {
    match fs::read_dir(dir) {
        Ok(mut entries) => Ok(entries.next().is_none()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error).with_context(|| format!("{} cannot be used", dir.display())),
    }
}

/// Writes `files` under `to`, which must be absent or empty. The files and the directories are readable by their
/// owner only, as the node's own data is. A failure part way takes back what was written.
pub fn write(files: &[ExportFile], to: &Path) -> Result<()> {
    if !is_free(to)? {
        bail!(
            "{} is not empty; export into a directory that holds nothing",
            to.display()
        );
    }
    let existed = to.is_dir();
    let mut made = Vec::new();
    let result = files
        .iter()
        .try_for_each(|file| write_one(to, file, &mut made));
    if result.is_err() {
        for path in made.iter().rev() {
            let _ = fs::remove_file(path).or_else(|_| fs::remove_dir(path));
        }
        if !existed {
            let _ = fs::remove_dir(to);
        }
    }
    result
}

fn write_one(to: &Path, file: &ExportFile, made: &mut Vec<std::path::PathBuf>) -> Result<()> {
    let path = to.join(&file.path);
    if let Some(parent) = path.parent() {
        let is_new = !parent.exists();
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .with_context(|| format!("{} cannot be made", parent.display()))?;
        if is_new && parent != to {
            made.push(parent.to_owned());
        }
    }
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| format!("{} cannot be written", path.display()))?;
    made.push(path.clone());
    out.write_all(&file.content)
        .and_then(|()| out.sync_all())
        .with_context(|| format!("{} cannot be written", path.display()))
}

/// Every file under `dir`, by its path from `dir` with `/`. Anything but a file or a directory (a link, a device)
/// is refused: an export holds nothing else.
pub fn read(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    read_into(dir, dir, &mut files)?;
    Ok(files)
}

fn read_into(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("{} cannot be read", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        let kind = fs::symlink_metadata(&path)?.file_type();
        if kind.is_dir() {
            read_into(root, &path, files)?;
        } else if kind.is_file() {
            let relative = path.strip_prefix(root)?;
            let name: Vec<String> = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.insert(name.join("/"), fs::read(&path)?);
        } else {
            bail!("{} is not a file", path.display());
        }
    }
    Ok(())
}

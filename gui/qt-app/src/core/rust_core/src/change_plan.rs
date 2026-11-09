//! Pure filesystem plan shared by Apply and its read-only preview.
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    File { bytes: Vec<u8>, mode: u32 },
    Directory { mode: u32 },
    Symlink { target: PathBuf },
    Missing,
}
#[derive(Default)]
pub struct Plan {
    pub before: BTreeMap<PathBuf, Entry>,
    pub after: BTreeMap<PathBuf, Entry>,
    pub reasons: BTreeMap<PathBuf, Vec<String>>,
    pub inputs: BTreeMap<String, String>,
}
pub fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
pub fn entry(path: &Path) -> Result<Entry> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Entry::Missing),
        Err(e) => return Err(e).with_context(|| format!("Read {}", path.display())),
    };
    Ok(if meta.file_type().is_symlink() {
        Entry::Symlink {
            target: fs::read_link(path)?,
        }
    } else if meta.is_dir() {
        Entry::Directory {
            mode: meta.permissions().mode() & 0o7777,
        }
    } else if meta.is_file() {
        Entry::File {
            bytes: fs::read(path)?,
            mode: meta.permissions().mode() & 0o7777,
        }
    } else {
        bail!("Unsupported file: {}", path.display())
    })
}
impl Plan {
    pub fn get(&mut self, path: &Path) -> Result<Entry> {
        if let Some(value) = self.after.get(path) {
            return Ok(value.clone());
        }
        let value = entry(path)?;
        self.inputs.insert(
            serde_json::to_string(&("entry", path))?,
            match &value {
                Entry::File { bytes, mode } => {
                    hash(format!("file:{mode}:{}", hash(bytes)).as_bytes())
                }
                _ => hash(&serde_json::to_vec(&value)?),
            },
        );
        Ok(value)
    }
    pub fn read(&mut self, path: &Path) -> Result<Option<Vec<u8>>> {
        let mut path = path.to_path_buf();
        for _ in 0..40 {
            match self.get(&path)? {
                Entry::File { bytes, .. } => return Ok(Some(bytes)),
                Entry::Symlink { target } => {
                    path = if target.is_absolute() {
                        target
                    } else {
                        path.parent().context("Link parent")?.join(target)
                    }
                }
                _ => return Ok(None),
            }
        }
        bail!("Too many symbolic links: {}", path.display())
    }
    pub fn text(&mut self, path: &Path) -> Result<Option<String>> {
        self.read(path)?
            .map(String::from_utf8)
            .transpose()
            .map_err(Into::into)
    }
    pub fn is_file(&mut self, path: &Path) -> Result<bool> {
        Ok(self.read(path)?.is_some())
    }
    pub fn put(&mut self, path: &Path, value: Entry, reason: &str) -> Result<()> {
        if !self.before.contains_key(path) {
            let old = self.get(path)?;
            self.before.insert(path.into(), old);
        }
        self.after.insert(path.into(), value);
        let reasons = self.reasons.entry(path.into()).or_default();
        if !reasons.iter().any(|r| r == reason) {
            reasons.push(reason.into());
        }
        Ok(())
    }
    pub fn ensure_parent(&mut self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if matches!(self.get(parent)?, Entry::Missing) {
                self.ensure_parent(parent)?;
                self.put(parent, Entry::Directory { mode: 0o755 }, "parent")?;
            }
        }
        Ok(())
    }
    pub fn remove_empty_parents(&mut self, path: &Path) -> Result<()> {
        let home = crate::paths::home_dir();
        let keep = [
            home.clone(),
            home.join(".config"),
            home.join(".local"),
            home.join(".local/share"),
        ];
        let mut parent = path.parent();
        while let Some(p) = parent {
            if !p.starts_with(&home) || keep.iter().any(|k| k == p) {
                break;
            }
            let mut empty = true;
            for child in self.children(p)? {
                if !matches!(self.get(&child)?, Entry::Missing) {
                    empty = false;
                    break;
                }
            }
            if !empty {
                break;
            }
            self.remove(p, "previous_theme")?;
            parent = p.parent();
        }
        Ok(())
    }
    pub fn write(&mut self, path: &Path, bytes: Vec<u8>, reason: &str) -> Result<()> {
        self.ensure_parent(path)?;
        let mode = match self.get(path)? {
            Entry::File { mode, .. } => mode,
            _ => 0o600,
        };
        self.put(path, Entry::File { bytes, mode }, reason)
    }
    pub fn write_text(&mut self, path: &Path, text: String, reason: &str) -> Result<()> {
        self.write(path, text.into_bytes(), reason)
    }
    pub fn remove(&mut self, path: &Path, reason: &str) -> Result<()> {
        self.put(path, Entry::Missing, reason)
    }
    pub fn children(&mut self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut paths = std::collections::BTreeSet::new();
        if path.is_dir() {
            for item in fs::read_dir(path)? {
                paths.insert(item?.path());
            }
        }
        for p in self.after.keys() {
            if p.parent() == Some(path) {
                paths.insert(p.clone());
            }
        }
        let listing: Vec<_> = paths.into_iter().collect();
        self.inputs.insert(
            serde_json::to_string(&("directory", path))?,
            hash(&serde_json::to_vec(&listing)?),
        );
        Ok(listing)
    }
    pub fn copy(&mut self, source: &Path, target: &Path, reason: &str) -> Result<()> {
        let value = self.get(source)?;
        let directory = matches!(value, Entry::Directory { .. });
        if matches!(value, Entry::Missing) {
            bail!("Source missing: {}", source.display());
        }
        self.ensure_parent(target)?;
        self.put(target, value, reason)?;
        if directory {
            for file in self.children(source)? {
                self.copy(
                    &file,
                    &target.join(file.file_name().context("File name")?),
                    reason,
                )?;
            }
        }
        Ok(())
    }
    pub fn track_tree(&mut self, path: &Path) -> Result<()> {
        if matches!(self.get(path)?, Entry::Directory { .. }) {
            for child in self.children(path)? {
                self.track_tree(&child)?;
            }
        }
        Ok(())
    }
    pub fn changed(&self) -> impl Iterator<Item = (&PathBuf, &Entry)> {
        self.after
            .iter()
            .filter(|(path, new)| self.before.get(*path) != Some(*new))
    }
    pub fn commit(&self) -> Result<()> {
        // Preserve the planning order of parent creation; file replacement is atomic.
        let mut removed: Vec<_> = self
            .changed()
            .filter(|(_, e)| matches!(e, Entry::Missing))
            .collect();
        removed.sort_by_key(|(p, _)| std::cmp::Reverse(p.components().count()));
        for (path, _) in removed {
            if fs::symlink_metadata(path).is_ok() {
                if path.is_dir() && !path.is_symlink() {
                    fs::remove_dir(path)?;
                } else {
                    fs::remove_file(path)?;
                }
            }
        }
        let mut entries: Vec<_> = self
            .changed()
            .filter(|(_, e)| !matches!(e, Entry::Missing))
            .collect();
        entries.sort_by_key(|(p, _)| p.components().count());
        for (path, value) in entries {
            match value {
                Entry::File { bytes, mode } => {
                    crate::gui_api::write_bytes(path, bytes, Some(*mode))?
                }
                Entry::Directory { mode } => {
                    fs::create_dir_all(path)?;
                    if self
                        .reasons
                        .get(path)
                        .is_none_or(|r| r.iter().any(|reason| reason != "parent"))
                    {
                        fs::set_permissions(path, fs::Permissions::from_mode(*mode))?;
                    }
                }
                Entry::Symlink { target } => {
                    if fs::symlink_metadata(path).is_ok() {
                        fs::remove_file(path)?;
                    }
                    fs::create_dir_all(path.parent().context("Link parent")?)?;
                    std::os::unix::fs::symlink(target, path)?;
                }
                Entry::Missing => {
                    if fs::symlink_metadata(path).is_ok() {
                        fs::remove_file(path)?;
                    }
                }
            }
        }
        Ok(())
    }
}

// Shared atomic file writes.
//
// Settings exports and database backups both need the same guarantee: a failed
// write leaves the previous good file untouched. The temporary file is claimed
// with `create_new` and a random suffix, and the final rename never removes the
// old target first. `std::fs::rename` maps to `MoveFileExW` with
// `MOVEFILE_REPLACE_EXISTING` on Windows and to `rename(2)` elsewhere, so it is
// an atomic replace on the same volume.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_TEMP_TRIES: usize = 16;
const RANDOM_BYTES: usize = 8;

/// A temporary file next to `target`, created with `create_new`. Dropping it
/// without `persist` removes the temporary file.
pub struct TempFile {
    path: PathBuf,
    file: Option<File>,
    persisted: bool,
}

impl TempFile {
    pub fn create(target: &Path) -> io::Result<Self> {
        let name = target
            .file_name()
            .ok_or_else(|| invalid_input("target path has no file name"))?
            .to_string_lossy()
            .into_owned();
        let parent = target.parent().unwrap_or_else(|| Path::new("."));
        for _ in 0..MAX_TEMP_TRIES {
            let path = parent.join(format!(".{name}.kaminari-{}.tmp", random_suffix()?));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                        persisted: false,
                    })
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique temporary file",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_mut(&mut self) -> io::Result<&mut File> {
        self.file
            .as_mut()
            .ok_or_else(|| invalid_input("temporary file is already closed"))
    }

    /// Closes the file handle while keeping the claimed path. Used by the
    /// backup path so SQLite can write into the reserved file.
    pub fn close(&mut self) {
        self.file.take();
    }

    /// Closes the file and atomically replaces `target` with it.
    pub fn persist(mut self, target: &Path) -> io::Result<()> {
        self.close();
        replace_file(&self.path, target)?;
        self.persisted = true;
        Ok(())
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.persisted {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Writes `bytes` to a temporary file and atomically replaces `target`.
pub fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temp = TempFile::create(target)?;
    {
        let file = temp.file_mut()?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    temp.persist(target)
}

/// Replaces `target` with `temp` without deleting the old target first.
/// A symlink or Windows reparse point at the target is rejected so a save
/// cannot be redirected to another path.
pub fn replace_file(temp: &Path, target: &Path) -> io::Result<()> {
    reject_link_or_reparse(target)?;
    std::fs::rename(temp, target)
}

fn reject_link_or_reparse(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if crate::path_guard::is_link_or_reparse(&metadata) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "target must not be a symlink or reparse point",
                ));
            }
            Ok(())
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn random_suffix() -> io::Result<String> {
    let mut bytes = [0u8; RANDOM_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|err| io::Error::other(format!("random source failed: {err}")))?;
    Ok(hex::encode(bytes))
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kaminari-atomic-{label}-{}-{}",
            std::process::id(),
            random_suffix().unwrap()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_atomic_replaces_an_existing_file() {
        let dir = temp_dir("replace");
        let target = dir.join("settings.json");
        std::fs::write(&target, b"old").unwrap();

        write_atomic(&target, b"new").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let leftovers = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".kaminari-"))
            .count();
        assert_eq!(leftovers, 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn temporary_names_are_unique_and_claimed_exclusively() {
        let dir = temp_dir("unique");
        let target = dir.join("backup.db");
        let first = TempFile::create(&target).unwrap();
        let second = TempFile::create(&target).unwrap();
        assert_ne!(first.path(), second.path());
        assert!(first.path().is_file());
        assert!(second.path().is_file());

        drop(first);
        drop(second);
        assert!(!target.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn failed_replace_keeps_the_old_target() {
        let dir = temp_dir("failed");
        let target = dir.join("target");
        std::fs::create_dir(&target).unwrap();
        let temp = dir.join("temp");
        std::fs::write(&temp, b"new").unwrap();

        assert!(replace_file(&temp, &target).is_err());
        assert!(target.is_dir());
        assert!(temp.is_file());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn replace_rejects_a_symlink_target() {
        let dir = temp_dir("symlink");
        let real = dir.join("real.json");
        let link = dir.join("link.json");
        std::fs::write(&real, b"old").unwrap();
        let temp = dir.join("temp");
        std::fs::write(&temp, b"new").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert!(replace_file(&temp, &link).is_err());
        assert_eq!(std::fs::read(&real).unwrap(), b"old");
        assert!(temp.is_file());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

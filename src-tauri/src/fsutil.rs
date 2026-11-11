// Shared filesystem helpers for writes that must not leave a half-written
// file at the destination path.

use std::path::Path;

/// Put `temp` at `target`, replacing an existing file. Windows cannot rename
/// over an open or existing file, so the fallback removes the target first.
/// Callers own `temp` and must remove it if this returns an error.
pub fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    match std::fs::rename(temp, target) {
        Ok(()) => Ok(()),
        Err(err)
            if err.kind() == std::io::ErrorKind::AlreadyExists
                || err.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            std::fs::remove_file(target)?;
            std::fs::rename(temp, target)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("kaminari-fsutil-{name}-{}", std::process::id()))
    }

    #[test]
    fn moves_a_temp_file_into_a_new_target() {
        let dir = scratch("new");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let temp = dir.join("temp");
        let target = dir.join("target");
        std::fs::write(&temp, b"fresh").unwrap();
        replace_file(&temp, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"fresh");
        assert!(!temp.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replaces_an_existing_target_and_leaves_no_temp() {
        let dir = scratch("replace");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let temp = dir.join("temp");
        let target = dir.join("target");
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&temp, b"new").unwrap();
        replace_file(&temp, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(!temp.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

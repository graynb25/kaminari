// Dialog-scoped path authority and path canonicalization.
//
// The webview never sends a file path for a sensitive operation. Rust opens the
// native dialog, stores the chosen path under a one-use opaque token bound to
// one operation, and consumes that token in the command that performs the work.
// Paths are canonicalized again at use time and symlinks, junctions and other
// reparse points are rejected for the final component.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const GRANT_TTL: Duration = Duration::from_secs(5 * 60);
const TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOperation {
    SettingsImport,
    SettingsExport,
    BackupExport,
    BackupRestore,
    ExtensionInstall,
}

struct Grant {
    path: PathBuf,
    operation: PathOperation,
    expires: Instant,
}

#[derive(Default)]
pub struct PathAuthority {
    grants: Mutex<HashMap<String, Grant>>,
}

impl PathAuthority {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&self, path: &Path, operation: PathOperation) -> Result<String, String> {
        let path = match operation {
            PathOperation::SettingsImport | PathOperation::BackupRestore => {
                resolve_read_file(path, "selected file")?
            }
            PathOperation::ExtensionInstall => resolve_read_file(path, "extension package")?,
            PathOperation::SettingsExport | PathOperation::BackupExport => {
                resolve_write_target(path, "selected file")?
            }
        };
        let token = random_token()?;
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| "file selection is unavailable".to_string())?;
        grants.retain(|_, grant| grant.expires > Instant::now());
        if grants.len() >= 64 {
            return Err("too many pending file selections; finish or cancel one first".to_string());
        }
        grants.insert(
            token.clone(),
            Grant {
                path,
                operation,
                expires: Instant::now() + GRANT_TTL,
            },
        );
        Ok(token)
    }

    pub fn consume(&self, token: &str, operation: PathOperation) -> Result<PathBuf, String> {
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| "file selection is unavailable".to_string())?;
        let grant = grants.remove(token).ok_or_else(selection_expired)?;
        if grant.operation != operation || grant.expires <= Instant::now() {
            return Err(selection_expired());
        }
        match operation {
            PathOperation::SettingsImport | PathOperation::BackupRestore => {
                resolve_read_file(&grant.path, "selected file")
            }
            PathOperation::ExtensionInstall => resolve_read_file(&grant.path, "extension package"),
            PathOperation::SettingsExport | PathOperation::BackupExport => {
                resolve_write_target(&grant.path, "selected file")
            }
        }
    }
}

fn selection_expired() -> String {
    "the file selection expired or was not approved for this action; choose the file again"
        .to_string()
}

fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|err| format!("random source failed: {err}"))?;
    Ok(hex::encode(bytes))
}

/// Resolves a file for reading. The final component must already exist, be a
/// regular file, and not be a symlink or Windows reparse point. The canonical
/// path is returned so later opens cannot follow a parent symlink.
pub fn resolve_read_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be an absolute path"));
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|err| format!("cannot read {label}: {err}"))?;
    if is_link_or_reparse(&metadata) {
        return Err(format!("{label} must not be a symlink or reparse point"));
    }
    if !metadata.is_file() {
        return Err(format!("{label} is not a regular file"));
    }
    let canonical =
        std::fs::canonicalize(path).map_err(|err| format!("cannot resolve {label}: {err}"))?;
    let metadata = std::fs::symlink_metadata(&canonical)
        .map_err(|err| format!("cannot read {label}: {err}"))?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(format!("{label} must be a regular file"));
    }
    Ok(simplify(canonical))
}

/// Resolves a write target. The parent directory is canonicalized, the final
/// name must be a single normal component, and an existing target must be a
/// regular file that is not a symlink or Windows reparse point.
pub fn resolve_write_target(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be an absolute path"));
    }
    let name = path
        .file_name()
        .ok_or_else(|| format!("{label} has no file name"))?;
    if name == "." || name == ".." || name.is_empty() {
        return Err(format!("{label} has an invalid file name"));
    }
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| format!("{label} has no parent folder"))?;
    let parent_metadata =
        std::fs::metadata(parent).map_err(|err| format!("cannot access {label} folder: {err}"))?;
    if !parent_metadata.is_dir() {
        return Err(format!("{label} folder is not a directory"));
    }
    let parent = std::fs::canonicalize(parent)
        .map_err(|err| format!("cannot resolve {label} folder: {err}"))?;
    let target = parent.join(name);
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if is_link_or_reparse(&metadata) {
                return Err(format!("{label} must not be a symlink or reparse point"));
            }
            if !metadata.is_file() {
                return Err(format!("{label} is not a regular file"));
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("cannot inspect {label}: {err}")),
    }
    Ok(simplify(target))
}

fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn simplify(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(text) = path.as_os_str().to_str() {
            if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
                return PathBuf::from(format!(r"\\{rest}"));
            }
            if let Some(rest) = text.strip_prefix(r"\\?\") {
                return PathBuf::from(rest);
            }
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kaminari-paths-{label}-{}-{}",
            std::process::id(),
            random_token().unwrap()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn grants_are_one_use_and_operation_bound() {
        let dir = temp_dir("grants");
        let file = dir.join("settings.json");
        std::fs::write(&file, b"{}").unwrap();
        let authority = PathAuthority::new();
        let token = authority
            .grant(&file, PathOperation::SettingsImport)
            .unwrap();
        assert!(authority
            .consume(&token, PathOperation::BackupRestore)
            .is_err());
        assert!(authority
            .consume(&token, PathOperation::SettingsImport)
            .is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn grant_consumes_once_with_the_right_operation() {
        let dir = temp_dir("consume");
        let file = dir.join("settings.json");
        std::fs::write(&file, b"{}").unwrap();
        let authority = PathAuthority::new();
        let token = authority
            .grant(&file, PathOperation::SettingsImport)
            .unwrap();
        assert_eq!(
            authority
                .consume(&token, PathOperation::SettingsImport)
                .unwrap(),
            simplify(std::fs::canonicalize(&file).unwrap())
        );
        assert!(authority
            .consume(&token, PathOperation::SettingsImport)
            .is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_tokens_are_rejected() {
        let authority = PathAuthority::new();
        assert!(authority
            .consume("not-a-token", PathOperation::SettingsImport)
            .is_err());
    }

    #[test]
    fn read_rejects_directories_and_missing_files() {
        let dir = temp_dir("read");
        assert!(resolve_read_file(&dir, "selected file").is_err());
        assert!(resolve_read_file(&dir.join("missing.json"), "selected file").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_target_rejects_directories_and_relative_paths() {
        let dir = temp_dir("write");
        let child = dir.join("child");
        std::fs::create_dir(&child).unwrap();
        assert!(resolve_write_target(&child, "selected file").is_err());
        assert!(resolve_write_target(Path::new("relative.json"), "selected file").is_err());
        assert!(resolve_write_target(&dir.join("ok.json"), "selected file").is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn read_rejects_symlinks() {
        let dir = temp_dir("symlink");
        let real = dir.join("real.json");
        let link = dir.join("link.json");
        std::fs::write(&real, b"{}").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(resolve_read_file(&link, "selected file").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn read_rejects_windows_reparse_links_when_available() {
        let dir = temp_dir("reparse");
        let real = dir.join("real.json");
        let link = dir.join("link.json");
        std::fs::write(&real, b"{}").unwrap();
        match std::os::windows::fs::symlink_file(&real, &link) {
            Ok(()) => {
                assert!(resolve_read_file(&link, "selected file").is_err());
                assert!(resolve_write_target(&link, "selected file").is_err());
            }
            Err(_) => {
                // Creating a symlink needs Developer Mode or elevation; the
                // reparse attribute check is still exercised by the code above.
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

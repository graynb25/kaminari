// Startup recovery for failures that happen before the WebView can show
// FatalState. The database is opened here so a corrupt or locked file gets a
// native dialog with retry, open-data-folder and quarantine choices instead of
// a silent panic behind the Windows subsystem.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::db::{remove_database_files_and_copies, Db, CLEAR_DATA_MARKER};

const ERROR_CODE: &str = "KAMINARI_STARTUP_DB";
const RETRY: &str = "Retry";
const OPEN_FOLDER: &str = "Open data folder";
const QUARANTINE: &str = "Quarantine and start fresh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Retry,
    OpenFolder,
    Quarantine,
    Quit,
}

/// Open the database, retrying until it succeeds or the user quits. Quarantine
/// moves the live files aside and leaves them on disk for manual recovery.
pub fn open_database(app: &tauri::AppHandle, data_dir: &Path) -> Option<Db> {
    if !clear_requested_data(data_dir) {
        tracing::warn!(
            path = %data_dir.display(),
            "startup: previous clear-local-data request left files behind"
        );
    }
    let db_path = data_dir.join("kaminari.db");
    loop {
        let attempt = std::fs::create_dir_all(data_dir)
            .map_err(|err| format!("The data folder could not be created: {err}"))
            .and_then(|()| {
                Db::new(db_path.to_string_lossy().as_ref()).map_err(|err| err.to_string())
            });

        match attempt {
            Ok(database) => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
                return Some(database);
            }
            Err(error) => {
                tracing::error!(
                    code = ERROR_CODE,
                    path = %db_path.display(),
                    error = %error,
                    "startup: database unavailable"
                );
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                match prompt(data_dir, &error) {
                    Choice::Retry => continue,
                    Choice::OpenFolder => {
                        if let Err(err) = open_data_folder(app, data_dir) {
                            notice(&format!(
                                "The data folder could not be opened: {err}\n\n{}",
                                data_dir.display()
                            ));
                        }
                    }
                    Choice::Quarantine => match quarantine_database_files(&db_path) {
                        Ok(target) => {
                            tracing::warn!(
                                code = ERROR_CODE,
                                path = %target.display(),
                                "startup: database quarantined"
                            );
                        }
                        Err(err) => notice(&format!(
                            "The database files could not be quarantined: {err}\n\nNo files were deleted. You can open the data folder and move them yourself."
                        )),
                    },
                    Choice::Quit => return None,
                }
            }
        }
    }
}

/// Completes a "clear all local data" request that could not remove every file
/// while the app was still running. The marker is written before the live
/// connection is closed, so a crash at any point still leaves the request.
pub(crate) fn clear_requested_data(data_dir: &Path) -> bool {
    let marker = data_dir.join(CLEAR_DATA_MARKER);
    if !marker.is_file() {
        return true;
    }
    let db_path = data_dir.join("kaminari.db");
    let mut all_removed = remove_database_files_and_copies(&db_path);
    for path in [
        data_dir.join("settings.json"),
        data_dir.join("settings.json.tmp"),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                all_removed = false;
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "startup: clear-local-data could not remove a file"
                );
            }
        }
    }
    if all_removed {
        let _ = std::fs::remove_file(&marker);
    }
    all_removed
}

/// Last-resort native message for a failure outside the database path, such as
/// a missing app data folder or an event loop that cannot start.
pub fn report_fatal(message: &str) {
    let _ = MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title("Kaminari could not start")
        .set_description(message)
        .set_buttons(MessageButtons::Ok)
        .show();
}

fn prompt(data_dir: &Path, error: &str) -> Choice {
    let message = format!(
        "Kaminari could not open its database.\n\nData folder:\n{}\n\nError ({ERROR_CODE}):\n{error}\n\nRetry opens the database again. Open data folder shows the files without changing them. Quarantine moves kaminari.db and its -wal/-shm files into a new folder and starts with an empty database; nothing is deleted.",
        data_dir.display()
    );
    let result = MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title("Kaminari could not start")
        .set_description(message)
        .set_buttons(MessageButtons::YesNoCancelCustom(
            RETRY.to_string(),
            OPEN_FOLDER.to_string(),
            QUARANTINE.to_string(),
        ))
        .show();
    match result {
        MessageDialogResult::Custom(label) if label == RETRY => Choice::Retry,
        MessageDialogResult::Custom(label) if label == OPEN_FOLDER => Choice::OpenFolder,
        MessageDialogResult::Custom(label) if label == QUARANTINE => Choice::Quarantine,
        MessageDialogResult::Yes => Choice::Retry,
        MessageDialogResult::No => Choice::OpenFolder,
        _ => Choice::Quit,
    }
}

fn open_data_folder(app: &tauri::AppHandle, data_dir: &Path) -> Result<(), String> {
    app.opener()
        .open_path(data_dir.to_string_lossy().into_owned(), None::<String>)
        .map_err(|err| err.to_string())
}

fn notice(message: &str) {
    let _ = MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("Kaminari")
        .set_description(message)
        .set_buttons(MessageButtons::Ok)
        .show();
}

/// Move the database and its WAL sidecars into a new sibling folder. All files
/// that exist are moved even if one move fails, so nothing is lost.
pub fn quarantine_database_files(db_path: &Path) -> std::io::Result<PathBuf> {
    let parent = db_path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = db_path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "kaminari.db".to_string());
    let sources: Vec<(PathBuf, PathBuf)> = ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            (
                sidecar_path(db_path, suffix),
                PathBuf::from(format!("{name}{suffix}")),
            )
        })
        .filter(|(source, _)| source.exists())
        .collect();
    if sources.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no database files were found",
        ));
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let target = parent.join(format!("{name}.quarantine-{stamp}"));
    std::fs::create_dir(&target)?;

    let mut first_error = None;
    for (source, file_name) in sources {
        if let Err(err) = std::fs::rename(&source, target.join(file_name)) {
            first_error.get_or_insert(err);
        }
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(target),
    }
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut raw = path.as_os_str().to_os_string();
    raw.push(suffix);
    PathBuf::from(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("kaminari-startup-{name}-{}", std::process::id()))
    }

    #[test]
    fn quarantines_the_database_and_its_sidecars() {
        let dir = scratch("full");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("kaminari.db");
        std::fs::write(&db, b"db").unwrap();
        std::fs::write(sidecar_path(&db, "-wal"), b"wal").unwrap();
        std::fs::write(sidecar_path(&db, "-shm"), b"shm").unwrap();

        let target = quarantine_database_files(&db).unwrap();
        assert!(!db.exists());
        assert!(!sidecar_path(&db, "-wal").exists());
        assert!(!sidecar_path(&db, "-shm").exists());
        assert_eq!(std::fs::read(target.join("kaminari.db")).unwrap(), b"db");
        assert_eq!(
            std::fs::read(target.join("kaminari.db-wal")).unwrap(),
            b"wal"
        );
        assert_eq!(
            std::fs::read(target.join("kaminari.db-shm")).unwrap(),
            b"shm"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quarantine_reports_an_empty_data_folder() {
        let dir = scratch("empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let result = quarantine_database_files(&dir.join("kaminari.db"));
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_marker_removes_database_settings_and_sidecars() {
        let dir = scratch("clear");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("kaminari.db");
        std::fs::write(&db, b"db").unwrap();
        std::fs::write(sidecar_path(&db, "-wal"), b"wal").unwrap();
        std::fs::write(sidecar_path(&db, "-shm"), b"shm").unwrap();
        std::fs::write(dir.join("settings.json"), b"{}").unwrap();
        std::fs::write(dir.join(CLEAR_DATA_MARKER), b"1").unwrap();
        let safety = dir.join("kaminari.db.safety-1.db");
        std::fs::write(&safety, b"older library").unwrap();

        assert!(clear_requested_data(&dir));
        assert!(!db.exists());
        assert!(!sidecar_path(&db, "-wal").exists());
        assert!(!sidecar_path(&db, "-shm").exists());
        assert!(!dir.join("settings.json").exists());
        assert!(!dir.join(CLEAR_DATA_MARKER).exists());
        assert!(!safety.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_clear_marker_leaves_data_alone() {
        let dir = scratch("no-clear");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("kaminari.db");
        std::fs::write(&db, b"db").unwrap();

        assert!(clear_requested_data(&dir));
        assert!(db.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

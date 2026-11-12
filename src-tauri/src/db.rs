// SQLite helpers. One file, WAL, migrations checked in.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::backup::Backup;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::anilist::{AiringScheduleEntry, AniListListEntry, CoverImage, Media, MediaTitle};
use crate::error::{AppError, AppResult};
use crate::ext_host::{ExtStorage, ExtensionTrust, Manifest};
#[cfg(debug_assertions)]
use crate::registry::LegacyExtension;
use crate::storage_crypto;

/// Hard ceiling for a restore input. A Kaminari library is a few megabytes;
/// a compiled backup anywhere near this size is not something the app should
/// spend memory or disk on.
const MAX_BACKUP_BYTES: u64 = 1024 * 1024 * 1024;

/// Written before a clear-all request touches the database. If Windows still
/// holds a file open, the next start removes whatever is left before opening a
/// fresh database.
pub(crate) const CLEAR_DATA_MARKER: &str = "clear-local-data";

/// Extension-controlled source metadata is persisted and rendered. Keep every
/// field bounded so one package cannot bloat the database or freeze a list.
const MAX_SOURCE_EXTENSION_ID_BYTES: usize = 128;
const MAX_SOURCE_ID_BYTES: usize = 2048;
const MAX_SOURCE_TITLE_BYTES: usize = 4096;
const MAX_SOURCE_COVER_BYTES: usize = 4096;
const MAX_SOURCE_STATUS_BYTES: usize = 64;
const MAX_SOURCE_EPISODE_TITLE_BYTES: usize = 512;
const MAX_SOURCE_EPISODES_PER_SHOW: usize = 1000;
const MAX_SOURCE_EPISODE_NUMBER: i64 = 10_000;
const MAX_SOURCE_SECONDS: f64 = 7.0 * 24.0 * 60.0 * 60.0;
const MAX_ANILIST_PROGRESS: i64 = 10_000;

/// Bumped whenever a migration changes the on-disk schema. A backup from a
/// newer version is rejected instead of being silently run against older code.
const SCHEMA_VERSION: i64 = 4;

/// Base delay before retrying a failed AniList queue row. Doubles per failed
/// attempt, with deterministic jitter so a large queue does not retry in
/// lockstep, and stops growing at half an hour.
pub const ANILIST_SYNC_BACKOFF_BASE_SECS: i64 = 30;
pub const ANILIST_SYNC_BACKOFF_MAX_SECS: i64 = 30 * 60;

/// Freshness window for the airing schedule snapshot. Inside the window a
/// matching id set returns the cache directly. The view refreshes with
/// force in the background when stale is true.
pub const AIRING_SCHEDULE_FRESH_SECS: i64 = 15 * 60;

/// Aired rows stay for two days so today still reads as a timeline after
/// an episode airs.
pub const AIRING_SCHEDULE_RETENTION_SECS: i64 = 48 * 3600;

/// Tables and columns that every Kaminari backup must contain before it can
/// replace the live library. Newer tables are validated separately when they
/// are present.
const REQUIRED_TABLES: &[(&str, &[&str])] = &[
    (
        "media",
        &["anilist_id", "title_romaji", "cover", "updated_at"],
    ),
    (
        "library",
        &["anilist_id", "status", "added_at", "updated_at"],
    ),
    ("watched", &["anilist_id", "episode", "watched_at"]),
    ("history", &["id", "anilist_id", "episode", "watched_at"]),
    (
        "source_library",
        &["extension_id", "show_id", "title", "added_at", "updated_at"],
    ),
];

/// Tables added by later migrations. Older backups may not have them, but if
/// they do, the columns the app reads must be there.
const OPTIONAL_TABLES: &[(&str, &[&str])] = &[
    ("episodes", &["anilist_id", "number"]),
    ("progress", &["anilist_id", "episode", "position_sec"]),
    ("sources", &["id", "manifest"]),
    ("http_cache", &["key", "body", "fetched_at"]),
    (
        "js_extensions",
        &[
            "id",
            "name",
            "version",
            "api_version",
            "manifest",
            "code",
            "enabled",
            "trust",
            "installed_at",
        ],
    ),
    ("js_storage", &["extension_id", "key", "value"]),
    (
        "source_progress",
        &[
            "extension_id",
            "show_id",
            "episode_id",
            "episode_number",
            "position_sec",
            "duration_sec",
            "watched",
            "updated_at",
        ],
    ),
    (
        "source_history",
        &[
            "extension_id",
            "show_id",
            "episode_id",
            "episode_number",
            "watched",
            "updated_at",
        ],
    ),
    (
        "anilist_sync_queue",
        &["anilist_id", "status", "progress", "updated_at"],
    ),
    ("app_meta", &["key", "value"]),
];

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    path: Arc<PathBuf>,
}

/// Counts that are safe to put in a redacted diagnostics report. Only table
/// names and integers leave the database; no row contents are read.
#[derive(Debug, Clone)]
pub struct DiagnosticsSnapshot {
    pub schema_version: i64,
    pub row_counts: Vec<(&'static str, i64)>,
}

/// Tables reported by the diagnostics export. Keep this list fixed: the report
/// must never take table names from the webview.
pub const DIAGNOSTICS_TABLES: &[&str] = &[
    "media",
    "library",
    "watched",
    "history",
    "source_library",
    "source_progress",
    "source_history",
    "js_extensions",
    "js_storage",
    "airing_schedule",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRetentionResult {
    pub anilist_history: usize,
    pub source_history: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub media: Media,
    pub status: String,
    pub added_at: i64,
    pub watched_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AniListSyncRow {
    pub anilist_id: i64,
    pub status: String,
    pub progress: i64,
    pub updated_at: i64,
    pub attempts: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueItem {
    pub media: Media,
    pub episode: i64,
    pub position_sec: i64,
    pub duration_sec: i64,
    pub watched_count: i64,
    pub total_episodes: i64,
    pub updated_at: i64,
}

#[cfg(debug_assertions)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSource {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub source_count: usize,
    pub nsfw: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub media: Media,
    pub episode: i64,
    pub watched_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsExtensionRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub kind: String,
    pub enabled: bool,
    pub trust: String,
    pub installed_at: i64,
    pub hosts: Vec<String>,
    pub nsfw: bool,
}

/// One field that differs between an installed package and a replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInstallChange {
    pub field: String,
    pub before: String,
    pub after: String,
}

/// What would happen if a package were installed over the current state. The
/// webview uses this to ask before replacing a package the user already has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInstallPreview {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub trust: String,
    pub new_install: bool,
    pub requires_approval: bool,
    pub code_changed: bool,
    pub name_changed: bool,
    pub version_changed: bool,
    pub manifest_changed: bool,
    pub trust_changed: bool,
    pub added_hosts: Vec<String>,
    pub removed_hosts: Vec<String>,
    pub existing_name: Option<String>,
    pub existing_version: Option<String>,
    pub existing_trust: Option<String>,
    pub changes: Vec<ExtensionInstallChange>,
}

#[derive(Debug, Clone)]
pub struct StoredJsExtension {
    pub manifest: Manifest,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLibraryItem {
    pub extension_id: String,
    pub extension_name: String,
    pub show_id: String,
    pub title: String,
    pub cover: Option<String>,
    pub year: Option<i32>,
    pub status: Option<String>,
    pub anilist_id: Option<i64>,
    pub added_at: i64,
    pub watched_count: i64,
    pub last_episode_id: Option<String>,
    pub last_episode: Option<i64>,
    pub last_position: Option<f64>,
    pub last_duration: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProgress {
    pub episode_id: String,
    pub episode_number: i64,
    pub episode_title: Option<String>,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub watched: bool,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProgressMark {
    pub episode_id: String,
    pub episode_number: i64,
    pub episode_title: Option<String>,
    pub duration_sec: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceContinueItem {
    pub extension_id: String,
    pub extension_name: String,
    pub show_id: String,
    pub title: String,
    pub cover: Option<String>,
    pub anilist_id: Option<i64>,
    pub episode_id: String,
    pub episode_number: i64,
    pub episode_title: Option<String>,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub watched: bool,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceHistoryItem {
    pub extension_id: String,
    pub extension_name: String,
    pub show_id: String,
    pub title: String,
    pub cover: Option<String>,
    pub episode_id: String,
    pub episode_number: i64,
    pub episode_title: Option<String>,
    pub watched: bool,
    pub updated_at: i64,
}

impl Db {
    pub fn new(path: &str) -> AppResult<Self> {
        let path = PathBuf::from(path);
        let conn = Connection::open(&path)?;
        apply_pragmas(&conn)?;
        apply_migrations(&conn)?;
        // Best effort: a DPAPI failure must not lock the user out of the app.
        // Plaintext rows stay readable and are retried when an extension reads
        // them or the database opens again.
        if let Err(err) = migrate_plaintext_storage(&conn) {
            tracing::warn!(
                error = %err,
                "extension storage: plaintext migration failed; values stay readable and will retry on read"
            );
        }
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: Arc::new(path),
        })
    }

    pub fn list_library(&self) -> AppResult<Vec<LibraryItem>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT m.anilist_id, m.title_romaji, m.title_english, m.title_native,
                    m.cover, m.banner, m.year, m.format, m.status, m.episodes,
                    m.score, m.genres, m.synopsis, l.status, l.added_at,
                    (SELECT COUNT(*) FROM watched w WHERE w.anilist_id = m.anilist_id)
             FROM library l
             JOIN media m ON m.anilist_id = l.anilist_id
             ORDER BY l.added_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            let genres: Option<String> = row.get(11)?;
            let cover: String = row.get(4)?;
            let score: Option<f64> = row.get(10)?;
            let media = Media {
                id: row.get(0)?,
                title: MediaTitle {
                    romaji: row.get(1)?,
                    english: row.get(2)?,
                    native: row.get(3)?,
                },
                cover_image: if cover.is_empty() {
                    None
                } else {
                    Some(CoverImage {
                        extra_large: Some(cover),
                        large: None,
                        color: None,
                    })
                },
                banner_image: row.get(5)?,
                description: row.get(12)?,
                genres: genres.and_then(|raw| serde_json::from_str(&raw).ok()),
                average_score: score.map(|value| value.round() as i32),
                season_year: row.get(6)?,
                format: row.get(7)?,
                status: row.get(8)?,
                episodes: row.get(9)?,
                recommendations: None,
            };
            Ok(LibraryItem {
                media,
                status: row.get(13)?,
                added_at: row.get(14)?,
                watched_count: row.get(15)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn add_to_library(&self, media: &Media, status: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        upsert_media(&conn, media)?;
        conn.execute(
            "INSERT INTO library(anilist_id, status, added_at, updated_at)
             VALUES (?1, ?2, unixepoch(), unixepoch())
             ON CONFLICT(anilist_id) DO UPDATE SET status = excluded.status, updated_at = unixepoch()",
            params![media.id, status],
        )?;
        Ok(())
    }

    pub fn remove_from_library(&self, id: i64) -> AppResult<()> {
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM library WHERE anilist_id = ?1", params![id])?;
        // A local removal must not be undone by a queued write from before it.
        tx.execute(
            "DELETE FROM anilist_sync_queue WHERE anilist_id = ?1",
            params![id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_library_status(&self, id: i64, status: &str) -> AppResult<()> {
        const ALLOWED: [&str; 5] = ["watching", "planning", "completed", "paused", "dropped"];
        if !ALLOWED.contains(&status) {
            return Err(AppError::Blocked(format!("Unknown status: {status}")));
        }
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let changed = conn.execute(
            "UPDATE library SET status = ?2, updated_at = unixepoch() WHERE anilist_id = ?1",
            params![id, status],
        )?;
        if changed > 0 {
            enqueue_anilist_sync(&conn, id, status)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn pending_anilist_sync(&self) -> AppResult<Vec<AniListSyncRow>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        read_anilist_queue(&conn)
    }

    /// Rows whose retry delay has elapsed, oldest first, capped at `limit`.
    /// A successful or fresh local change resets `attempts`, so those rows are
    /// immediately eligible again.
    pub fn pending_anilist_sync_due(&self, limit: usize) -> AppResult<Vec<AniListSyncRow>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let rows = read_anilist_queue(&conn)?;
        let now = unix_now();
        Ok(rows
            .into_iter()
            .filter(|row| anilist_sync_row_due(row, now))
            .take(limit)
            .collect())
    }

    pub fn anilist_sync_row(&self, anilist_id: i64) -> AppResult<Option<AniListSyncRow>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.query_row(
            "SELECT anilist_id, status, progress, updated_at, attempts, last_error
             FROM anilist_sync_queue WHERE anilist_id = ?1",
            params![anilist_id],
            |row| {
                Ok(AniListSyncRow {
                    anilist_id: row.get(0)?,
                    status: row.get(1)?,
                    progress: row.get(2)?,
                    updated_at: row.get(3)?,
                    attempts: row.get(4)?,
                    last_error: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn pending_anilist_sync_count(&self) -> AppResult<usize> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        pending_anilist_sync_count(&conn)
    }

    #[cfg(test)]
    pub fn remove_anilist_sync(&self, anilist_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "DELETE FROM anilist_sync_queue WHERE anilist_id = ?1",
            params![anilist_id],
        )?;
        Ok(())
    }

    /// Removes the queue row only when it still matches the snapshot that was
    /// pushed. A newer local edit stays queued for the next run.
    pub fn complete_anilist_sync(&self, snapshot: &AniListSyncRow) -> AppResult<bool> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let changed = conn.execute(
            "DELETE FROM anilist_sync_queue
             WHERE anilist_id = ?1 AND updated_at = ?2 AND status = ?3
               AND progress = ?4 AND attempts = ?5",
            params![
                snapshot.anilist_id,
                snapshot.updated_at,
                snapshot.status,
                snapshot.progress,
                snapshot.attempts
            ],
        )?;
        Ok(changed > 0)
    }

    #[cfg(test)]
    pub fn fail_anilist_sync(&self, anilist_id: i64, error: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "UPDATE anilist_sync_queue
             SET attempts = attempts + 1, last_error = ?2, updated_at = unixepoch()
             WHERE anilist_id = ?1",
            params![anilist_id, error],
        )?;
        Ok(())
    }

    /// Records a failure only for the queue snapshot that was attempted, so a
    /// newer local edit keeps its zero-attempt state and is retried at once.
    pub fn fail_anilist_sync_snapshot(
        &self,
        snapshot: &AniListSyncRow,
        error: &str,
    ) -> AppResult<bool> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let changed = conn.execute(
            "UPDATE anilist_sync_queue
             SET attempts = attempts + 1, last_error = ?6, updated_at = unixepoch()
             WHERE anilist_id = ?1 AND updated_at = ?2 AND status = ?3
               AND progress = ?4 AND attempts = ?5",
            params![
                snapshot.anilist_id,
                snapshot.updated_at,
                snapshot.status,
                snapshot.progress,
                snapshot.attempts,
                error
            ],
        )?;
        Ok(changed > 0)
    }

    pub fn sync_anilist_entries(&self, entries: &[AniListListEntry]) -> AppResult<usize> {
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        let mut synced = HashSet::new();
        for entry in entries {
            let Some(status) = anilist_library_status(&entry.status) else {
                continue;
            };
            upsert_media(&tx, &entry.media)?;
            tx.execute(
                "INSERT INTO library(anilist_id, status, added_at, updated_at)
                 VALUES (?1, ?2, unixepoch(), unixepoch())
                 ON CONFLICT(anilist_id) DO UPDATE SET
                    status = excluded.status,
                    updated_at = unixepoch()",
                params![entry.media.id, status],
            )?;
            let mut progress = entry.progress.clamp(0, MAX_ANILIST_PROGRESS);
            if let Some(episodes) = entry.media.episodes.filter(|episodes| *episodes > 0) {
                progress = progress.min(i64::from(episodes).min(MAX_ANILIST_PROGRESS));
            }
            for episode in 1..=progress {
                tx.execute(
                    "INSERT OR IGNORE INTO watched(anilist_id, episode, watched_at)
                     VALUES (?1, ?2, unixepoch())",
                    params![entry.media.id, episode],
                )?;
            }
            synced.insert(entry.media.id);
        }
        tx.commit()?;
        Ok(synced.len())
    }

    pub fn history_list(&self, limit: usize) -> AppResult<Vec<HistoryItem>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT m.anilist_id, m.title_romaji, m.title_english, m.title_native,
                    m.cover, m.banner, m.year, m.format, m.status, m.episodes,
                    m.score, m.genres, m.synopsis,
                    h.episode, MAX(h.watched_at) AS watched_at
             FROM history h
             JOIN media m ON m.anilist_id = h.anilist_id
             GROUP BY h.anilist_id, h.episode
             ORDER BY MAX(h.watched_at) DESC, MAX(h.id) DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit as i64], |row| {
            let genres: Option<String> = row.get(11)?;
            let cover: String = row.get(4)?;
            let score: Option<f64> = row.get(10)?;
            let total_episodes: Option<i64> = row.get(9)?;
            let media = Media {
                id: row.get(0)?,
                title: MediaTitle {
                    romaji: row.get(1)?,
                    english: row.get(2)?,
                    native: row.get(3)?,
                },
                cover_image: if cover.is_empty() {
                    None
                } else {
                    Some(CoverImage {
                        extra_large: Some(cover),
                        large: None,
                        color: None,
                    })
                },
                banner_image: row.get(5)?,
                description: row.get(12)?,
                genres: genres.and_then(|raw| serde_json::from_str(&raw).ok()),
                average_score: score.map(|value| value.round() as i32),
                season_year: row.get(6)?,
                format: row.get(7)?,
                status: row.get(8)?,
                episodes: total_episodes.map(|value| value as i32),
                recommendations: None,
            };
            Ok(HistoryItem {
                media,
                episode: row.get(13)?,
                watched_at: row.get(14)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn history_clear(&self) -> AppResult<()> {
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM history", [])?;
        tx.execute("DELETE FROM source_history", [])?;
        tx.execute("DELETE FROM progress", [])?;
        tx.execute(
            "UPDATE source_progress SET position_sec = 0, continue_visible = 0",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn history_remove(&self, anilist_id: i64, episode: i64) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "DELETE FROM history WHERE anilist_id = ?1 AND episode = ?2",
            params![anilist_id, episode],
        )?;
        Ok(())
    }

    /// Deletes activity rows older than `days`. Watched marks, library rows,
    /// playback progress, Continue Watching and extension storage are not
    /// touched; only the two history tables are pruned. `None` means Forever
    /// and does nothing.
    pub fn purge_history_older_than(&self, days: Option<u32>) -> AppResult<HistoryRetentionResult> {
        let Some(days) = days else {
            return Ok(HistoryRetentionResult::default());
        };
        if days == 0 {
            return Err(AppError::Blocked(
                "history retention must be at least one day".to_string(),
            ));
        }
        let cutoff = history_retention_cutoff(unix_now(), days);
        self.purge_history_before(cutoff)
    }

    fn purge_history_before(&self, cutoff: i64) -> AppResult<HistoryRetentionResult> {
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        let anilist_history =
            tx.execute("DELETE FROM history WHERE watched_at < ?1", params![cutoff])?;
        let source_history = tx.execute(
            "DELETE FROM source_history WHERE updated_at < ?1",
            params![cutoff],
        )?;
        tx.commit()?;
        Ok(HistoryRetentionResult {
            anilist_history,
            source_history,
        })
    }

    /// Reads the schema version and row counts for the redacted diagnostics
    /// report. No titles, URLs, identifiers or row values are selected.
    pub fn diagnostics_snapshot(&self) -> AppResult<DiagnosticsSnapshot> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let mut row_counts = Vec::with_capacity(DIAGNOSTICS_TABLES.len());
        for table in DIAGNOSTICS_TABLES {
            let count: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |row| {
                    row.get(0)
                })?;
            row_counts.push((*table, count));
        }
        Ok(DiagnosticsSnapshot {
            schema_version,
            row_counts,
        })
    }

    /// Distinct AniList ids that can appear in the schedule. Library rows
    /// except dropped, plus linked source rows. Sorted for stable storage.
    pub fn airing_schedule_ids(&self) -> AppResult<Vec<i64>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT anilist_id FROM library WHERE status <> 'dropped'
             UNION
             SELECT anilist_id FROM source_library WHERE anilist_id IS NOT NULL",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
        let mut ids: Vec<i64> = rows.collect::<Result<Vec<_>, _>>()?;
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    /// Cached schedule rows ordered by air time.
    pub fn airing_schedule_cached(&self) -> AppResult<Vec<AiringScheduleEntry>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT anilist_id, episode, airing_at FROM airing_schedule ORDER BY airing_at ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AiringScheduleEntry {
                anilist_id: row.get(0)?,
                episode: row.get(1)?,
                airing_at: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Snapshot time for the schedule, if a fetch ever succeeded.
    pub fn airing_schedule_fetched_at(&self) -> AppResult<Option<i64>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'airing_schedule_fetched_at'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(raw.and_then(|value| value.parse::<i64>().ok()))
    }

    /// Library id set stored with the last snapshot. Empty when no snapshot
    /// exists or the stored value does not parse.
    pub fn airing_schedule_id_set(&self) -> AppResult<Vec<i64>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'airing_schedule_id_set'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let Some(raw) = raw else {
            return Ok(Vec::new());
        };
        let mut ids: Vec<i64> = serde_json::from_str(&raw).unwrap_or_default();
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    /// Replaces the schedule snapshot in one transaction. Removes ids that
    /// left the library, drops rows older than 48 hours, clears future rows
    /// for the refreshed ids, upserts the new entries, then stores the id
    /// set and the snapshot time. Returns the snapshot time.
    pub fn airing_schedule_replace(
        &self,
        ids: &[i64],
        entries: &[AiringScheduleEntry],
    ) -> AppResult<i64> {
        let mut sorted_ids: Vec<i64> = ids.to_vec();
        sorted_ids.sort_unstable();
        sorted_ids.dedup();
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        let now = unix_now();
        let cutoff = now.saturating_sub(AIRING_SCHEDULE_RETENTION_SECS);
        if sorted_ids.is_empty() {
            tx.execute("DELETE FROM airing_schedule", [])?;
        } else {
            let placeholders = sorted_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let delete_removed =
                format!("DELETE FROM airing_schedule WHERE anilist_id NOT IN ({placeholders})");
            tx.execute(
                &delete_removed,
                rusqlite::params_from_iter(sorted_ids.iter()),
            )?;
            tx.execute(
                "DELETE FROM airing_schedule WHERE airing_at < ?1",
                params![cutoff],
            )?;
            let delete_future = format!(
                "DELETE FROM airing_schedule WHERE anilist_id IN ({placeholders}) AND airing_at >= ?"
            );
            let mut future_params: Vec<i64> = sorted_ids.clone();
            future_params.push(now);
            tx.execute(
                &delete_future,
                rusqlite::params_from_iter(future_params.iter()),
            )?;
            let mut allowed: HashSet<i64> = HashSet::new();
            for id in &sorted_ids {
                allowed.insert(*id);
            }
            for entry in entries {
                if !allowed.contains(&entry.anilist_id) {
                    continue;
                }
                tx.execute(
                    "INSERT INTO airing_schedule(anilist_id, episode, airing_at)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(anilist_id, airing_at) DO UPDATE SET episode = excluded.episode",
                    params![entry.anilist_id, entry.episode, entry.airing_at],
                )?;
            }
        }
        let id_set =
            serde_json::to_string(&sorted_ids).map_err(|err| AppError::Db(err.to_string()))?;
        tx.execute(
            "INSERT OR REPLACE INTO app_meta(key, value) VALUES ('airing_schedule_id_set', ?1)",
            params![id_set],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO app_meta(key, value) VALUES ('airing_schedule_fetched_at', ?1)",
            params![now.to_string()],
        )?;
        tx.commit()?;
        Ok(now)
    }

    pub fn watched_list(&self, anilist_id: i64) -> AppResult<Vec<i64>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        read_watched(&conn, anilist_id)
    }

    pub fn set_watched(&self, media: &Media, episode: i64, watched: bool) -> AppResult<Vec<i64>> {
        check_source_episode_number(episode)?;
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        upsert_media(&conn, media)?;
        if watched {
            ensure_library_entry(&conn, media.id)?;
        }
        apply_watched(&conn, media.id, episode, watched)?;
        // A negative local action must not add the title to AniList. Only an
        // existing library row can produce a queued write, and it keeps the
        // status the user already chose.
        enqueue_anilist_sync_if_in_library(&conn, media.id)?;
        read_watched(&conn, media.id)
    }

    pub fn mark_all_watched(
        &self,
        media: &Media,
        through: i64,
        watched: bool,
    ) -> AppResult<Vec<i64>> {
        let through = clamp_watched_through(through, media.episodes);
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        upsert_media(&tx, media)?;
        if watched {
            ensure_library_entry(&tx, media.id)?;
            for episode in 1..=through {
                tx.execute(
                    "INSERT OR IGNORE INTO watched(anilist_id, episode, watched_at)
                     VALUES (?1, ?2, unixepoch())",
                    params![media.id, episode],
                )?;
            }
            tx.execute(
                "INSERT INTO history(anilist_id, episode, watched_at)
                 VALUES (?1, ?2, unixepoch())",
                params![media.id, through],
            )?;
            tx.execute(
                "INSERT INTO progress(anilist_id, episode, position_sec, duration_sec, updated_at)
                 VALUES (?1, ?2, 0, 0, unixepoch())
                 ON CONFLICT(anilist_id) DO UPDATE SET
                    episode = excluded.episode,
                    updated_at = unixepoch()",
                params![media.id, through],
            )?;
        } else {
            tx.execute(
                "DELETE FROM watched WHERE anilist_id = ?1",
                params![media.id],
            )?;
            tx.execute(
                "DELETE FROM history WHERE anilist_id = ?1",
                params![media.id],
            )?;
            tx.execute(
                "DELETE FROM progress WHERE anilist_id = ?1",
                params![media.id],
            )?;
        }
        enqueue_anilist_sync_if_in_library(&tx, media.id)?;
        tx.commit()?;
        read_watched(&conn, media.id)
    }

    pub fn continue_watching(&self, limit: usize) -> AppResult<Vec<ContinueItem>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT m.anilist_id, m.title_romaji, m.title_english, m.title_native,
                    m.cover, m.banner, m.year, m.format, m.status, m.episodes,
                    m.score, m.genres, m.synopsis,
                    p.episode, p.position_sec, p.duration_sec, p.updated_at,
                    (SELECT COUNT(*) FROM watched w WHERE w.anilist_id = m.anilist_id)
             FROM progress p
             JOIN media m ON m.anilist_id = p.anilist_id
             JOIN library l ON l.anilist_id = p.anilist_id
             WHERE l.status IN ('watching', 'completed')
             ORDER BY p.updated_at DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit as i64], |row| {
            let genres: Option<String> = row.get(11)?;
            let cover: String = row.get(4)?;
            let score: Option<f64> = row.get(10)?;
            let total_episodes: Option<i64> = row.get(9)?;
            let media = Media {
                id: row.get(0)?,
                title: MediaTitle {
                    romaji: row.get(1)?,
                    english: row.get(2)?,
                    native: row.get(3)?,
                },
                cover_image: if cover.is_empty() {
                    None
                } else {
                    Some(CoverImage {
                        extra_large: Some(cover),
                        large: None,
                        color: None,
                    })
                },
                banner_image: row.get(5)?,
                description: row.get(12)?,
                genres: genres.and_then(|raw| serde_json::from_str(&raw).ok()),
                average_score: score.map(|value| value.round() as i32),
                season_year: row.get(6)?,
                format: row.get(7)?,
                status: row.get(8)?,
                episodes: total_episodes.map(|value| value as i32),
                recommendations: None,
            };
            Ok(ContinueItem {
                media,
                episode: row.get(13)?,
                position_sec: row.get(14)?,
                duration_sec: row.get(15)?,
                updated_at: row.get(16)?,
                watched_count: row.get(17)?,
                total_episodes: total_episodes.unwrap_or(0),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn list_js_extensions(&self) -> AppResult<Vec<JsExtensionRow>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT id, name, version, api_version, enabled, trust, installed_at, manifest
             FROM js_extensions ORDER BY name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            let manifest_raw: String = row.get(7)?;
            let manifest = serde_json::from_str::<Manifest>(&manifest_raw).ok();
            let id: String = row.get(0)?;
            let (hosts, nsfw) = manifest
                .map(|manifest| (manifest.permissions.hosts, manifest.nsfw))
                .unwrap_or_default();
            Ok(JsExtensionRow {
                kind: if id.starts_with("mangayomi.") {
                    "mangayomi".to_string()
                } else {
                    "native".to_string()
                },
                id,
                name: row.get(1)?,
                version: row.get(2)?,
                api_version: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
                trust: ExtensionTrust::from_stored(&row.get::<_, String>(5)?)
                    .as_str()
                    .to_string(),
                installed_at: row.get(6)?,
                hosts,
                nsfw,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn js_extension(&self, id: &str) -> AppResult<Option<StoredJsExtension>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT manifest, code FROM js_extensions WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((manifest_raw, code)) = row else {
            return Ok(None);
        };
        let manifest: Manifest = serde_json::from_str(&manifest_raw)
            .map_err(|err| AppError::Db(format!("stored manifest is invalid: {err}")))?;
        Ok(Some(StoredJsExtension { manifest, code }))
    }

    /// Compares a package with the installed row and reports every user-visible
    /// change. Any replacement that is not byte-identical needs approval.
    pub fn extension_install_preview(
        &self,
        manifest: &Manifest,
        code: &str,
        trust: ExtensionTrust,
    ) -> AppResult<ExtensionInstallPreview> {
        manifest.validate()?;
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        install_preview(&conn, manifest, code, trust)
    }

    /// Installs only when the ID is free or the package is identical. A
    /// different package with the same ID is refused; the caller must review
    /// the preview and use `replace_js_extension` after approval.
    pub fn install_js_extension(
        &self,
        manifest: &Manifest,
        code: &str,
        trust: ExtensionTrust,
    ) -> AppResult<()> {
        manifest.validate()?;
        let manifest_raw =
            serde_json::to_string(manifest).map_err(|err| AppError::Db(err.to_string()))?;
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        let preview = install_preview(&tx, manifest, code, trust)?;
        if preview.requires_approval {
            return Err(AppError::Blocked(format!(
                "extension {} is already installed with different content; review the update before replacing it",
                manifest.id
            )));
        }
        if preview.new_install {
            tx.execute(
                "INSERT INTO js_extensions(
                    id, name, version, api_version, manifest, code, enabled, trust, installed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, unixepoch())",
                params![
                    manifest.id,
                    manifest.name,
                    manifest.version,
                    manifest.api_version,
                    manifest_raw,
                    code,
                    trust.as_str(),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Replaces an installed package. This is only called after the webview
    /// showed the preview and the user approved the exact changes.
    pub fn replace_js_extension(
        &self,
        manifest: &Manifest,
        code: &str,
        trust: ExtensionTrust,
    ) -> AppResult<()> {
        manifest.validate()?;
        let manifest_raw =
            serde_json::to_string(manifest).map_err(|err| AppError::Db(err.to_string()))?;
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "INSERT INTO js_extensions(
                id, name, version, api_version, manifest, code, enabled, trust, installed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, unixepoch())
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                version = excluded.version,
                api_version = excluded.api_version,
                manifest = excluded.manifest,
                code = excluded.code,
                trust = excluded.trust",
            params![
                manifest.id,
                manifest.name,
                manifest.version,
                manifest.api_version,
                manifest_raw,
                code,
                trust.as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn remove_js_extension(&self, id: &str) -> AppResult<()> {
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        // The source tables have no foreign key to js_extensions. Delete the
        // rows in the same transaction so uninstalling a source cannot leave
        // dead Continue Watching, history or library cards behind.
        tx.execute(
            "DELETE FROM source_history WHERE extension_id = ?1",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM source_progress WHERE extension_id = ?1",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM source_library WHERE extension_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM js_extensions WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn source_library_add(
        &self,
        extension_id: &str,
        show_id: &str,
        title: &str,
        cover: Option<&str>,
        year: Option<i32>,
        status: Option<&str>,
        anilist_id: Option<i64>,
    ) -> AppResult<()> {
        validate_source_library_input(extension_id, show_id, title, cover, status)?;
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        // One source show per AniList id per extension. Linking a second show
        // to the same AniList entry clears the old link instead of letting two
        // seasons queue progress to the same AniList title.
        if let Some(anilist_id) = anilist_id {
            tx.execute(
                "UPDATE source_library SET anilist_id = NULL
                 WHERE extension_id = ?1 AND anilist_id = ?2 AND show_id <> ?3",
                params![extension_id, anilist_id, show_id],
            )?;
        }
        tx.execute(
            "INSERT INTO source_library(
                extension_id, show_id, title, cover, year, status, anilist_id,
                added_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch(), unixepoch())
             ON CONFLICT(extension_id, show_id) DO UPDATE SET
                title = excluded.title,
                cover = excluded.cover,
                year = excluded.year,
                status = excluded.status,
                anilist_id = COALESCE(excluded.anilist_id, source_library.anilist_id),
                updated_at = unixepoch()",
            params![
                extension_id,
                show_id,
                title,
                cover,
                year,
                status,
                anilist_id
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn source_library_list(&self) -> AppResult<Vec<SourceLibraryItem>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT sl.extension_id,
                    COALESCE(je.name, sl.extension_id),
                    sl.show_id, sl.title, sl.cover, sl.year, sl.status, sl.anilist_id,
                    sl.added_at,
                    (SELECT COUNT(*) FROM source_progress sp
                     WHERE sp.extension_id = sl.extension_id AND sp.show_id = sl.show_id AND sp.watched = 1),
                    (SELECT episode_id FROM source_progress sp
                     WHERE sp.extension_id = sl.extension_id AND sp.show_id = sl.show_id
                     ORDER BY sp.updated_at DESC, sp.episode_number DESC LIMIT 1),
                    (SELECT episode_number FROM source_progress sp
                     WHERE sp.extension_id = sl.extension_id AND sp.show_id = sl.show_id
                     ORDER BY sp.updated_at DESC, sp.episode_number DESC LIMIT 1),
                    (SELECT position_sec FROM source_progress sp
                     WHERE sp.extension_id = sl.extension_id AND sp.show_id = sl.show_id
                     ORDER BY sp.updated_at DESC, sp.episode_number DESC LIMIT 1),
                    (SELECT duration_sec FROM source_progress sp
                     WHERE sp.extension_id = sl.extension_id AND sp.show_id = sl.show_id
                     ORDER BY sp.updated_at DESC, sp.episode_number DESC LIMIT 1)
             FROM source_library sl
             LEFT JOIN js_extensions je ON je.id = sl.extension_id
             WHERE je.id IS NOT NULL
             ORDER BY sl.updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(SourceLibraryItem {
                extension_id: row.get(0)?,
                extension_name: row.get(1)?,
                show_id: row.get(2)?,
                title: row.get(3)?,
                cover: row.get(4)?,
                year: row.get(5)?,
                status: row.get(6)?,
                anilist_id: row.get(7)?,
                added_at: row.get(8)?,
                watched_count: row.get(9)?,
                last_episode_id: row.get(10)?,
                last_episode: row.get(11)?,
                last_position: row.get(12)?,
                last_duration: row.get(13)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn source_library_remove(&self, extension_id: &str, show_id: &str) -> AppResult<()> {
        validate_source_keys(extension_id, show_id)?;
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "DELETE FROM source_library WHERE extension_id = ?1 AND show_id = ?2",
            params![extension_id, show_id],
        )?;
        conn.execute(
            "DELETE FROM source_progress WHERE extension_id = ?1 AND show_id = ?2",
            params![extension_id, show_id],
        )?;
        conn.execute(
            "DELETE FROM source_history WHERE extension_id = ?1 AND show_id = ?2",
            params![extension_id, show_id],
        )?;
        Ok(())
    }

    pub fn source_progress_get(
        &self,
        extension_id: &str,
        show_id: &str,
    ) -> AppResult<Vec<SourceProgress>> {
        validate_source_keys(extension_id, show_id)?;
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT episode_id, episode_number, episode_title, position_sec, duration_sec, watched, updated_at
             FROM source_progress
             WHERE extension_id = ?1 AND show_id = ?2
             ORDER BY episode_number",
        )?;
        let rows = statement.query_map(params![extension_id, show_id], |row| {
            Ok(SourceProgress {
                episode_id: row.get(0)?,
                episode_number: row.get(1)?,
                episode_title: row.get(2)?,
                position_sec: row.get(3)?,
                duration_sec: row.get(4)?,
                watched: row.get::<_, i64>(5)? != 0,
                updated_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn source_progress_set(
        &self,
        extension_id: &str,
        show_id: &str,
        episode_id: &str,
        episode_number: i64,
        episode_title: Option<&str>,
        position_sec: f64,
        duration_sec: f64,
        watched: bool,
    ) -> AppResult<()> {
        validate_source_progress_input(
            extension_id,
            show_id,
            episode_id,
            episode_number,
            episode_title,
            position_sec,
            duration_sec,
        )?;
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let exists: bool = conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM source_progress
                WHERE extension_id = ?1 AND show_id = ?2 AND episode_id = ?3
             )",
            params![extension_id, show_id, episode_id],
            |row| row.get(0),
        )?;
        if !exists {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM source_progress
                 WHERE extension_id = ?1 AND show_id = ?2",
                params![extension_id, show_id],
                |row| row.get(0),
            )?;
            if count as usize >= MAX_SOURCE_EPISODES_PER_SHOW {
                return Err(AppError::Blocked(format!(
                    "episode limit reached ({MAX_SOURCE_EPISODES_PER_SHOW} per show)"
                )));
            }
        }
        conn.execute(
            "INSERT INTO source_progress(
                extension_id, show_id, episode_id, episode_number, episode_title,
                position_sec, duration_sec, watched, updated_at, continue_visible
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, unixepoch(), 1)
             ON CONFLICT(extension_id, show_id, episode_id) DO UPDATE SET
                episode_number = excluded.episode_number,
                episode_title = excluded.episode_title,
                position_sec = excluded.position_sec,
                duration_sec = excluded.duration_sec,
                watched = excluded.watched,
                updated_at = unixepoch(),
                continue_visible = 1",
            params![
                extension_id,
                show_id,
                episode_id,
                episode_number,
                episode_title,
                position_sec,
                duration_sec,
                i64::from(watched),
            ],
        )?;
        conn.execute(
            "INSERT INTO source_history(
                extension_id, show_id, episode_id, episode_number, episode_title,
                watched, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, unixepoch())
             ON CONFLICT(extension_id, show_id, episode_id) DO UPDATE SET
                episode_number = excluded.episode_number,
                episode_title = excluded.episode_title,
                watched = excluded.watched,
                updated_at = unixepoch()",
            params![
                extension_id,
                show_id,
                episode_id,
                episode_number,
                episode_title,
                i64::from(watched),
            ],
        )?;
        conn.execute(
            "UPDATE source_library SET updated_at = unixepoch()
             WHERE extension_id = ?1 AND show_id = ?2",
            params![extension_id, show_id],
        )?;
        Ok(())
    }

    pub fn source_progress_mark_all(
        &self,
        extension_id: &str,
        show_id: &str,
        episodes: &[SourceProgressMark],
        watched: bool,
    ) -> AppResult<()> {
        validate_source_progress_marks(extension_id, show_id, episodes)?;
        if episodes.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        if watched {
            let existing: HashSet<String> = {
                let mut statement = tx.prepare(
                    "SELECT episode_id FROM source_progress
                     WHERE extension_id = ?1 AND show_id = ?2",
                )?;
                let rows = statement.query_map(params![extension_id, show_id], |row| row.get(0))?;
                rows.collect::<Result<HashSet<_>, _>>()?
            };
            let mut incoming = HashSet::new();
            for episode in episodes {
                if !existing.contains(episode.episode_id.as_str()) {
                    incoming.insert(episode.episode_id.as_str());
                }
            }
            if existing.len() + incoming.len() > MAX_SOURCE_EPISODES_PER_SHOW {
                return Err(AppError::Blocked(format!(
                    "episode limit reached ({MAX_SOURCE_EPISODES_PER_SHOW} per show)"
                )));
            }
            let mut statement = tx.prepare(
                "INSERT INTO source_progress(
                    extension_id, show_id, episode_id, episode_number, episode_title,
                    position_sec, duration_sec, watched, updated_at, continue_visible
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, unixepoch(), 1)
                 ON CONFLICT(extension_id, show_id, episode_id) DO UPDATE SET
                    episode_number = excluded.episode_number,
                    episode_title = excluded.episode_title,
                    position_sec = excluded.position_sec,
                    duration_sec = excluded.duration_sec,
                    watched = 1,
                    updated_at = unixepoch(),
                    continue_visible = 1",
            )?;
            let mut history_statement = tx.prepare(
                "INSERT INTO source_history(
                    extension_id, show_id, episode_id, episode_number, episode_title,
                    watched, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 1, unixepoch())
                 ON CONFLICT(extension_id, show_id, episode_id) DO UPDATE SET
                    episode_number = excluded.episode_number,
                    episode_title = excluded.episode_title,
                    watched = 1,
                    updated_at = unixepoch()",
            )?;
            for episode in episodes {
                let duration = episode.duration_sec;
                statement.execute(params![
                    extension_id,
                    show_id,
                    episode.episode_id,
                    episode.episode_number,
                    episode.episode_title.as_deref(),
                    duration,
                    duration,
                ])?;
                history_statement.execute(params![
                    extension_id,
                    show_id,
                    episode.episode_id,
                    episode.episode_number,
                    episode.episode_title.as_deref(),
                ])?;
            }
        } else {
            let mut statement = tx.prepare(
                "DELETE FROM source_progress
                 WHERE extension_id = ?1 AND show_id = ?2 AND episode_id = ?3",
            )?;
            let mut history_statement = tx.prepare(
                "DELETE FROM source_history
                 WHERE extension_id = ?1 AND show_id = ?2 AND episode_id = ?3",
            )?;
            for episode in episodes {
                statement.execute(params![extension_id, show_id, episode.episode_id])?;
                history_statement.execute(params![extension_id, show_id, episode.episode_id])?;
            }
        }
        tx.execute(
            "UPDATE source_library SET updated_at = unixepoch()
             WHERE extension_id = ?1 AND show_id = ?2",
            params![extension_id, show_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn source_continue(&self, limit: usize) -> AppResult<Vec<SourceContinueItem>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT sp.extension_id, COALESCE(je.name, sp.extension_id),
                    sp.show_id, sl.title, sl.cover, sl.anilist_id,
                    sp.episode_id, sp.episode_number, sp.episode_title,
                    sp.position_sec, sp.duration_sec, sp.watched, sp.updated_at
             FROM (
               SELECT latest.*, ROW_NUMBER() OVER (
                 PARTITION BY latest.extension_id, latest.show_id
                 ORDER BY latest.watched ASC, latest.updated_at DESC, latest.episode_number DESC
               ) AS rank
               FROM source_progress latest
               WHERE latest.continue_visible = 1
                 AND (latest.position_sec > 0 OR latest.watched = 1)
             ) sp
             JOIN source_library sl
               ON sl.extension_id = sp.extension_id AND sl.show_id = sp.show_id
             LEFT JOIN js_extensions je ON je.id = sp.extension_id
             WHERE sp.rank = 1
               AND je.id IS NOT NULL
             ORDER BY sp.updated_at DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit as i64], |row| {
            Ok(SourceContinueItem {
                extension_id: row.get(0)?,
                extension_name: row.get(1)?,
                show_id: row.get(2)?,
                title: row.get(3)?,
                cover: row.get(4)?,
                anilist_id: row.get(5)?,
                episode_id: row.get(6)?,
                episode_number: row.get(7)?,
                episode_title: row.get(8)?,
                position_sec: row.get(9)?,
                duration_sec: row.get(10)?,
                watched: row.get::<_, i64>(11)? != 0,
                updated_at: row.get(12)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn source_history(&self, limit: usize) -> AppResult<Vec<SourceHistoryItem>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT sh.extension_id, COALESCE(je.name, sh.extension_id),
                    sh.show_id, sl.title, sl.cover,
                    sh.episode_id, sh.episode_number, sh.episode_title, sh.watched, sh.updated_at
             FROM source_history sh
             JOIN source_library sl
               ON sl.extension_id = sh.extension_id AND sl.show_id = sh.show_id
             LEFT JOIN js_extensions je ON je.id = sh.extension_id
             WHERE je.id IS NOT NULL
             ORDER BY sh.updated_at DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit as i64], |row| {
            Ok(SourceHistoryItem {
                extension_id: row.get(0)?,
                extension_name: row.get(1)?,
                show_id: row.get(2)?,
                title: row.get(3)?,
                cover: row.get(4)?,
                episode_id: row.get(5)?,
                episode_number: row.get(6)?,
                episode_title: row.get(7)?,
                watched: row.get::<_, i64>(8)? != 0,
                updated_at: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn source_history_remove(
        &self,
        extension_id: &str,
        show_id: &str,
        episode_id: &str,
    ) -> AppResult<()> {
        validate_source_keys(extension_id, show_id)?;
        check_source_len("episode id", episode_id, MAX_SOURCE_ID_BYTES)?;
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "DELETE FROM source_history
             WHERE extension_id = ?1 AND show_id = ?2 AND episode_id = ?3",
            params![extension_id, show_id, episode_id],
        )?;
        Ok(())
    }

    #[cfg(debug_assertions)]
    pub fn list_sources(&self) -> AppResult<Vec<InstalledSource>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT id, name, version, enabled, manifest FROM sources ORDER BY name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            let manifest: String = row.get(4)?;
            let parsed = serde_json::from_str::<LegacyExtension>(&manifest).ok();
            Ok(InstalledSource {
                id: row.get(0)?,
                name: row.get(1)?,
                version: row.get(2)?,
                enabled: row.get::<_, i64>(3)? != 0,
                source_count: parsed
                    .as_ref()
                    .map(LegacyExtension::source_count)
                    .unwrap_or(0),
                nsfw: parsed
                    .as_ref()
                    .map(LegacyExtension::is_nsfw)
                    .unwrap_or(false),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    #[cfg(debug_assertions)]
    pub fn install_source(&self, entry: &LegacyExtension) -> AppResult<()> {
        if !entry.valid() {
            return Err(AppError::Blocked(
                "Extension metadata is invalid".to_string(),
            ));
        }
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let manifest = serde_json::to_string(entry).map_err(|err| AppError::Db(err.to_string()))?;
        conn.execute(
            "INSERT INTO sources(id, name, version, enabled, manifest, installed_at)
             VALUES (?1, ?2, ?3, 1, ?4, unixepoch())
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                version = excluded.version,
                manifest = excluded.manifest",
            params![entry.pkg, entry.name, entry.version, manifest],
        )?;
        Ok(())
    }

    #[cfg(debug_assertions)]
    pub fn remove_source(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute("DELETE FROM sources WHERE id = ?1", params![id])?;
        Ok(())
    }

    #[cfg(debug_assertions)]
    pub fn toggle_source(&self, id: &str, enabled: bool) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "UPDATE sources SET enabled = ?2 WHERE id = ?1",
            params![id, i64::from(enabled)],
        )?;
        Ok(())
    }

    pub fn backup_to(&self, path: &str) -> AppResult<u64> {
        let target = absolute_path(path)?;
        let target = crate::path_guard::resolve_write_target(&target, "backup target")
            .map_err(AppError::Blocked)?;
        let mut temp = crate::atomic_file::TempFile::create(&target)
            .map_err(|err| AppError::Db(format!("cannot create backup file: {err}")))?;
        // SQLite accepts a zero-length file as the VACUUM INTO destination.
        temp.close();
        let temp_raw = temp.path().to_string_lossy().into_owned();

        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute("VACUUM INTO ?1", params![temp_raw])?;
        temp.persist(&target)
            .map_err(|err| AppError::Db(format!("cannot replace backup: {err}")))?;

        let bytes = std::fs::metadata(&target)
            .map_err(|err| AppError::Db(format!("cannot read backup size: {err}")))?
            .len();
        Ok(bytes)
    }

    pub fn restore_from(&self, path: &str) -> AppResult<()> {
        let source_path = absolute_path(path)?;
        let source_path = crate::path_guard::resolve_read_file(&source_path, "backup")
            .map_err(AppError::Blocked)?;
        validate_backup_file(&source_path, MAX_BACKUP_BYTES)?;
        validate_backup_database(&source_path)?;

        // Build and migrate the restored database in a scratch file first.
        // The live database is touched only after that copy is complete and
        // passes its own integrity check.
        let (parent, name) = database_parent_and_name(&self.path);
        let temp = parent.join(format!(".{name}.restore-{}.db", timestamp_nanos()));
        remove_database_files(&temp);
        if let Err(err) = prepare_restore_database(&source_path, &temp) {
            remove_database_files(&temp);
            return Err(err);
        }

        let safety = safety_copy_path(&self.path);
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        if let Err(err) = write_safety_copy(&conn, &safety) {
            remove_database_files(&temp);
            return Err(err);
        }

        if let Err(err) = replace_live_database(&mut conn, &temp) {
            let rollback = restore_safety_copy(&safety, &mut conn);
            remove_database_files(&temp);
            return match rollback {
                Ok(()) => Err(AppError::Db(format!(
                    "restore failed: {err}; the previous library was put back from the safety copy at {}",
                    safety.display()
                ))),
                Err(rollback_err) => Err(AppError::Db(format!(
                    "restore failed: {err}; putting the safety copy back also failed: {rollback_err}. A copy of the previous library is kept at {}",
                    safety.display()
                ))),
            };
        }

        remove_database_files(&temp);
        let removed = cleanup_safety_copies(&self.path);
        if removed > 0 {
            tracing::info!(
                removed,
                "restore: removed old safety copies, keeping the newest three"
            );
        }
        Ok(())
    }

    /// Clears all rows by closing the on-disk database and deleting its file,
    /// WAL and SHM. The process keeps a fresh in-memory database so commands
    /// that arrive before the restart still work. If Windows still holds a
    /// file open, the marker makes the next start retry before opening a new
    /// database.
    pub fn clear_local_data(&self) -> AppResult<bool> {
        let (parent, _) = database_parent_and_name(&self.path);
        let marker = parent.join(CLEAR_DATA_MARKER);

        let replacement = Connection::open_in_memory()?;
        apply_pragmas(&replacement)?;
        apply_migrations(&replacement)?;
        std::fs::write(&marker, b"1").map_err(|err| {
            AppError::Db(format!(
                "cannot write the clear-data marker at {}: {err}",
                marker.display()
            ))
        })?;

        let old = {
            let mut conn = self.conn.lock().map_err(|_| db_locked())?;
            std::mem::replace(&mut *conn, replacement)
        };
        old.close()
            .map_err(|(_, err)| AppError::Db(format!("cannot close the local database: {err}")))?;

        let all_removed = remove_database_files_and_copies(&self.path);
        if all_removed {
            let _ = std::fs::remove_file(&marker);
        }
        Ok(all_removed)
    }
}

fn absolute_path(path: &str) -> AppResult<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::Blocked("backup path is empty".to_string()));
    }
    let target = PathBuf::from(trimmed);
    if !target.is_absolute() {
        return Err(AppError::Blocked(
            "backup path must be absolute".to_string(),
        ));
    }
    Ok(target)
}

fn validate_backup_file(path: &Path, max_bytes: u64) -> AppResult<()> {
    let metadata = std::fs::metadata(path)
        .map_err(|err| AppError::Blocked(format!("cannot read backup: {err}")))?;
    if !metadata.is_file() {
        return Err(AppError::Blocked("backup path is not a file".to_string()));
    }
    if metadata.len() == 0 {
        return Err(AppError::Blocked("backup is empty".to_string()));
    }
    if metadata.len() > max_bytes {
        return Err(AppError::Blocked(format!(
            "backup is too large ({} bytes; limit is {max_bytes} bytes)",
            metadata.len()
        )));
    }
    Ok(())
}

fn validate_backup_database(path: &Path) -> AppResult<()> {
    let source = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| AppError::Blocked(format!("cannot open backup: {err}")))?;
    check_integrity(&source).map_err(|err| {
        AppError::Blocked(format!("backup failed the SQLite integrity check: {err}"))
    })?;

    let version: i64 = source
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|err| AppError::Blocked(format!("cannot read backup schema version: {err}")))?;
    if version > SCHEMA_VERSION {
        return Err(AppError::Blocked(format!(
            "backup was made by a newer Kaminari version (schema {version}; this build supports {SCHEMA_VERSION})"
        )));
    }

    let mut problems = Vec::new();
    for &(table, columns) in REQUIRED_TABLES {
        validate_table(&source, table, columns, false, &mut problems)?;
    }
    for &(table, columns) in OPTIONAL_TABLES {
        validate_table(&source, table, columns, true, &mut problems)?;
    }
    if !problems.is_empty() {
        return Err(AppError::Blocked(format!(
            "not a Kaminari database: {}",
            problems.join(", ")
        )));
    }
    Ok(())
}

fn validate_table(
    conn: &Connection,
    table: &str,
    required_columns: &[&str],
    optional: bool,
    problems: &mut Vec<String>,
) -> AppResult<()> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![table],
            |row| row.get(0),
        )
        .map_err(|err| AppError::Blocked(format!("cannot inspect backup schema: {err}")))?;
    if !exists {
        if !optional {
            problems.push(format!("missing table {table}"));
        }
        return Ok(());
    }

    let present = table_columns(conn, table)?;
    for column in required_columns {
        if !present.contains(*column) {
            problems.push(format!("table {table} is missing column {column}"));
        }
    }
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> AppResult<HashSet<String>> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = HashSet::new();
    for column in rows {
        columns.insert(column?);
    }
    Ok(columns)
}

fn check_integrity(conn: &Connection) -> Result<(), String> {
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|err| err.to_string())?;
    if verdict == "ok" {
        Ok(())
    } else {
        Err(verdict)
    }
}

fn prepare_restore_database(source_path: &Path, temp: &Path) -> AppResult<()> {
    let source = Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| AppError::Blocked(format!("cannot open backup: {err}")))?;
    let mut temp_conn = Connection::open(temp)
        .map_err(|err| AppError::Db(format!("cannot create restored database: {err}")))?;
    copy_database(&source, &mut temp_conn)?;
    apply_pragmas(&temp_conn)?;
    apply_migrations(&temp_conn)?;
    // Disable packages in the prepared copy as well, so a crash between the
    // live page copy and the final UPDATE cannot leave them enabled.
    temp_conn.execute(
        "UPDATE js_extensions SET enabled = 0, trust = 'sideloaded'",
        [],
    )?;
    // Leave the scratch database as a single file: a read-only reopen below
    // must not depend on a WAL that was never checkpointed.
    temp_conn
        .pragma_update(None, "journal_mode", "DELETE")
        .map_err(|err| AppError::Db(format!("cannot finalize restored database: {err}")))?;
    check_integrity(&temp_conn).map_err(|err| {
        AppError::Db(format!(
            "restored database failed the SQLite integrity check: {err}"
        ))
    })?;
    Ok(())
}

fn replace_live_database(conn: &mut Connection, temp: &Path) -> AppResult<()> {
    let restored = Connection::open_with_flags(temp, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| AppError::Db(format!("cannot open the prepared backup: {err}")))?;
    copy_database(&restored, conn)?;
    apply_pragmas(conn)?;
    apply_migrations(conn)?;
    // A package from an untrusted backup was never reviewed by this
    // installation. Keep the row so the user can inspect it, but never let its
    // code run until it is explicitly enabled again, and do not preserve a
    // trust label the backup supplied.
    conn.execute(
        "UPDATE js_extensions SET enabled = 0, trust = 'sideloaded'",
        [],
    )?;
    check_integrity(conn).map_err(|err| {
        AppError::Db(format!(
            "restored library failed the SQLite integrity check: {err}"
        ))
    })
}

fn restore_safety_copy(safety: &Path, conn: &mut Connection) -> AppResult<()> {
    let safety_conn = Connection::open_with_flags(safety, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| AppError::Db(format!("cannot reopen the safety copy: {err}")))?;
    copy_database(&safety_conn, conn)?;
    apply_pragmas(conn)?;
    Ok(())
}

fn write_safety_copy(conn: &Connection, safety: &Path) -> AppResult<()> {
    conn.execute("VACUUM INTO ?1", params![safety.to_string_lossy()])
        .map_err(|err| {
            AppError::Db(format!(
                "cannot create a safety copy of the live library at {}: {err}",
                safety.display()
            ))
        })?;
    Ok(())
}

fn copy_database(source: &Connection, target: &mut Connection) -> AppResult<()> {
    let backup = Backup::new(source, target)?;
    backup.run_to_completion(100, Duration::from_millis(10), None)?;
    Ok(())
}

fn database_parent_and_name(path: &Path) -> (PathBuf, String) {
    if path.as_os_str() == ":memory:" {
        return (
            std::env::temp_dir(),
            format!("kaminari-memory-{}", std::process::id()),
        );
    }
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "kaminari.db".to_string());
    (parent, name)
}

fn safety_copy_path(live: &Path) -> PathBuf {
    let (parent, name) = database_parent_and_name(live);
    parent.join(format!("{name}.safety-{}.db", timestamp_nanos()))
}

/// Restore keeps a copy of the library it replaces. Three is enough for a
/// recent mistake without turning the data folder into an unbounded archive.
pub(crate) const SAFETY_COPY_LIMIT: usize = 3;

/// Removes older `kaminari.db.safety-<nanos>.db` files, keeping the newest
/// `SAFETY_COPY_LIMIT`. The live database, WAL/SHM sidecars, quarantine folders
/// and scratch restore files do not match and are never touched.
pub(crate) fn cleanup_safety_copies(db_path: &Path) -> usize {
    if db_path.as_os_str() == ":memory:" {
        return 0;
    }
    let (parent, name) = database_parent_and_name(db_path);
    let prefix = format!("{name}.safety-");
    let entries = match std::fs::read_dir(&parent) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return 0,
        Err(err) => {
            tracing::warn!(
                path = %parent.display(),
                error = %err,
                "safety copies: database folder could not be inspected"
            );
            return 0;
        }
    };

    let mut copies: Vec<(u128, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Some(stamp) = file_name.strip_prefix(&prefix) else {
            continue;
        };
        let Some(stamp) = stamp.strip_suffix(".db") else {
            continue;
        };
        if stamp.is_empty() || !stamp.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let Ok(stamp) = stamp.parse::<u128>() else {
            continue;
        };
        copies.push((stamp, entry.path()));
    }

    if copies.len() <= SAFETY_COPY_LIMIT {
        return 0;
    }
    copies.sort_by_key(|copy| std::cmp::Reverse(copy.0));
    let mut removed = 0usize;
    for (_, path) in copies.into_iter().skip(SAFETY_COPY_LIMIT) {
        match std::fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!(
                path = %path.display(),
                error = %err,
                "safety copies: an old copy could not be removed"
            ),
        }
    }
    removed
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default()
}

fn remove_database_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(sidecar_path(path, "-wal"));
    let _ = std::fs::remove_file(sidecar_path(path, "-shm"));
}

/// Removes the safety copies and quarantine folders Kaminari itself creates next
/// to the live database. A clear-all request must not leave an older library
/// behind under one of those names.
pub(crate) fn remove_database_copies(db_path: &Path) -> bool {
    if db_path.as_os_str() == ":memory:" {
        return true;
    }
    let (parent, name) = database_parent_and_name(db_path);
    let entries = match std::fs::read_dir(&parent) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return true,
        Err(err) => {
            tracing::warn!(
                path = %parent.display(),
                error = %err,
                "clear: database folder could not be inspected"
            );
            return false;
        }
    };
    let safety = format!("{name}.safety-");
    let quarantine = format!("{name}.quarantine-");
    let restore = format!(".{name}.restore-");
    let legacy_backup = format!("{name}.bak");
    let mut all_removed = true;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name != legacy_backup
            && !file_name.starts_with(&safety)
            && !file_name.starts_with(&quarantine)
            && !file_name.starts_with(&restore)
        {
            continue;
        }
        let path = entry.path();
        let result = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if let Err(err) = result {
            if err.kind() != std::io::ErrorKind::NotFound {
                all_removed = false;
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "clear: database copy could not be removed; the next start will retry"
                );
            }
        }
    }
    all_removed
}

/// Removes the live database, its WAL/SHM sidecars and the safety/quarantine
/// copies that belong to it. Returns false when a file is still open; callers
/// keep the clear marker so the next start retries.
pub(crate) fn remove_database_files_and_copies(db_path: &Path) -> bool {
    let mut all_removed = true;
    for path in [
        db_path.to_path_buf(),
        sidecar_path(db_path, "-wal"),
        sidecar_path(db_path, "-shm"),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                all_removed = false;
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "clear: database file could not be removed; the next start will retry"
                );
            }
        }
    }
    if !remove_database_copies(db_path) {
        all_removed = false;
    }
    all_removed
}

/// Encrypts legacy plaintext `js_storage` rows in one transaction. Rows that
/// already carry the `dpapi:` family prefix are left alone even when they are
/// malformed, so a corrupt ciphertext is never encrypted a second time.
fn migrate_plaintext_storage(conn: &Connection) -> AppResult<usize> {
    let rows: Vec<(i64, String)> = {
        let mut statement = conn.prepare(
            "SELECT rowid, value FROM js_storage
             WHERE substr(value, 1, 6) <> 'dpapi:'",
        )?;
        let mapped = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    if rows.is_empty() {
        return Ok(0);
    }

    let transaction = conn.unchecked_transaction()?;
    let mut migrated = 0usize;
    for (rowid, value) in rows {
        match storage_crypto::encrypt(&value) {
            Ok(encrypted) => {
                migrated += transaction.execute(
                    "UPDATE js_storage SET value = ?1 WHERE rowid = ?2 AND value = ?3",
                    params![encrypted, rowid, value],
                )?;
            }
            Err(err) => tracing::warn!(
                error = %err,
                "extension storage: startup migration skipped one plaintext row"
            ),
        }
    }
    transaction.commit()?;
    Ok(migrated)
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut raw = path.as_os_str().to_os_string();
    raw.push(suffix);
    PathBuf::from(raw)
}

fn apply_pragmas(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(())
}

fn apply_migrations(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(include_str!("../migrations/0001_init.sql"))?;
    conn.execute_batch(include_str!("../migrations/0002_watched.sql"))?;
    conn.execute_batch(include_str!("../migrations/0003_js_extensions.sql"))?;
    conn.execute_batch(include_str!("../migrations/0004_source_library.sql"))?;
    conn.execute_batch(include_str!("../migrations/0005_anilist_sync.sql"))?;
    conn.execute_batch(include_str!("../migrations/0006_source_history.sql"))?;
    if !has_column(conn, "source_library", "anilist_id")? {
        conn.execute(
            "ALTER TABLE source_library ADD COLUMN anilist_id INTEGER",
            [],
        )?;
    }
    conn.execute_batch(include_str!("../migrations/0007_source_anilist.sql"))?;
    conn.execute_batch(include_str!("../migrations/0008_airing_schedule.sql"))?;
    // Trust is earned from the install path, never from a package, a caller or
    // an older backup. Drop legacy labels such as `signed`.
    conn.execute(
        "UPDATE js_extensions SET trust = 'sideloaded'
         WHERE trust NOT IN ('builtin', 'sideloaded')",
        [],
    )?;
    backfill_continue_visible(conn)?;
    dedupe_source_anilist_links(conn)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Adds `source_progress.continue_visible` and preserves pre-upgrade Continue
/// rows in one transaction. The `app_meta` marker makes the repair one-time:
/// if an older upgrade was interrupted after `ALTER TABLE` but before the
/// backfill, a later startup still repairs the hidden rows. Once the marker is
/// present, a row the user deliberately hid stays hidden.
fn backfill_continue_visible(conn: &Connection) -> AppResult<()> {
    const BACKFILL_KEY: &str = "source_progress_continue_visible_backfilled";
    let transaction = conn.unchecked_transaction()?;
    let marked: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM app_meta WHERE key = ?1 AND value = '1')",
        params![BACKFILL_KEY],
        |row| row.get(0),
    )?;
    if marked {
        return Ok(());
    }
    if !has_column(&transaction, "source_progress", "continue_visible")? {
        transaction.execute(
            "ALTER TABLE source_progress
             ADD COLUMN continue_visible INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    transaction.execute(
        "UPDATE source_progress SET continue_visible = 1
         WHERE watched = 0 AND position_sec > 0 AND continue_visible = 0",
        [],
    )?;
    transaction.execute(
        "INSERT OR REPLACE INTO app_meta(key, value) VALUES (?1, '1')",
        params![BACKFILL_KEY],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Older builds could link two source shows to the same AniList entry, which
/// let one season queue progress to another. Keep the newest row's link per
/// extension and AniList id; clear the rest. The source rows stay.
fn dedupe_source_anilist_links(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE source_library SET anilist_id = NULL
         WHERE anilist_id IS NOT NULL
           AND rowid NOT IN (
             SELECT MAX(rowid) FROM source_library
             WHERE anilist_id IS NOT NULL
             GROUP BY extension_id, anilist_id
           )",
        [],
    )?;
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> AppResult<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn anilist_library_status(status: &str) -> Option<&'static str> {
    match status {
        "CURRENT" | "REPEATING" => Some("watching"),
        "PLANNING" => Some("planning"),
        "COMPLETED" => Some("completed"),
        "PAUSED" => Some("paused"),
        "DROPPED" => Some("dropped"),
        _ => None,
    }
}

fn upsert_media(conn: &Connection, media: &Media) -> AppResult<()> {
    let genres = media
        .genres
        .as_ref()
        .map(|list| serde_json::to_string(list).unwrap_or_else(|_| "[]".to_string()));
    let cover = media
        .cover_image
        .as_ref()
        .and_then(|image| image.extra_large.clone().or_else(|| image.large.clone()))
        .unwrap_or_default();
    conn.execute(
        "INSERT INTO media(
            anilist_id, title_romaji, title_english, title_native, cover, banner,
            year, format, status, episodes, score, genres, synopsis, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, unixepoch())
         ON CONFLICT(anilist_id) DO UPDATE SET
            title_romaji = excluded.title_romaji,
            title_english = excluded.title_english,
            title_native = excluded.title_native,
            cover = excluded.cover,
            banner = excluded.banner,
            year = excluded.year,
            format = excluded.format,
            status = excluded.status,
            episodes = excluded.episodes,
            score = excluded.score,
            genres = excluded.genres,
            synopsis = excluded.synopsis,
            updated_at = unixepoch()",
        params![
            media.id,
            media.title.romaji.clone().unwrap_or_default(),
            &media.title.english,
            &media.title.native,
            &cover,
            &media.banner_image,
            media.season_year,
            &media.format,
            &media.status,
            media.episodes,
            media.average_score,
            &genres,
            &media.description,
        ],
    )?;
    Ok(())
}

fn ensure_library_entry(conn: &Connection, anilist_id: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO library(anilist_id, status, added_at, updated_at)
         VALUES (?1, 'watching', unixepoch(), unixepoch())
         ON CONFLICT(anilist_id) DO NOTHING",
        params![anilist_id],
    )?;
    Ok(())
}

fn enqueue_anilist_sync_if_in_library(conn: &Connection, anilist_id: i64) -> AppResult<bool> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM library WHERE anilist_id = ?1",
            params![anilist_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(status) = status else {
        return Ok(false);
    };
    enqueue_anilist_sync(conn, anilist_id, &status)?;
    Ok(true)
}

fn enqueue_anilist_sync(conn: &Connection, anilist_id: i64, status: &str) -> AppResult<()> {
    let progress = watched_count(conn, anilist_id)?;
    conn.execute(
        "INSERT INTO anilist_sync_queue(
            anilist_id, status, progress, updated_at, attempts, last_error
         ) VALUES (?1, ?2, ?3, unixepoch(), 0, NULL)
         ON CONFLICT(anilist_id) DO UPDATE SET
            status = excluded.status,
            progress = excluded.progress,
            updated_at = unixepoch(),
            attempts = 0,
            last_error = NULL",
        params![anilist_id, status, progress],
    )?;
    Ok(())
}

fn watched_count(conn: &Connection, anilist_id: i64) -> AppResult<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM watched WHERE anilist_id = ?1",
        params![anilist_id],
        |row| row.get(0),
    )
    .map_err(AppError::from)
}

fn pending_anilist_sync_count(conn: &Connection) -> AppResult<usize> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM anilist_sync_queue", [], |row| {
        row.get(0)
    })?;
    Ok(count as usize)
}

fn read_anilist_queue(conn: &Connection) -> AppResult<Vec<AniListSyncRow>> {
    let mut statement = conn.prepare(
        "SELECT anilist_id, status, progress, updated_at, attempts, last_error
         FROM anilist_sync_queue
         ORDER BY updated_at, anilist_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(AniListSyncRow {
            anilist_id: row.get(0)?,
            status: row.get(1)?,
            progress: row.get(2)?,
            updated_at: row.get(3)?,
            attempts: row.get(4)?,
            last_error: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Deterministic exponential backoff with up to 25 percent jitter. Attempt 1
/// waits about 30 seconds; the cap is 30 minutes.
pub fn anilist_sync_backoff_secs(anilist_id: i64, attempts: i64) -> i64 {
    if attempts <= 0 {
        return 0;
    }
    let exponent = (attempts - 1).min(6) as u32;
    let base = ANILIST_SYNC_BACKOFF_BASE_SECS
        .saturating_mul(1i64 << exponent)
        .min(ANILIST_SYNC_BACKOFF_MAX_SECS);
    let jitter_span = (base / 4).max(1) as u64;
    let mut seed = (anilist_id as u64) ^ ((attempts as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15u64;
    seed ^= seed >> 30;
    seed = seed.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    seed ^= seed >> 27;
    base + (seed % jitter_span) as i64
}

fn anilist_sync_row_due(row: &AniListSyncRow, now: i64) -> bool {
    if row.attempts <= 0 {
        return true;
    }
    let delay = anilist_sync_backoff_secs(row.anilist_id, row.attempts);
    now >= row.updated_at.saturating_add(delay)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

/// True when the schedule snapshot is missing or older than 15 minutes.
pub fn airing_schedule_stale(fetched_at: Option<i64>, now: i64) -> bool {
    match fetched_at {
        None => true,
        Some(stamped) => now.saturating_sub(stamped) >= AIRING_SCHEDULE_FRESH_SECS,
    }
}

/// True when the schedule must fetch. A changed id set forces a fetch even
/// inside the freshness window. Staleness alone does not fetch here; the
/// view triggers a background force refresh when stale is true.
pub fn airing_schedule_should_fetch(
    force: bool,
    fetched_at: Option<i64>,
    stored_ids: &[i64],
    current_ids: &[i64],
) -> bool {
    if force {
        return true;
    }
    if fetched_at.is_none() {
        return true;
    }
    if stored_ids.len() != current_ids.len() {
        return true;
    }
    let mut stored = stored_ids.to_vec();
    let mut current = current_ids.to_vec();
    stored.sort_unstable();
    current.sort_unstable();
    stored != current
}

/// Unix timestamp before which activity is older than the selected retention.
/// Rows exactly at the cutoff are kept, so a 30 day setting removes entries
/// strictly older than 30 days.
pub fn history_retention_cutoff(now: i64, days: u32) -> i64 {
    now.saturating_sub(i64::from(days) * 86_400)
}

fn code_hash(code: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    hex::encode(hasher.finalize())
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

fn install_preview(
    conn: &Connection,
    manifest: &Manifest,
    code: &str,
    trust: ExtensionTrust,
) -> AppResult<ExtensionInstallPreview> {
    let manifest_value =
        serde_json::to_value(manifest).map_err(|err| AppError::Db(err.to_string()))?;
    let existing = conn
        .query_row(
            "SELECT name, version, api_version, manifest, code, trust
             FROM js_extensions WHERE id = ?1",
            params![manifest.id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;

    let mut preview = ExtensionInstallPreview {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        api_version: manifest.api_version.clone(),
        trust: trust.as_str().to_string(),
        new_install: existing.is_none(),
        requires_approval: false,
        code_changed: false,
        name_changed: false,
        version_changed: false,
        manifest_changed: false,
        trust_changed: false,
        added_hosts: Vec::new(),
        removed_hosts: Vec::new(),
        existing_name: None,
        existing_version: None,
        existing_trust: None,
        changes: Vec::new(),
    };
    let Some((name, version, api_version, old_manifest_raw, old_code, old_trust)) = existing else {
        return Ok(preview);
    };

    preview.existing_name = Some(name.clone());
    preview.existing_version = Some(version.clone());
    preview.existing_trust = Some(old_trust.clone());
    preview.name_changed = name != manifest.name;
    preview.version_changed = version != manifest.version;
    preview.code_changed = old_code != code;
    preview.trust_changed = old_trust != trust.as_str();
    let old_manifest = serde_json::from_str::<Manifest>(&old_manifest_raw).ok();
    let old_manifest_value = serde_json::from_str::<serde_json::Value>(&old_manifest_raw).ok();
    preview.manifest_changed = old_manifest_value.as_ref() != Some(&manifest_value);

    if preview.name_changed {
        preview.changes.push(ExtensionInstallChange {
            field: "name".to_string(),
            before: name,
            after: manifest.name.clone(),
        });
    }
    if preview.version_changed {
        preview.changes.push(ExtensionInstallChange {
            field: "version".to_string(),
            before: version,
            after: manifest.version.clone(),
        });
    }
    if api_version != manifest.api_version {
        preview.changes.push(ExtensionInstallChange {
            field: "apiVersion".to_string(),
            before: api_version,
            after: manifest.api_version.clone(),
        });
    }
    if preview.code_changed {
        preview.changes.push(ExtensionInstallChange {
            field: "code".to_string(),
            before: short_hash(&code_hash(&old_code)),
            after: short_hash(&code_hash(code)),
        });
    }
    if preview.trust_changed {
        preview.changes.push(ExtensionInstallChange {
            field: "trust".to_string(),
            before: old_trust,
            after: trust.as_str().to_string(),
        });
    }
    if let Some(old_manifest) = old_manifest {
        if old_manifest.base_url != manifest.base_url {
            preview.changes.push(ExtensionInstallChange {
                field: "baseUrl".to_string(),
                before: old_manifest.base_url,
                after: manifest.base_url.clone(),
            });
        }
        if old_manifest.nsfw != manifest.nsfw {
            preview.changes.push(ExtensionInstallChange {
                field: "nsfw".to_string(),
                before: old_manifest.nsfw.to_string(),
                after: manifest.nsfw.to_string(),
            });
        }
        let old_hosts: HashSet<&str> = old_manifest
            .permissions
            .hosts
            .iter()
            .map(String::as_str)
            .collect();
        let new_hosts: HashSet<&str> = manifest
            .permissions
            .hosts
            .iter()
            .map(String::as_str)
            .collect();
        preview.added_hosts = manifest
            .permissions
            .hosts
            .iter()
            .filter(|host| !old_hosts.contains(host.as_str()))
            .cloned()
            .collect();
        preview.removed_hosts = old_manifest
            .permissions
            .hosts
            .iter()
            .filter(|host| !new_hosts.contains(host.as_str()))
            .cloned()
            .collect();
    } else {
        preview.added_hosts = manifest.permissions.hosts.clone();
    }
    if preview.manifest_changed
        && preview.changes.is_empty()
        && preview.added_hosts.is_empty()
        && preview.removed_hosts.is_empty()
    {
        preview.changes.push(ExtensionInstallChange {
            field: "manifest".to_string(),
            before: "installed package".to_string(),
            after: "new package".to_string(),
        });
    }
    preview.requires_approval = preview.manifest_changed
        || preview.code_changed
        || preview.name_changed
        || preview.version_changed
        || preview.trust_changed
        || !preview.added_hosts.is_empty()
        || !preview.removed_hosts.is_empty()
        || !preview.changes.is_empty();
    Ok(preview)
}

fn apply_watched(conn: &Connection, anilist_id: i64, episode: i64, watched: bool) -> AppResult<()> {
    if watched {
        conn.execute(
            "INSERT OR REPLACE INTO watched(anilist_id, episode, watched_at)
             VALUES (?1, ?2, unixepoch())",
            params![anilist_id, episode],
        )?;
        conn.execute(
            "INSERT INTO history(anilist_id, episode, watched_at)
             VALUES (?1, ?2, unixepoch())",
            params![anilist_id, episode],
        )?;
        conn.execute(
            "INSERT INTO progress(anilist_id, episode, position_sec, duration_sec, updated_at)
             VALUES (?1, ?2, 0, 0, unixepoch())
             ON CONFLICT(anilist_id) DO UPDATE SET
                episode = MAX(progress.episode, excluded.episode),
                updated_at = unixepoch()",
            params![anilist_id, episode],
        )?;
    } else {
        conn.execute(
            "DELETE FROM watched WHERE anilist_id = ?1 AND episode = ?2",
            params![anilist_id, episode],
        )?;
        conn.execute(
            "DELETE FROM history WHERE anilist_id = ?1 AND episode = ?2",
            params![anilist_id, episode],
        )?;
        let latest: Option<i64> = conn.query_row(
            "SELECT MAX(episode) FROM watched WHERE anilist_id = ?1",
            params![anilist_id],
            |row| row.get(0),
        )?;
        match latest {
            Some(value) => {
                conn.execute(
                    "UPDATE progress SET episode = ?2, updated_at = unixepoch()
                     WHERE anilist_id = ?1",
                    params![anilist_id, value],
                )?;
            }
            None => {
                conn.execute(
                    "DELETE FROM progress WHERE anilist_id = ?1",
                    params![anilist_id],
                )?;
            }
        }
    }
    Ok(())
}

fn read_watched(conn: &Connection, anilist_id: i64) -> AppResult<Vec<i64>> {
    let mut statement =
        conn.prepare("SELECT episode FROM watched WHERE anilist_id = ?1 ORDER BY episode")?;
    let rows = statement.query_map(params![anilist_id], |row| row.get(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn db_locked() -> AppError {
    AppError::Db("database lock poisoned".to_string())
}

fn check_source_len(label: &str, value: &str, max: usize) -> AppResult<()> {
    if value.len() > max {
        return Err(AppError::Blocked(format!(
            "{label} is too long ({} bytes; limit is {max})",
            value.len()
        )));
    }
    Ok(())
}

fn check_optional_source_len(label: &str, value: Option<&str>, max: usize) -> AppResult<()> {
    match value {
        Some(value) => check_source_len(label, value, max),
        None => Ok(()),
    }
}

fn check_source_episode_number(value: i64) -> AppResult<()> {
    if !(0..=MAX_SOURCE_EPISODE_NUMBER).contains(&value) {
        return Err(AppError::Blocked(format!(
            "episode number must be between 0 and {MAX_SOURCE_EPISODE_NUMBER}"
        )));
    }
    Ok(())
}

fn check_source_seconds(label: &str, value: f64) -> AppResult<()> {
    if !value.is_finite() || value < 0.0 || value > MAX_SOURCE_SECONDS {
        return Err(AppError::Blocked(format!(
            "{label} must be a finite number between 0 and {MAX_SOURCE_SECONDS} seconds"
        )));
    }
    Ok(())
}

fn validate_source_keys(extension_id: &str, show_id: &str) -> AppResult<()> {
    check_source_len("extension id", extension_id, MAX_SOURCE_EXTENSION_ID_BYTES)?;
    check_source_len("show id", show_id, MAX_SOURCE_ID_BYTES)
}

pub(crate) fn validate_source_library_input(
    extension_id: &str,
    show_id: &str,
    title: &str,
    cover: Option<&str>,
    status: Option<&str>,
) -> AppResult<()> {
    validate_source_keys(extension_id, show_id)?;
    check_source_len("title", title, MAX_SOURCE_TITLE_BYTES)?;
    check_optional_source_len("cover", cover, MAX_SOURCE_COVER_BYTES)?;
    check_optional_source_len("status", status, MAX_SOURCE_STATUS_BYTES)
}

pub(crate) fn validate_source_progress_input(
    extension_id: &str,
    show_id: &str,
    episode_id: &str,
    episode_number: i64,
    episode_title: Option<&str>,
    position_sec: f64,
    duration_sec: f64,
) -> AppResult<()> {
    validate_source_keys(extension_id, show_id)?;
    check_source_len("episode id", episode_id, MAX_SOURCE_ID_BYTES)?;
    check_optional_source_len(
        "episode title",
        episode_title,
        MAX_SOURCE_EPISODE_TITLE_BYTES,
    )?;
    check_source_episode_number(episode_number)?;
    check_source_seconds("position", position_sec)?;
    check_source_seconds("duration", duration_sec)
}

pub(crate) fn validate_source_progress_marks(
    extension_id: &str,
    show_id: &str,
    episodes: &[SourceProgressMark],
) -> AppResult<()> {
    validate_source_keys(extension_id, show_id)?;
    if episodes.len() > MAX_SOURCE_EPISODES_PER_SHOW {
        return Err(AppError::Blocked(format!(
            "too many episodes in one request (limit is {MAX_SOURCE_EPISODES_PER_SHOW})"
        )));
    }
    for episode in episodes {
        check_source_len("episode id", &episode.episode_id, MAX_SOURCE_ID_BYTES)?;
        check_optional_source_len(
            "episode title",
            episode.episode_title.as_deref(),
            MAX_SOURCE_EPISODE_TITLE_BYTES,
        )?;
        check_source_episode_number(episode.episode_number)?;
        check_source_seconds("episode duration", episode.duration_sec)?;
    }
    Ok(())
}

fn clamp_watched_through(through: i64, episodes: Option<i32>) -> i64 {
    let media_limit = episodes
        .filter(|value| *value > 0)
        .map(i64::from)
        .unwrap_or(MAX_ANILIST_PROGRESS)
        .min(MAX_ANILIST_PROGRESS);
    through.clamp(0, media_limit)
}

impl ExtStorage for Db {
    fn get(&self, extension_id: &str, key: &str) -> Option<String> {
        let conn = self.conn.lock().ok()?;
        let stored: String = conn
            .query_row(
                "SELECT value FROM js_storage WHERE extension_id = ?1 AND key = ?2",
                params![extension_id, key],
                |row| row.get(0),
            )
            .optional()
            .ok()??;

        if !storage_crypto::is_encrypted(&stored) {
            // Legacy plaintext row. Return it now even if the in-place upgrade
            // fails, so an extension is never locked out of its own data.
            match storage_crypto::encrypt(&stored) {
                Ok(encrypted) => {
                    if let Err(err) = conn.execute(
                        "UPDATE js_storage SET value = ?1
                         WHERE extension_id = ?2 AND key = ?3 AND value = ?4",
                        params![encrypted, extension_id, key, stored],
                    ) {
                        tracing::warn!(
                            extension = extension_id,
                            key,
                            error = %err,
                            "extension storage: plaintext row could not be migrated on read"
                        );
                    }
                }
                Err(err) => tracing::warn!(
                    extension = extension_id,
                    key,
                    error = %err,
                    "extension storage: plaintext row could not be encrypted on read"
                ),
            }
            return Some(stored);
        }

        match storage_crypto::decrypt(&stored) {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!(
                    extension = extension_id,
                    key,
                    error = %err,
                    "extension storage: unreadable encrypted value ignored"
                );
                None
            }
        }
    }

    fn set(&self, extension_id: &str, key: &str, value: &str) -> AppResult<()> {
        let encrypted = storage_crypto::encrypt(value)?;
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let used = storage_usage(&conn, extension_id)?;
        let previous = conn
            .query_row(
                "SELECT value FROM js_storage WHERE extension_id = ?1 AND key = ?2",
                params![extension_id, key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let previous_bytes = previous
            .as_deref()
            .map(|stored| key.len() + storage_plaintext_len(stored))
            .unwrap_or(0);
        if used.saturating_sub(previous_bytes) + key.len() + value.len() > 1024 * 1024 {
            return Err(AppError::Blocked(
                "extension storage limit reached (1 MB)".to_string(),
            ));
        }
        conn.execute(
            "INSERT INTO js_storage(extension_id, key, value, updated_at)
             VALUES (?1, ?2, ?3, unixepoch())
             ON CONFLICT(extension_id, key) DO UPDATE SET
                value = excluded.value,
                updated_at = unixepoch()",
            params![extension_id, key, encrypted],
        )?;
        Ok(())
    }

    fn remove(&self, extension_id: &str, key: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "DELETE FROM js_storage WHERE extension_id = ?1 AND key = ?2",
            params![extension_id, key],
        )?;
        Ok(())
    }
}

/// The quota stays measured in plaintext bytes, even though SQLite now holds
/// ciphertext. A malformed row counts as its stored length so it cannot make
/// the quota smaller than it looks.
fn storage_usage(conn: &Connection, extension_id: &str) -> AppResult<usize> {
    let mut statement =
        conn.prepare("SELECT key, value FROM js_storage WHERE extension_id = ?1")?;
    let rows = statement.query_map(params![extension_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut used = 0usize;
    for row in rows {
        let (key, stored) = row?;
        used = used
            .saturating_add(key.len())
            .saturating_add(storage_plaintext_len(&stored));
    }
    Ok(used)
}

fn storage_plaintext_len(stored: &str) -> usize {
    if storage_crypto::is_encrypted(stored) {
        storage_crypto::decrypt(stored)
            .map(|value| value.len())
            .unwrap_or(stored.len())
    } else {
        stored.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext_host::ExtStorage;
    #[cfg(debug_assertions)]
    use crate::registry::LegacySource;

    fn sample_media() -> Media {
        Media {
            id: 15125,
            title: MediaTitle {
                romaji: Some("Sousou no Frieren".to_string()),
                english: Some("Frieren: Beyond Journey's End".to_string()),
                native: None,
            },
            cover_image: Some(CoverImage {
                extra_large: Some("https://img.example/frieren.jpg".to_string()),
                large: None,
                color: None,
            }),
            banner_image: None,
            description: Some("An elf mage walks on.".to_string()),
            genres: Some(vec!["Adventure".to_string()]),
            average_score: Some(91),
            season_year: Some(2023),
            format: Some("TV".to_string()),
            status: Some("FINISHED".to_string()),
            episodes: Some(28),
            recommendations: None,
        }
    }

    #[test]
    fn add_list_and_remove() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        let items = db.list_library().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].media.id, 15125);
        assert_eq!(items[0].status, "watching");
        assert_eq!(items[0].watched_count, 0);
        db.remove_from_library(15125).unwrap();
        assert!(db.list_library().unwrap().is_empty());
    }

    fn airing_media(id: i64) -> Media {
        let mut media = sample_media();
        media.id = id;
        media
    }

    #[test]
    fn airing_ids_union_library_and_sources_without_dropped() {
        let db = Db::new(":memory:").unwrap();
        db.add_to_library(&airing_media(1), "watching").unwrap();
        db.add_to_library(&airing_media(2), "planning").unwrap();
        db.add_to_library(&airing_media(3), "dropped").unwrap();
        db.source_library_add(
            "ext.test",
            "show-9",
            "Linked Nine",
            None,
            None,
            None,
            Some(9),
        )
        .unwrap();
        db.source_library_add(
            "ext.test",
            "show-2",
            "Linked Two",
            None,
            None,
            None,
            Some(2),
        )
        .unwrap();
        db.source_library_add("ext.test", "show-x", "Unlinked", None, None, None, None)
            .unwrap();

        let ids = db.airing_schedule_ids().unwrap();
        assert_eq!(ids, vec![1, 2, 9]);
    }

    #[test]
    fn airing_replace_caches_entries_in_order() {
        let db = Db::new(":memory:").unwrap();
        let now = unix_now();
        let entries = vec![
            AiringScheduleEntry {
                anilist_id: 2,
                episode: 5,
                airing_at: now + 7200,
            },
            AiringScheduleEntry {
                anilist_id: 1,
                episode: 8,
                airing_at: now + 3600,
            },
        ];
        let stamped = db.airing_schedule_replace(&[1, 2], &entries).unwrap();
        let cached = db.airing_schedule_cached().unwrap();
        assert_eq!(cached.len(), 2);
        assert_eq!(cached[0].anilist_id, 1);
        assert_eq!(cached[1].anilist_id, 2);
        assert_eq!(db.airing_schedule_fetched_at().unwrap(), Some(stamped));
        assert_eq!(db.airing_schedule_id_set().unwrap(), vec![1, 2]);
    }

    #[test]
    fn airing_replace_removes_ids_missing_from_library() {
        let db = Db::new(":memory:").unwrap();
        let now = unix_now();
        db.airing_schedule_replace(
            &[1, 2],
            &[
                AiringScheduleEntry {
                    anilist_id: 1,
                    episode: 8,
                    airing_at: now + 3600,
                },
                AiringScheduleEntry {
                    anilist_id: 2,
                    episode: 3,
                    airing_at: now + 7200,
                },
            ],
        )
        .unwrap();
        db.airing_schedule_replace(
            &[1],
            &[AiringScheduleEntry {
                anilist_id: 1,
                episode: 9,
                airing_at: now + 9000,
            }],
        )
        .unwrap();
        let cached = db.airing_schedule_cached().unwrap();
        assert!(cached.iter().all(|entry| entry.anilist_id == 1));
        assert_eq!(db.airing_schedule_id_set().unwrap(), vec![1]);
    }

    #[test]
    fn airing_replace_keeps_recent_past_and_drops_old() {
        let db = Db::new(":memory:").unwrap();
        let now = unix_now();
        db.airing_schedule_replace(
            &[1],
            &[AiringScheduleEntry {
                anilist_id: 1,
                episode: 7,
                airing_at: now + 3600,
            }],
        )
        .unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO airing_schedule(anilist_id, episode, airing_at) VALUES (?1, ?2, ?3)",
                params![1, 6, now - 47 * 3600],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO airing_schedule(anilist_id, episode, airing_at) VALUES (?1, ?2, ?3)",
                params![1, 5, now - 49 * 3600],
            )
            .unwrap();
        }
        db.airing_schedule_replace(
            &[1],
            &[AiringScheduleEntry {
                anilist_id: 1,
                episode: 8,
                airing_at: now + 7200,
            }],
        )
        .unwrap();
        let cached = db.airing_schedule_cached().unwrap();
        let pairs: Vec<(i64, i64)> = cached
            .iter()
            .map(|entry| (entry.episode, entry.airing_at))
            .collect();
        assert!(pairs.iter().any(|(ep, _)| *ep == 6), "{pairs:?}");
        assert!(!pairs.iter().any(|(ep, _)| *ep == 5), "{pairs:?}");
        assert!(pairs.iter().any(|(ep, _)| *ep == 8), "{pairs:?}");
        assert!(!pairs.iter().any(|(ep, _)| *ep == 7), "{pairs:?}");
    }

    #[test]
    fn airing_id_set_change_forces_fetch() {
        assert!(airing_schedule_should_fetch(true, Some(100), &[1], &[1]));
        assert!(airing_schedule_should_fetch(false, None, &[], &[1]));
        assert!(!airing_schedule_should_fetch(
            false,
            Some(100),
            &[1, 2],
            &[2, 1]
        ));
        assert!(airing_schedule_should_fetch(
            false,
            Some(100),
            &[1, 2],
            &[1, 3]
        ));
        assert!(airing_schedule_should_fetch(
            false,
            Some(100),
            &[1],
            &[1, 2]
        ));
    }

    #[test]
    fn airing_stale_boundary_uses_fifteen_minutes() {
        let now = 1_790_000_000;
        assert!(airing_schedule_stale(None, now));
        assert!(!airing_schedule_stale(Some(now - 14 * 60 - 59), now));
        assert!(airing_schedule_stale(Some(now - 15 * 60 - 1), now));
        assert!(!airing_schedule_stale(Some(now + 60), now));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn install_toggle_remove_source() {
        let db = Db::new(":memory:").unwrap();
        let entry = LegacyExtension {
            name: "Aniyomi: AnimeOnsen".to_string(),
            pkg: "eu.kanade.tachiyomi.animeextension.all.animeonsen".to_string(),
            apk: "aniyomi-all.animeonsen-v14.10.apk".to_string(),
            lang: "all".to_string(),
            code: 10,
            version: "14.10".to_string(),
            nsfw: 0,
            torrent: 0,
            sources: vec![LegacySource {
                name: "AnimeOnsen".to_string(),
                lang: "all".to_string(),
                id: "8542735178285060053".to_string(),
                base_url: "https://www.animeonsen.xyz".to_string(),
            }],
        };
        db.install_source(&entry).unwrap();
        let list = db.list_sources().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].source_count, 1);
        assert!(list[0].enabled);
        assert!(!list[0].nsfw);

        db.toggle_source(&entry.pkg, false).unwrap();
        assert!(!db.list_sources().unwrap()[0].enabled);

        db.remove_source(&entry.pkg).unwrap();
        assert!(db.list_sources().unwrap().is_empty());
    }

    #[test]
    fn install_js_extension_and_storage() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();

        let list = db.list_js_extensions().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "app.kaminari.publicdomain");
        assert_eq!(list[0].hosts, vec!["https://archive.org/*"]);
        assert!(!list[0].nsfw);
        assert!(db.js_extension(&package.manifest.id).unwrap().is_some());

        ExtStorage::set(&db, &package.manifest.id, "token", "abc").unwrap();
        assert_eq!(
            ExtStorage::get(&db, &package.manifest.id, "token"),
            Some("abc".to_string())
        );

        db.remove_js_extension(&package.manifest.id).unwrap();
        assert!(db.list_js_extensions().unwrap().is_empty());
        assert_eq!(ExtStorage::get(&db, &package.manifest.id, "token"), None);
    }

    fn scratch_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "kaminari-db-{name}-{}-{}",
            std::process::id(),
            timestamp_nanos()
        ))
    }

    fn raw_storage_value(db: &Db, extension_id: &str, key: &str) -> String {
        db.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM js_storage WHERE extension_id = ?1 AND key = ?2",
                params![extension_id, key],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn extension_storage_round_trips_and_writes_only_ciphertext() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        let id = &package.manifest.id;

        for value in ["secret-token", "日本語", ""] {
            ExtStorage::set(&db, id, "round-trip", value).unwrap();
            assert_eq!(
                ExtStorage::get(&db, id, "round-trip"),
                Some(value.to_string())
            );
            let raw = raw_storage_value(&db, id, "round-trip");
            assert!(raw.starts_with(storage_crypto::PREFIX), "{raw}");
            if !value.is_empty() {
                assert!(!raw.contains(value), "{raw}");
            }
        }
    }

    #[test]
    fn plaintext_storage_row_migrates_on_read() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        let id = &package.manifest.id;
        db.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO js_storage(extension_id, key, value, updated_at)
                 VALUES (?1, 'legacy', 'old plaintext', unixepoch())",
                params![id],
            )
            .unwrap();

        assert_eq!(
            ExtStorage::get(&db, id, "legacy"),
            Some("old plaintext".to_string())
        );
        let raw = raw_storage_value(&db, id, "legacy");
        assert!(raw.starts_with(storage_crypto::PREFIX));
        assert!(!raw.contains("old plaintext"));
        assert_eq!(storage_crypto::decrypt(&raw).unwrap(), "old plaintext");
    }

    #[test]
    fn startup_pass_encrypts_existing_plaintext_rows() {
        let dir = scratch_dir("migrate");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kaminari.db");
        let path_text = path.to_string_lossy().into_owned();
        let package = crate::ext_host::sample_package().unwrap();
        {
            let db = Db::new(&path_text).unwrap();
            db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
                .unwrap();
            db.conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO js_storage(extension_id, key, value, updated_at)
                     VALUES (?1, 'legacy', 'startup plaintext', unixepoch())",
                    params![&package.manifest.id],
                )
                .unwrap();
        }

        let reopened = Db::new(&path_text).unwrap();
        let raw = raw_storage_value(&reopened, &package.manifest.id, "legacy");
        assert!(raw.starts_with(storage_crypto::PREFIX));
        assert!(!raw.contains("startup plaintext"));
        assert_eq!(
            ExtStorage::get(&reopened, &package.manifest.id, "legacy"),
            Some("startup plaintext".to_string())
        );
        drop(reopened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_ciphertext_is_ignored_without_panicking() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        let id = &package.manifest.id;
        for (key, raw) in [
            ("bad-base64", "dpapi:v1:not base64"),
            ("bad-version", "dpapi:v2:AAAA"),
            ("bad-ciphertext", "dpapi:v1:AAAA"),
        ] {
            db.conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO js_storage(extension_id, key, value, updated_at)
                     VALUES (?1, ?2, ?3, unixepoch())",
                    params![id, key, raw],
                )
                .unwrap();
            assert_eq!(ExtStorage::get(&db, id, key), None);
            assert_eq!(raw_storage_value(&db, id, key), raw);
        }
    }

    #[test]
    fn clear_local_data_removes_database_and_sidecars() {
        let dir = scratch_dir("clear");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kaminari.db");
        let path_text = path.to_string_lossy().into_owned();
        let db = Db::new(&path_text).unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        ExtStorage::set(&db, &package.manifest.id, "token", "still-secret").unwrap();
        let safety = dir.join("kaminari.db.safety-1.db");
        let quarantine = dir.join("kaminari.db.quarantine-1");
        let legacy_backup = dir.join("kaminari.db.bak");
        std::fs::write(&safety, b"older library").unwrap();
        std::fs::write(&legacy_backup, b"older library").unwrap();
        std::fs::create_dir_all(&quarantine).unwrap();
        std::fs::write(quarantine.join("kaminari.db"), b"older library").unwrap();

        assert!(db.clear_local_data().unwrap());
        assert!(!path.exists());
        assert!(!sidecar_path(&path, "-wal").exists());
        assert!(!sidecar_path(&path, "-shm").exists());
        assert!(!dir.join(CLEAR_DATA_MARKER).exists());
        assert!(!safety.exists());
        assert!(!legacy_backup.exists());
        assert!(!quarantine.exists());
        assert!(db.list_js_extensions().unwrap().is_empty());

        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extension_update_preview_detects_replacement_changes() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();

        let unchanged = db
            .extension_install_preview(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        assert!(!unchanged.new_install);
        assert!(!unchanged.requires_approval);
        assert!(unchanged.changes.is_empty());

        let mut updated = package.manifest.clone();
        updated.name = "Updated Animation".to_string();
        updated.version = "9.9.9".to_string();
        updated.permissions.hosts = vec![
            "https://archive.org/*".to_string(),
            "https://cdn.example/*".to_string(),
        ];
        let new_code = format!("{}\n// replaced", package.code);
        let preview = db
            .extension_install_preview(&updated, &new_code, ExtensionTrust::Sideloaded)
            .unwrap();
        assert!(preview.requires_approval);
        assert!(preview.name_changed);
        assert!(preview.version_changed);
        assert!(preview.code_changed);
        assert!(preview.trust_changed);
        assert_eq!(preview.added_hosts, vec!["https://cdn.example/*"]);
        assert_eq!(
            preview.existing_name.as_deref(),
            Some("Public Domain Animation")
        );

        // A create-only install refuses to replace a different package.
        let error = db
            .install_js_extension(&updated, &new_code, ExtensionTrust::Sideloaded)
            .unwrap_err();
        assert!(error.to_string().contains("review the update"));

        db.replace_js_extension(&updated, &new_code, ExtensionTrust::Sideloaded)
            .unwrap();
        let list = db.list_js_extensions().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Updated Animation");
        assert_eq!(list[0].version, "9.9.9");
        assert_eq!(list[0].trust, "sideloaded");
        assert_eq!(list[0].hosts.len(), 2);
    }

    #[test]
    fn extension_preview_reports_a_new_install() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        let preview = db
            .extension_install_preview(&package.manifest, &package.code, ExtensionTrust::Sideloaded)
            .unwrap();
        assert!(preview.new_install);
        assert!(!preview.requires_approval);
        assert!(preview.changes.is_empty());
        assert_eq!(preview.trust, "sideloaded");
    }

    #[test]
    fn unwatch_without_library_does_not_queue_anilist_write() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.set_watched(&media, 1, true).unwrap();
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 1);

        // Removing the title is a local negative action and must cancel the
        // write that was queued by the positive action before it.
        db.remove_from_library(media.id).unwrap();
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 0);
        assert!(db.list_library().unwrap().is_empty());

        db.set_watched(&media, 1, false).unwrap();
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 0);
        assert!(db.list_library().unwrap().is_empty());
    }

    #[test]
    fn unwatch_in_library_keeps_the_users_status() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "planning").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        let row = db.anilist_sync_row(media.id).unwrap().unwrap();
        assert_eq!(row.status, "planning");
        assert_eq!(row.progress, 1);

        db.set_watched(&media, 1, false).unwrap();
        let row = db.anilist_sync_row(media.id).unwrap().unwrap();
        assert_eq!(row.status, "planning");
        assert_eq!(row.progress, 0);
    }

    #[test]
    fn clearing_a_completed_title_keeps_completed_status() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "completed").unwrap();
        db.mark_all_watched(&media, 5, true).unwrap();
        db.mark_all_watched(&media, 5, false).unwrap();
        let row = db.anilist_sync_row(media.id).unwrap().unwrap();
        assert_eq!(row.status, "completed");
        assert_eq!(row.progress, 0);
    }

    #[test]
    fn anilist_backoff_grows_and_caps() {
        assert_eq!(anilist_sync_backoff_secs(1, 0), 0);
        let first = anilist_sync_backoff_secs(1, 1);
        assert!(
            (ANILIST_SYNC_BACKOFF_BASE_SECS..ANILIST_SYNC_BACKOFF_BASE_SECS * 2).contains(&first)
        );
        assert!(
            anilist_sync_backoff_secs(1, 2) > anilist_sync_backoff_secs(1, 1),
            "backoff must grow with attempts"
        );
        let capped = anilist_sync_backoff_secs(1, 99);
        assert!(capped >= ANILIST_SYNC_BACKOFF_MAX_SECS);
        assert!(capped <= ANILIST_SYNC_BACKOFF_MAX_SECS * 5 / 4);
        // Different titles do not all retry at the same second.
        assert_ne!(
            anilist_sync_backoff_secs(1, 3),
            anilist_sync_backoff_secs(2, 3)
        );
    }

    #[test]
    fn pending_sync_due_skips_backed_off_rows_and_caps_the_batch() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        let mut second = sample_media();
        second.id = 2;
        db.add_to_library(&second, "watching").unwrap();
        db.set_watched(&second, 1, true).unwrap();

        db.fail_anilist_sync(media.id, "offline").unwrap();
        let due = db.pending_anilist_sync_due(10).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].anilist_id, second.id);

        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE anilist_sync_queue SET updated_at = unixepoch() - 3600
                 WHERE anilist_id = ?1",
                params![media.id],
            )
            .unwrap();
        }
        assert_eq!(db.pending_anilist_sync_due(10).unwrap().len(), 2);
        assert_eq!(db.pending_anilist_sync_due(1).unwrap().len(), 1);
    }

    #[test]
    fn completing_sync_only_removes_the_pushed_snapshot() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        let snapshot = db.anilist_sync_row(media.id).unwrap().unwrap();

        // A newer local edit keeps the row and must survive the old delete,
        // even when both edits land in the same wall-clock second.
        db.set_watched(&media, 2, true).unwrap();
        assert!(!db.complete_anilist_sync(&snapshot).unwrap());
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 1);

        let fresh = db.anilist_sync_row(media.id).unwrap().unwrap();
        assert!(db.complete_anilist_sync(&fresh).unwrap());
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 0);
    }

    #[test]
    fn list_js_extensions_exposes_manifest_nsfw() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        let mut manifest = package.manifest.clone();
        manifest.nsfw = true;
        db.install_js_extension(&manifest, &package.code, ExtensionTrust::Sideloaded)
            .unwrap();

        let list = db.list_js_extensions().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].nsfw);
    }

    #[test]
    fn source_library_and_progress() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();

        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            Some("https://example.com/cover.jpg"),
            Some(1933),
            Some("finished"),
            Some(9),
        )
        .unwrap();
        let items = db.source_library_list().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].extension_name, "Public Domain Animation");
        assert_eq!(items[0].anilist_id, Some(9));
        assert_eq!(items[0].watched_count, 0);

        // A later metadata refresh without a link must not clear the durable
        // link, while an explicit new link replaces it.
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            Some("https://example.com/cover.jpg"),
            Some(1933),
            Some("finished"),
            None,
        )
        .unwrap();
        assert_eq!(db.source_library_list().unwrap()[0].anilist_id, Some(9));
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            Some("https://example.com/cover.jpg"),
            Some(1933),
            Some("finished"),
            Some(10),
        )
        .unwrap();
        assert_eq!(db.source_library_list().unwrap()[0].anilist_id, Some(10));
        // Restore the original link used by the rest of this test.
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            Some("https://example.com/cover.jpg"),
            Some(1933),
            Some("finished"),
            Some(9),
        )
        .unwrap();

        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-1",
            1,
            Some("Episode 1"),
            120.0,
            600.0,
            false,
        )
        .unwrap();
        let progress = db
            .source_progress_get(&package.manifest.id, "show-1")
            .unwrap();
        assert_eq!(progress.len(), 1);
        assert_eq!(progress[0].position_sec, 120.0);
        let items = db.source_library_list().unwrap();
        assert_eq!(items[0].last_episode, Some(1));
        assert_eq!(items[0].last_episode_id.as_deref(), Some("ep-1"));
        assert_eq!(items[0].last_position, Some(120.0));

        let continue_items = db.source_continue(10).unwrap();
        assert_eq!(continue_items.len(), 1);
        assert_eq!(continue_items[0].episode_number, 1);
        assert_eq!(continue_items[0].position_sec, 120.0);
        assert_eq!(continue_items[0].anilist_id, Some(9));
        let history = db.source_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].episode_id, "ep-1");
        assert!(!history[0].watched);

        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-2",
            2,
            Some("Episode 2"),
            30.0,
            600.0,
            false,
        )
        .unwrap();
        let continue_items = db.source_continue(10).unwrap();
        assert_eq!(continue_items.len(), 1);
        assert_eq!(continue_items[0].episode_number, 2);
        assert_eq!(continue_items[0].position_sec, 30.0);
        assert_eq!(db.source_history(10).unwrap().len(), 2);
        let items = db.source_library_list().unwrap();
        assert_eq!(items[0].last_episode, Some(2));
        assert_eq!(items[0].last_episode_id.as_deref(), Some("ep-2"));

        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-1",
            1,
            Some("Episode 1"),
            600.0,
            600.0,
            true,
        )
        .unwrap();
        assert_eq!(db.source_library_list().unwrap()[0].watched_count, 1);
        assert_eq!(db.source_continue(10).unwrap()[0].episode_number, 2);
        assert!(db.source_history(10).unwrap().iter().any(|row| row.watched));

        db.source_library_remove(&package.manifest.id, "show-1")
            .unwrap();
        assert!(db.source_library_list().unwrap().is_empty());
        assert!(db
            .source_progress_get(&package.manifest.id, "show-1")
            .unwrap()
            .is_empty());
        assert!(db.source_history(10).unwrap().is_empty());
    }

    #[test]
    fn source_library_keeps_one_anilist_link_per_extension() {
        let db = Db::new(":memory:").unwrap();
        db.source_library_add(
            "ext.one",
            "season-1",
            "Season 1",
            None,
            None,
            None,
            Some(176314),
        )
        .unwrap();
        db.source_library_add(
            "ext.one",
            "season-2",
            "Season 2",
            None,
            None,
            None,
            Some(176314),
        )
        .unwrap();

        let season_1: Option<i64> = {
            let conn = db.conn.lock().unwrap();
            conn.query_row(
                "SELECT anilist_id FROM source_library
                 WHERE extension_id = 'ext.one' AND show_id = 'season-1'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        let season_2: Option<i64> = {
            let conn = db.conn.lock().unwrap();
            conn.query_row(
                "SELECT anilist_id FROM source_library
                 WHERE extension_id = 'ext.one' AND show_id = 'season-2'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(season_1, None);
        assert_eq!(season_2, Some(176314));

        // A different extension may link its own copy of the same show.
        db.source_library_add(
            "ext.two",
            "season-1",
            "Season 1",
            None,
            None,
            None,
            Some(176314),
        )
        .unwrap();
        let season_1_other: Option<i64> = {
            let conn = db.conn.lock().unwrap();
            conn.query_row(
                "SELECT anilist_id FROM source_library
                 WHERE extension_id = 'ext.two' AND show_id = 'season-1'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(season_1_other, Some(176314));
    }

    #[test]
    fn legacy_duplicate_anilist_links_are_deduped_on_open() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO source_library(
                extension_id, show_id, title, anilist_id, added_at, updated_at
             ) VALUES ('ext.one', 'season-1', 'Season 1', 176314, 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO source_library(
                extension_id, show_id, title, anilist_id, added_at, updated_at
             ) VALUES ('ext.one', 'season-2', 'Season 2', 176314, 2, 2)",
            [],
        )
        .unwrap();

        apply_migrations(&conn).unwrap();

        let season_1: Option<i64> = conn
            .query_row(
                "SELECT anilist_id FROM source_library WHERE show_id = 'season-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let season_2: Option<i64> = conn
            .query_row(
                "SELECT anilist_id FROM source_library WHERE show_id = 'season-2'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(season_1, None);
        assert_eq!(season_2, Some(176314));
    }

    #[test]
    fn removing_extension_deletes_source_rows_and_hides_orphans() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            Some("https://example.com/cover.jpg"),
            Some(1933),
            Some("finished"),
            None,
        )
        .unwrap();
        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-1",
            1,
            Some("Episode 1"),
            30.0,
            600.0,
            false,
        )
        .unwrap();
        // A legacy orphan from a source that is already gone.
        db.source_library_add(
            "removed.source",
            "orphan-show",
            "Orphan",
            None,
            None,
            None,
            None,
        )
        .unwrap();
        db.source_progress_set(
            "removed.source",
            "orphan-show",
            "ep-1",
            1,
            None,
            30.0,
            600.0,
            false,
        )
        .unwrap();

        assert_eq!(db.source_library_list().unwrap().len(), 1);
        assert_eq!(db.source_continue(10).unwrap().len(), 1);
        assert_eq!(db.source_history(10).unwrap().len(), 1);

        db.remove_js_extension(&package.manifest.id).unwrap();

        assert!(db.source_library_list().unwrap().is_empty());
        assert!(db.source_continue(10).unwrap().is_empty());
        assert!(db.source_history(10).unwrap().is_empty());
        assert!(db
            .source_progress_get(&package.manifest.id, "show-1")
            .unwrap()
            .is_empty());

        let conn = db.conn.lock().unwrap();
        for table in ["source_library", "source_progress", "source_history"] {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE extension_id = ?1");
            let count: i64 = conn
                .query_row(&sql, params![package.manifest.id], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0, "{table} kept rows for the removed extension");
        }
        // Orphan rows are hidden from every list, but deleting one extension
        // must not delete another extension's rows.
        let orphans: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_library WHERE extension_id = 'removed.source'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 1);
    }

    #[test]
    fn episode_counts_are_clamped_for_watch_all_and_anilist() {
        let db = Db::new(":memory:").unwrap();

        let mut bounded = sample_media();
        bounded.episodes = Some(3);
        let watched = db.mark_all_watched(&bounded, i64::MAX, true).unwrap();
        assert_eq!(watched, vec![1, 2, 3]);

        let mut unbounded = sample_media();
        unbounded.id = 42;
        unbounded.title.romaji = Some("Unbounded".to_string());
        unbounded.episodes = None;
        let watched = db.mark_all_watched(&unbounded, i64::MAX, true).unwrap();
        assert_eq!(watched.len(), MAX_ANILIST_PROGRESS as usize);
        assert_eq!(watched.last().copied(), Some(MAX_ANILIST_PROGRESS));
        assert!(db.set_watched(&unbounded, -1, true).is_err());
        assert!(db.set_watched(&unbounded, i64::MAX, true).is_err());

        let mut pulled = sample_media();
        pulled.id = 77;
        pulled.title.romaji = Some("Pulled".to_string());
        pulled.episodes = None;
        let entries = vec![AniListListEntry {
            media: pulled,
            status: "CURRENT".to_string(),
            progress: i64::MAX,
        }];
        db.sync_anilist_entries(&entries).unwrap();
        let row = db
            .list_library()
            .unwrap()
            .into_iter()
            .find(|item| item.media.id == 77)
            .unwrap();
        assert_eq!(row.watched_count, MAX_ANILIST_PROGRESS);
    }

    #[test]
    fn source_metadata_limits_reject_oversized_and_nonfinite_values() {
        let db = Db::new(":memory:").unwrap();

        let title = "x".repeat(MAX_SOURCE_TITLE_BYTES + 1);
        assert!(db
            .source_library_add("ext", "show", &title, None, None, None, None)
            .is_err());
        let cover = "x".repeat(MAX_SOURCE_COVER_BYTES + 1);
        assert!(db
            .source_library_add("ext", "show", "ok", Some(&cover), None, None, None)
            .is_err());
        let status = "x".repeat(MAX_SOURCE_STATUS_BYTES + 1);
        assert!(db
            .source_library_add("ext", "show", "ok", None, None, Some(&status), None)
            .is_err());
        let long_id = "x".repeat(MAX_SOURCE_ID_BYTES + 1);
        assert!(db
            .source_progress_set("ext", "show", &long_id, 1, None, 0.0, 0.0, false)
            .is_err());
        let long_episode_title = "x".repeat(MAX_SOURCE_EPISODE_TITLE_BYTES + 1);
        assert!(db
            .source_progress_set(
                "ext",
                "show",
                "ep",
                1,
                Some(&long_episode_title),
                0.0,
                0.0,
                false
            )
            .is_err());

        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            MAX_SOURCE_SECONDS + 1.0,
        ] {
            assert!(db
                .source_progress_set("ext", "show", "ep", 1, None, value, 0.0, false)
                .is_err());
            assert!(db
                .source_progress_set("ext", "show", "ep", 1, None, 0.0, value, false)
                .is_err());
        }
        assert!(db
            .source_progress_set("ext", "show", "ep", -1, None, 0.0, 0.0, false)
            .is_err());
        assert!(db
            .source_progress_set(
                "ext",
                "show",
                "ep",
                MAX_SOURCE_EPISODE_NUMBER + 1,
                None,
                0.0,
                0.0,
                false
            )
            .is_err());
        db.source_progress_set("ext", "show", "ep", 1, None, 0.0, 0.0, false)
            .unwrap();

        let marks = (0..=MAX_SOURCE_EPISODES_PER_SHOW)
            .map(|index| SourceProgressMark {
                episode_id: format!("ep-{index}"),
                episode_number: index as i64,
                episode_title: None,
                duration_sec: 0.0,
            })
            .collect::<Vec<_>>();
        assert!(db
            .source_progress_mark_all("ext", "show", &marks, true)
            .is_err());
        db.source_progress_mark_all(
            "ext",
            "marked",
            &marks[..MAX_SOURCE_EPISODES_PER_SHOW],
            true,
        )
        .unwrap();
        assert!(db
            .source_progress_mark_all(
                "ext",
                "marked",
                &marks[MAX_SOURCE_EPISODES_PER_SHOW..],
                true
            )
            .is_err());

        for index in 0..MAX_SOURCE_EPISODES_PER_SHOW as i64 {
            db.source_progress_set(
                "ext",
                "full",
                &format!("ep-{index}"),
                index,
                None,
                0.0,
                0.0,
                false,
            )
            .unwrap();
        }
        assert!(db
            .source_progress_set(
                "ext",
                "full",
                "one-too-many",
                MAX_SOURCE_EPISODES_PER_SHOW as i64,
                None,
                0.0,
                0.0,
                false
            )
            .is_err());
    }

    #[test]
    fn source_progress_mark_all_updates_library_and_continue() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
            None,
            None,
        )
        .unwrap();

        let marks = vec![
            SourceProgressMark {
                episode_id: "ep-1".to_string(),
                episode_number: 1,
                episode_title: Some("Episode 1".to_string()),
                duration_sec: 600.0,
            },
            SourceProgressMark {
                episode_id: "ep-2".to_string(),
                episode_number: 2,
                episode_title: None,
                duration_sec: 0.0,
            },
            SourceProgressMark {
                episode_id: "ep-3".to_string(),
                episode_number: 3,
                episode_title: Some("Episode 3".to_string()),
                duration_sec: 720.0,
            },
        ];

        db.source_progress_mark_all(&package.manifest.id, "show-1", &marks, true)
            .unwrap();
        let progress = db
            .source_progress_get(&package.manifest.id, "show-1")
            .unwrap();
        assert_eq!(progress.len(), 3);
        assert!(progress.iter().all(|row| row.watched));
        assert_eq!(progress[0].position_sec, 600.0);
        assert_eq!(progress[2].position_sec, 720.0);
        assert_eq!(db.source_library_list().unwrap()[0].watched_count, 3);
        // Watched rows are Continue-eligible, so the shelf keeps one card per
        // show with the newest episode as the resume target.
        let continue_items = db.source_continue(10).unwrap();
        assert_eq!(continue_items.len(), 1);
        assert_eq!(continue_items[0].episode_number, 3);
        assert!(continue_items[0].watched);
        assert_eq!(db.source_history(10).unwrap().len(), 3);

        db.source_progress_mark_all(&package.manifest.id, "show-1", &marks, false)
            .unwrap();
        assert!(db
            .source_progress_get(&package.manifest.id, "show-1")
            .unwrap()
            .is_empty());
        assert_eq!(db.source_library_list().unwrap()[0].watched_count, 0);
        assert!(db.source_history(10).unwrap().is_empty());
        assert!(db.source_continue(10).unwrap().is_empty());

        // Re-marking watched restores the rewatch-eligible rows.
        db.source_progress_mark_all(&package.manifest.id, "show-1", &marks, true)
            .unwrap();
        let continue_items = db.source_continue(10).unwrap();
        assert_eq!(continue_items.len(), 1);
        assert!(continue_items[0].watched);
        assert_eq!(continue_items[0].episode_number, 3);
    }

    #[test]
    fn zero_duration_mark_all_returns_to_continue() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
            None,
            None,
        )
        .unwrap();
        let marks = vec![
            SourceProgressMark {
                episode_id: "ep-1".to_string(),
                episode_number: 1,
                episode_title: Some("Episode 1".to_string()),
                duration_sec: 0.0,
            },
            SourceProgressMark {
                episode_id: "ep-2".to_string(),
                episode_number: 2,
                episode_title: Some("Episode 2".to_string()),
                duration_sec: 0.0,
            },
        ];

        db.source_progress_mark_all(&package.manifest.id, "show-1", &marks, true)
            .unwrap();
        assert_eq!(db.source_library_list().unwrap()[0].watched_count, 2);
        // Zero-duration manual marks are Continue-eligible again. One card per
        // show, with the newest episode as the resume target.
        let continue_items = db.source_continue(10).unwrap();
        assert_eq!(continue_items.len(), 1);
        assert_eq!(continue_items[0].episode_number, 2);
        assert!(continue_items[0].watched);
    }

    #[test]
    fn history_clear_removes_activity_and_keeps_progress() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        db.set_watched(&media, 2, true).unwrap();

        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
            None,
            None,
        )
        .unwrap();
        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-1",
            1,
            Some("Episode 1"),
            120.0,
            600.0,
            false,
        )
        .unwrap();
        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-2",
            2,
            Some("Episode 2"),
            600.0,
            600.0,
            true,
        )
        .unwrap();

        assert_eq!(db.history_list(10).unwrap().len(), 2);
        assert_eq!(db.source_history(10).unwrap().len(), 2);

        db.history_clear().unwrap();

        assert!(db.history_list(10).unwrap().is_empty());
        assert!(db.source_history(10).unwrap().is_empty());

        let library = db.list_library().unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].watched_count, 2);
        assert_eq!(db.watched_list(media.id).unwrap(), vec![1, 2]);
        assert!(db.continue_watching(10).unwrap().is_empty());
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 1);

        let progress = db
            .source_progress_get(&package.manifest.id, "show-1")
            .unwrap();
        assert_eq!(progress.len(), 2);
        assert!(progress.iter().all(|row| row.position_sec == 0.0));
        assert!(progress.iter().all(|row| row.duration_sec == 600.0));
        assert!(
            progress
                .iter()
                .find(|row| row.episode_id == "ep-2")
                .unwrap()
                .watched
        );
        assert!(
            !progress
                .iter()
                .find(|row| row.episode_id == "ep-1")
                .unwrap()
                .watched
        );
        assert!(db.source_continue(10).unwrap().is_empty());
        let source_library = db.source_library_list().unwrap();
        assert_eq!(source_library.len(), 1);
        assert_eq!(source_library[0].watched_count, 1);

        // A later startup replays migrations; cleared activity must stay gone.
        {
            let conn = db.conn.lock().unwrap();
            apply_migrations(&conn).unwrap();
        }
        assert!(db.source_history(10).unwrap().is_empty());
    }

    #[test]
    fn history_retention_cutoff_keeps_the_boundary_row() {
        let now = 1_700_000_000i64;
        let cutoff = history_retention_cutoff(now, 30);
        assert_eq!(cutoff, now - 30 * 86_400);

        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        db.set_watched(&media, 2, true).unwrap();

        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
            None,
            None,
        )
        .unwrap();
        for episode in 1..=2 {
            db.source_progress_set(
                &package.manifest.id,
                "show-1",
                &format!("ep-{episode}"),
                episode,
                None,
                10.0,
                600.0,
                false,
            )
            .unwrap();
        }

        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE history SET watched_at = ?1 WHERE episode = 1",
                params![cutoff],
            )
            .unwrap();
            conn.execute(
                "UPDATE history SET watched_at = ?1 WHERE episode = 2",
                params![cutoff - 1],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_history SET updated_at = ?1 WHERE episode_id = 'ep-1'",
                params![cutoff],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_history SET updated_at = ?1 WHERE episode_id = 'ep-2'",
                params![cutoff - 1],
            )
            .unwrap();
        }

        let result = db.purge_history_before(cutoff).unwrap();
        assert_eq!(result.anilist_history, 1);
        assert_eq!(result.source_history, 1);
        let history = db.history_list(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].episode, 1);
        let source_history = db.source_history(10).unwrap();
        assert_eq!(source_history.len(), 1);
        assert_eq!(source_history[0].episode_id, "ep-1");
    }

    #[test]
    fn history_retention_preserves_library_watched_progress_and_storage() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        db.set_watched(&media, 2, true).unwrap();

        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
            None,
            None,
        )
        .unwrap();
        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-1",
            1,
            Some("Episode 1"),
            120.0,
            600.0,
            false,
        )
        .unwrap();
        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-2",
            2,
            Some("Episode 2"),
            30.0,
            600.0,
            false,
        )
        .unwrap();
        db.set(&package.manifest.id, "session", "keep me").unwrap();

        let now = unix_now();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE history SET watched_at = ?1 WHERE episode = 1",
                params![now - 31 * 86_400],
            )
            .unwrap();
            conn.execute(
                "UPDATE history SET watched_at = ?1 WHERE episode = 2",
                params![now - 29 * 86_400],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_history SET updated_at = ?1 WHERE episode_id = 'ep-1'",
                params![now - 31 * 86_400],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_history SET updated_at = ?1 WHERE episode_id = 'ep-2'",
                params![now - 29 * 86_400],
            )
            .unwrap();
        }

        let result = db.purge_history_older_than(Some(30)).unwrap();
        assert_eq!(
            result,
            HistoryRetentionResult {
                anilist_history: 1,
                source_history: 1,
            }
        );

        let history = db.history_list(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].episode, 2);

        assert_eq!(db.watched_list(media.id).unwrap(), vec![1, 2]);
        let library = db.list_library().unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].watched_count, 2);
        assert_eq!(db.continue_watching(10).unwrap().len(), 1);

        let source_history = db.source_history(10).unwrap();
        assert_eq!(source_history.len(), 1);
        assert_eq!(source_history[0].episode_id, "ep-2");
        let progress = db
            .source_progress_get(&package.manifest.id, "show-1")
            .unwrap();
        assert_eq!(progress.len(), 2);
        assert_eq!(db.source_continue(10).unwrap().len(), 1);
        assert_eq!(
            db.get(&package.manifest.id, "session").as_deref(),
            Some("keep me")
        );
    }

    #[test]
    fn forever_retention_deletes_nothing() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute("UPDATE history SET watched_at = 1 WHERE episode = 1", [])
                .unwrap();
        }

        assert_eq!(
            db.purge_history_older_than(None).unwrap(),
            HistoryRetentionResult::default()
        );
        assert_eq!(db.history_list(10).unwrap().len(), 1);
    }

    #[test]
    fn single_history_entries_can_be_removed() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        db.set_watched(&media, 2, true).unwrap();

        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
            None,
            None,
        )
        .unwrap();
        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-1",
            1,
            Some("Episode 1"),
            120.0,
            600.0,
            false,
        )
        .unwrap();
        db.source_progress_set(
            &package.manifest.id,
            "show-1",
            "ep-2",
            2,
            Some("Episode 2"),
            30.0,
            600.0,
            false,
        )
        .unwrap();

        assert_eq!(db.history_list(10).unwrap().len(), 2);
        db.history_remove(media.id, 1).unwrap();
        let history = db.history_list(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].episode, 2);
        assert_eq!(db.watched_list(media.id).unwrap(), vec![1, 2]);
        assert_eq!(db.continue_watching(10).unwrap().len(), 1);

        let source_history = db.source_history(10).unwrap();
        assert_eq!(source_history.len(), 2);
        let target = source_history
            .iter()
            .find(|row| row.episode_id == "ep-1")
            .unwrap();
        db.source_history_remove(&target.extension_id, &target.show_id, &target.episode_id)
            .unwrap();
        let remaining = db.source_history(10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].episode_id, "ep-2");
        assert_eq!(
            db.source_progress_get(&package.manifest.id, "show-1")
                .unwrap()
                .len(),
            2
        );
        assert_eq!(db.source_continue(10).unwrap().len(), 1);
    }

    #[test]
    fn migration_adds_anilist_link_to_source_library() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/0001_init.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/0004_source_library.sql"))
            .unwrap();
        conn.execute(
            "INSERT INTO source_library(
                extension_id, show_id, title, added_at, updated_at
             ) VALUES ('ext.one', 'show-1', 'Popeye', 1, 1)",
            [],
        )
        .unwrap();

        apply_migrations(&conn).unwrap();
        assert!(has_column(&conn, "source_library", "anilist_id").unwrap());
        let anilist_id: Option<i64> = conn
            .query_row(
                "SELECT anilist_id FROM source_library WHERE show_id = 'show-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(anilist_id, None);

        // Startup replays migrations; the added column and row must survive.
        apply_migrations(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM source_library", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn continue_visible_backfill_is_marked_and_repairs_interrupted_upgrade() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/0001_init.sql"))
            .unwrap();
        conn.execute_batch(
            "CREATE TABLE source_progress(
                extension_id TEXT NOT NULL,
                show_id TEXT NOT NULL,
                episode_id TEXT NOT NULL,
                episode_number INTEGER NOT NULL,
                episode_title TEXT,
                position_sec REAL NOT NULL DEFAULT 0,
                duration_sec REAL NOT NULL DEFAULT 0,
                watched INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY(extension_id, show_id, episode_id)
            );
            INSERT INTO source_progress(
                extension_id, show_id, episode_id, episode_number,
                position_sec, duration_sec, watched, updated_at
            ) VALUES ('ext.one', 'show-1', 'ep-1', 1, 90.0, 600.0, 0, 1);",
        )
        .unwrap();

        apply_migrations(&conn).unwrap();
        assert!(has_column(&conn, "source_progress", "continue_visible").unwrap());
        let visible: i64 = conn
            .query_row(
                "SELECT continue_visible FROM source_progress WHERE episode_id = 'ep-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(visible, 1);
        let marker: String = conn
            .query_row(
                "SELECT value FROM app_meta
                 WHERE key = 'source_progress_continue_visible_backfilled'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(marker, "1");

        // A row the user hides after the migration stays hidden on restart.
        conn.execute(
            "UPDATE source_progress SET continue_visible = 0 WHERE episode_id = 'ep-1'",
            [],
        )
        .unwrap();
        apply_migrations(&conn).unwrap();
        let visible: i64 = conn
            .query_row(
                "SELECT continue_visible FROM source_progress WHERE episode_id = 'ep-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(visible, 0);

        // Simulate an upgrade interrupted after ALTER TABLE but before the
        // backfill: the column exists, the marker does not, and the row is 0.
        conn.execute(
            "DELETE FROM app_meta
             WHERE key = 'source_progress_continue_visible_backfilled'",
            [],
        )
        .unwrap();
        apply_migrations(&conn).unwrap();
        let visible: i64 = conn
            .query_row(
                "SELECT continue_visible FROM source_progress WHERE episode_id = 'ep-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(visible, 1);
    }

    #[test]
    fn source_history_backfill_runs_once() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, ExtensionTrust::Builtin)
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
            None,
            None,
        )
        .unwrap();

        // Simulate a database created before 0006: progress exists but the
        // activity list and its guard do not.
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "DELETE FROM app_meta WHERE key = 'source_history_backfilled'",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO source_progress(
                    extension_id, show_id, episode_id, episode_number, episode_title,
                    position_sec, duration_sec, watched, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, unixepoch())",
                params![
                    &package.manifest.id,
                    "show-1",
                    "ep-1",
                    1,
                    "Episode 1",
                    90.0,
                    600.0,
                    0,
                ],
            )
            .unwrap();
            apply_migrations(&conn).unwrap();
        }

        let history = db.source_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].episode_number, 1);
        assert!(!history[0].watched);

        db.history_clear().unwrap();
        assert!(db.source_history(10).unwrap().is_empty());

        {
            let conn = db.conn.lock().unwrap();
            apply_migrations(&conn).unwrap();
        }
        assert!(db.source_history(10).unwrap().is_empty());
    }

    #[test]
    fn watched_progress_round_trip() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();

        let watched = db.set_watched(&media, 3, true).unwrap();
        assert_eq!(watched, vec![3]);
        assert_eq!(db.list_library().unwrap()[0].watched_count, 1);
        // A manual mark is Continue Watching progress.
        let items = db.continue_watching(10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].episode, 3);

        let all = db.mark_all_watched(&media, 5, true).unwrap();
        assert_eq!(all, vec![1, 2, 3, 4, 5]);
        assert_eq!(db.continue_watching(10).unwrap()[0].episode, 5);

        let history = db.history_list(10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].episode, 5);
        assert_eq!(history[0].media.id, 15125);

        db.set_library_status(15125, "completed").unwrap();
        assert_eq!(db.list_library().unwrap()[0].status, "completed");
        assert_eq!(db.continue_watching(10).unwrap()[0].episode, 5);
        assert!(db.set_library_status(15125, "bogus").is_err());
        db.set_library_status(15125, "watching").unwrap();

        db.set_watched(&media, 5, false).unwrap();
        assert_eq!(db.continue_watching(10).unwrap()[0].episode, 4);

        let cleared = db.mark_all_watched(&media, 28, false).unwrap();
        assert!(cleared.is_empty());
        assert!(db.continue_watching(10).unwrap().is_empty());
    }

    #[test]
    fn continue_watching_includes_manual_marks() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        // A manual watched mark is Continue Watching progress.
        db.set_watched(&media, 3, true).unwrap();
        let items = db.continue_watching(10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].episode, 3);

        // Clearing the mark removes the row again.
        db.set_watched(&media, 3, false).unwrap();
        assert!(db.continue_watching(10).unwrap().is_empty());
    }

    #[test]
    fn library_add_and_remove_do_not_enqueue() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        assert!(db.pending_anilist_sync().unwrap().is_empty());
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 0);

        db.remove_from_library(media.id).unwrap();
        assert!(db.pending_anilist_sync().unwrap().is_empty());
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 0);
    }

    #[test]
    fn queue_replaces_the_previous_change_and_tracks_watched_count() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();

        db.set_library_status(media.id, "completed").unwrap();
        let rows = db.pending_anilist_sync().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].anilist_id, media.id);
        assert_eq!(rows[0].status, "completed");
        assert_eq!(rows[0].progress, 0);

        db.set_watched(&media, 1, true).unwrap();
        db.set_watched(&media, 2, true).unwrap();
        let rows = db.pending_anilist_sync().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "completed");
        assert_eq!(rows[0].progress, 2);

        db.mark_all_watched(&media, 5, true).unwrap();
        let rows = db.pending_anilist_sync().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].progress, 5);
    }

    #[test]
    fn queue_failure_updates_attempts_and_a_new_change_clears_them() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_library_status(media.id, "paused").unwrap();

        db.fail_anilist_sync(media.id, "offline").unwrap();
        db.fail_anilist_sync(media.id, "still offline").unwrap();
        let mut rows = db.pending_anilist_sync().unwrap();
        let row = rows.remove(0);
        assert_eq!(row.attempts, 2);
        assert_eq!(row.last_error.as_deref(), Some("still offline"));

        db.set_watched(&media, 1, true).unwrap();
        let mut rows = db.pending_anilist_sync().unwrap();
        let row = rows.remove(0);
        assert_eq!(row.attempts, 0);
        assert_eq!(row.last_error, None);
        assert_eq!(row.progress, 1);

        db.remove_anilist_sync(media.id).unwrap();
        assert_eq!(db.pending_anilist_sync_count().unwrap(), 0);
    }

    #[test]
    fn sync_anilist_entries_is_idempotent_and_keeps_local_rows() {
        let db = Db::new(":memory:").unwrap();

        let mut current = sample_media();
        current.id = 15125;
        current.episodes = Some(12);
        let mut completed = sample_media();
        completed.id = 21;
        completed.title.romaji = Some("One Piece".to_string());
        completed.title.english = None;
        completed.episodes = Some(12);

        let mut local = sample_media();
        local.id = 99;
        local.title.romaji = Some("Local only".to_string());
        local.title.english = None;
        db.add_to_library(&local, "paused").unwrap();

        let entries = vec![
            AniListListEntry {
                media: current.clone(),
                status: "CURRENT".to_string(),
                progress: 3,
            },
            AniListListEntry {
                media: completed.clone(),
                status: "COMPLETED".to_string(),
                progress: 15,
            },
        ];

        assert_eq!(db.sync_anilist_entries(&entries).unwrap(), 2);
        let items = db.list_library().unwrap();
        assert_eq!(items.len(), 3);
        let current_row = items
            .iter()
            .find(|item| item.media.id == current.id)
            .unwrap();
        assert_eq!(current_row.status, "watching");
        assert_eq!(current_row.watched_count, 3);
        let completed_row = items
            .iter()
            .find(|item| item.media.id == completed.id)
            .unwrap();
        assert_eq!(completed_row.status, "completed");
        assert_eq!(completed_row.watched_count, 12);
        assert!(db.history_list(10).unwrap().is_empty());

        // A local episode above server progress survives the next pull.
        db.set_watched(&current, 5, true).unwrap();
        assert_eq!(db.sync_anilist_entries(&entries).unwrap(), 2);
        let items = db.list_library().unwrap();
        assert_eq!(items.len(), 3);
        let current_row = items
            .iter()
            .find(|item| item.media.id == current.id)
            .unwrap();
        assert_eq!(current_row.status, "watching");
        assert_eq!(current_row.watched_count, 4);
        let completed_row = items
            .iter()
            .find(|item| item.media.id == completed.id)
            .unwrap();
        assert_eq!(completed_row.status, "completed");
        assert_eq!(completed_row.watched_count, 12);
        let local_row = items.iter().find(|item| item.media.id == 99).unwrap();
        assert_eq!(local_row.status, "paused");
    }

    fn test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let dir =
            std::env::temp_dir().join(format!("kaminari-{label}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn safety_copies(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".safety-"))
            })
            .collect()
    }

    #[test]
    fn safety_copy_cleanup_keeps_the_newest_three() {
        let dir = test_dir("safety-cleanup");
        let live_path = dir.join("kaminari.db");
        std::fs::write(&live_path, b"live").unwrap();
        for stamp in 1..=5 {
            std::fs::write(
                dir.join(format!("kaminari.db.safety-{stamp}.db")),
                format!("copy {stamp}"),
            )
            .unwrap();
        }
        let quarantine = dir.join("kaminari.db.quarantine-1");
        std::fs::create_dir(&quarantine).unwrap();
        std::fs::write(quarantine.join("kaminari.db"), b"quarantined").unwrap();
        let unrelated = dir.join("notes.db.safety-9.db");
        std::fs::write(&unrelated, b"other database").unwrap();

        assert_eq!(cleanup_safety_copies(&live_path), 2);

        assert!(live_path.exists());
        assert!(quarantine.join("kaminari.db").exists());
        assert!(unrelated.exists());
        for stamp in 1..=2 {
            assert!(!dir.join(format!("kaminari.db.safety-{stamp}.db")).exists());
        }
        for stamp in 3..=5 {
            assert!(dir.join(format!("kaminari.db.safety-{stamp}.db")).exists());
        }
        assert_eq!(cleaned_safety_copies(&dir), 3);

        // A second run has nothing left to remove.
        assert_eq!(cleanup_safety_copies(&live_path), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn cleaned_safety_copies(dir: &Path) -> usize {
        safety_copies(dir)
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("kaminari.db.safety-"))
            })
            .count()
    }

    #[test]
    fn restore_keeps_the_newest_three_safety_copies() {
        let dir = test_dir("restore-safety-cleanup");
        let live_path = dir.join("kaminari.db");
        let backup_path = dir.join("backup.db");
        let live_raw = live_path.to_string_lossy().into_owned();
        let backup_raw = backup_path.to_string_lossy().into_owned();

        for stamp in 1..=4 {
            std::fs::write(
                dir.join(format!("kaminari.db.safety-{stamp}.db")),
                format!("old copy {stamp}"),
            )
            .unwrap();
        }

        let db = Db::new(&live_raw).unwrap();
        db.add_to_library(&sample_media(), "watching").unwrap();
        db.backup_to(&backup_raw).unwrap();
        db.restore_from(&backup_raw).unwrap();

        assert_eq!(cleaned_safety_copies(&dir), 3);
        assert!(!dir.join("kaminari.db.safety-1.db").exists());
        assert!(!dir.join("kaminari.db.safety-2.db").exists());
        assert!(live_path.exists());
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_and_restore_round_trip() {
        let dir = test_dir("backup-round-trip");
        let live_path = dir.join("live.db");
        let backup_path = dir.join("backup.db");
        let other_path = dir.join("other.db");
        let live_raw = live_path.to_string_lossy().into_owned();
        let backup_raw = backup_path.to_string_lossy().into_owned();

        {
            let db = Db::new(&live_raw).unwrap();
            db.add_to_library(&sample_media(), "watching").unwrap();

            let bytes = db.backup_to(&backup_raw).unwrap();
            assert!(bytes > 0);
            assert!(backup_path.exists());

            let mut second = sample_media();
            second.id = 2;
            second.title.romaji = Some("Second show".to_string());
            db.add_to_library(&second, "planning").unwrap();
            assert_eq!(db.list_library().unwrap().len(), 2);

            db.restore_from(&backup_raw).unwrap();
            let items = db.list_library().unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].media.id, 15125);
            assert_eq!(safety_copies(&dir).len(), 1);

            assert!(db.backup_to("relative.db").is_err());
            assert!(db.restore_from("relative.db").is_err());
        }

        {
            let other = Connection::open(&other_path).unwrap();
            other
                .execute_batch("CREATE TABLE notes(id INTEGER PRIMARY KEY, body TEXT);")
                .unwrap();
        }

        let db = Db::new(&live_raw).unwrap();
        let error = db
            .restore_from(other_path.to_string_lossy().as_ref())
            .unwrap_err();
        assert!(error.to_string().contains("not a Kaminari database"));
        drop(db);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_rejects_corrupt_backup_and_keeps_live_data() {
        let dir = test_dir("backup-corrupt");
        let live_raw = dir.join("live.db").to_string_lossy().into_owned();
        let corrupt_path = dir.join("corrupt.db");
        std::fs::write(&corrupt_path, b"this is not a sqlite database").unwrap();

        let db = Db::new(&live_raw).unwrap();
        db.add_to_library(&sample_media(), "watching").unwrap();

        let error = db
            .restore_from(corrupt_path.to_string_lossy().as_ref())
            .unwrap_err();
        assert!(
            error.to_string().contains("integrity check"),
            "unexpected error: {error}"
        );

        let items = db.list_library().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].media.id, 15125);
        assert!(safety_copies(&dir).is_empty());
        drop(db);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_rejects_wrong_schema_and_keeps_live_data() {
        let dir = test_dir("backup-wrong-schema");
        let live_raw = dir.join("live.db").to_string_lossy().into_owned();
        let wrong_path = dir.join("wrong.db");

        {
            let wrong = Connection::open(&wrong_path).unwrap();
            wrong
                .execute_batch(
                    "CREATE TABLE media(anilist_id INTEGER PRIMARY KEY, title_romaji TEXT NOT NULL);
                     CREATE TABLE library(anilist_id INTEGER PRIMARY KEY, status TEXT NOT NULL);
                     CREATE TABLE watched(anilist_id INTEGER NOT NULL, episode INTEGER NOT NULL);
                     CREATE TABLE history(id INTEGER PRIMARY KEY, anilist_id INTEGER NOT NULL);
                     CREATE TABLE source_library(extension_id TEXT NOT NULL, show_id TEXT NOT NULL);",
                )
                .unwrap();
        }

        let db = Db::new(&live_raw).unwrap();
        db.add_to_library(&sample_media(), "watching").unwrap();

        let error = db
            .restore_from(wrong_path.to_string_lossy().as_ref())
            .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("not a Kaminari database"),
            "unexpected error: {error}"
        );
        assert!(
            message.contains("missing column"),
            "unexpected error: {error}"
        );

        let items = db.list_library().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].media.id, 15125);
        assert!(safety_copies(&dir).is_empty());
        drop(db);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_rejects_newer_schema_version() {
        let dir = test_dir("backup-newer-schema");
        let live_raw = dir.join("live.db").to_string_lossy().into_owned();
        let future_path = dir.join("future.db");

        {
            let future = Connection::open(&future_path).unwrap();
            apply_migrations(&future).unwrap();
            future
                .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();
        }

        let db = Db::new(&live_raw).unwrap();
        db.add_to_library(&sample_media(), "watching").unwrap();

        let error = db
            .restore_from(future_path.to_string_lossy().as_ref())
            .unwrap_err();
        assert!(
            error.to_string().contains("newer Kaminari version"),
            "unexpected error: {error}"
        );
        assert_eq!(db.list_library().unwrap().len(), 1);
        assert!(safety_copies(&dir).is_empty());
        drop(db);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_rejects_oversized_backup() {
        let dir = test_dir("backup-size");
        let path = dir.join("oversized.db");
        std::fs::write(&path, b"0123456789").unwrap();

        let error = validate_backup_file(&path, 4).unwrap_err();
        assert!(
            error.to_string().contains("too large"),
            "unexpected error: {error}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_preserves_data_and_disables_extensions() {
        let dir = test_dir("backup-preserve");
        let live_raw = dir.join("live.db").to_string_lossy().into_owned();
        let source_path = dir.join("source.db");
        let source_raw = source_path.to_string_lossy().into_owned();
        let backup_path = dir.join("backup.db");
        let backup_raw = backup_path.to_string_lossy().into_owned();
        let package = crate::ext_host::sample_package().unwrap();

        {
            let source = Db::new(&source_raw).unwrap();
            source
                .install_js_extension(&package.manifest, &package.code, ExtensionTrust::Sideloaded)
                .unwrap();
            // Simulate a backup made by an older build that trusted a label
            // supplied by the package.
            {
                let conn = source.conn.lock().unwrap();
                conn.execute("UPDATE js_extensions SET trust = 'signed'", [])
                    .unwrap();
            }
            source.add_to_library(&sample_media(), "watching").unwrap();
            source.mark_all_watched(&sample_media(), 2, true).unwrap();
            source
                .source_library_add(
                    &package.manifest.id,
                    "show-1",
                    "Popeye",
                    None,
                    Some(1933),
                    Some("finished"),
                    Some(15125),
                )
                .unwrap();
            source
                .source_progress_set(
                    &package.manifest.id,
                    "show-1",
                    "ep-1",
                    1,
                    Some("Episode 1"),
                    42.5,
                    600.0,
                    false,
                )
                .unwrap();
            source.backup_to(&backup_raw).unwrap();
        }

        let db = Db::new(&live_raw).unwrap();
        let mut live_only = sample_media();
        live_only.id = 2;
        live_only.title.romaji = Some("Live only".to_string());
        db.add_to_library(&live_only, "planning").unwrap();
        db.mark_all_watched(&live_only, 1, true).unwrap();

        db.restore_from(&backup_raw).unwrap();

        let items = db.list_library().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].media.id, 15125);
        assert_eq!(db.watched_list(15125).unwrap(), vec![1, 2]);
        let history = db.history_list(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].episode, 2);

        let source_library = db.source_library_list().unwrap();
        assert_eq!(source_library.len(), 1);
        assert_eq!(source_library[0].show_id, "show-1");
        assert_eq!(source_library[0].anilist_id, Some(15125));
        assert_eq!(
            db.source_progress_get(&package.manifest.id, "show-1")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(db.source_history(10).unwrap().len(), 1);

        let extensions = db.list_js_extensions().unwrap();
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].id, package.manifest.id);
        assert_eq!(extensions[0].trust, "sideloaded");
        assert!(!extensions[0].enabled);

        let copies = safety_copies(&dir);
        assert_eq!(copies.len(), 1);
        let safety =
            Connection::open_with_flags(&copies[0], OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let live_only_rows: i64 = safety
            .query_row(
                "SELECT COUNT(*) FROM library WHERE anilist_id = 2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(live_only_rows, 1);
        let live_extensions: i64 = safety
            .query_row("SELECT COUNT(*) FROM js_extensions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(live_extensions, 0);
        drop(safety);
        drop(db);

        let leftovers = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.contains(".restore-"))
            })
            .count();
        assert_eq!(leftovers, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

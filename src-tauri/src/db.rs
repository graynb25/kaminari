// SQLite helpers. One file, WAL, migrations checked in.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::anilist::{AniListListEntry, CoverImage, Media, MediaTitle};
use crate::error::{AppError, AppResult};
use crate::ext_host::{ExtStorage, Manifest};
use crate::registry::LegacyExtension;

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
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
    pub episode_id: String,
    pub episode_number: i64,
    pub episode_title: Option<String>,
    pub position_sec: f64,
    pub duration_sec: f64,
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
        let conn = Connection::open(path)?;
        apply_pragmas(&conn)?;
        apply_migrations(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
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
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute("DELETE FROM library WHERE anilist_id = ?1", params![id])?;
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

    pub fn pending_anilist_sync(&self) -> AppResult<Vec<AniListSyncRow>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
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

    pub fn pending_anilist_sync_count(&self) -> AppResult<usize> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        pending_anilist_sync_count(&conn)
    }

    pub fn remove_anilist_sync(&self, anilist_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "DELETE FROM anilist_sync_queue WHERE anilist_id = ?1",
            params![anilist_id],
        )?;
        Ok(())
    }

    pub fn fail_anilist_sync(&self, anilist_id: i64, error: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "UPDATE anilist_sync_queue SET attempts = attempts + 1, last_error = ?2
             WHERE anilist_id = ?1",
            params![anilist_id, error],
        )?;
        Ok(())
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
            let mut progress = entry.progress.max(0);
            if let Some(episodes) = entry.media.episodes.filter(|episodes| *episodes > 0) {
                progress = progress.min(i64::from(episodes));
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
        tx.execute("UPDATE source_progress SET position_sec = 0", [])?;
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

    pub fn watched_list(&self, anilist_id: i64) -> AppResult<Vec<i64>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        read_watched(&conn, anilist_id)
    }

    pub fn set_watched(&self, media: &Media, episode: i64, watched: bool) -> AppResult<Vec<i64>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        upsert_media(&conn, media)?;
        if watched {
            ensure_library_entry(&conn, media.id)?;
        }
        apply_watched(&conn, media.id, episode, watched)?;
        enqueue_anilist_sync(&conn, media.id, &current_library_status(&conn, media.id)?)?;
        read_watched(&conn, media.id)
    }

    pub fn mark_all_watched(
        &self,
        media: &Media,
        through: i64,
        watched: bool,
    ) -> AppResult<Vec<i64>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        upsert_media(&conn, media)?;
        if watched {
            ensure_library_entry(&conn, media.id)?;
            for episode in 1..=through.max(0) {
                conn.execute(
                    "INSERT OR IGNORE INTO watched(anilist_id, episode, watched_at)
                     VALUES (?1, ?2, unixepoch())",
                    params![media.id, episode],
                )?;
            }
            conn.execute(
                "INSERT INTO history(anilist_id, episode, watched_at)
                 VALUES (?1, ?2, unixepoch())",
                params![media.id, through.max(0)],
            )?;
            conn.execute(
                "INSERT INTO progress(anilist_id, episode, position_sec, duration_sec, updated_at)
                 VALUES (?1, ?2, 0, 0, unixepoch())
                 ON CONFLICT(anilist_id) DO UPDATE SET
                    episode = excluded.episode,
                    updated_at = unixepoch()",
                params![media.id, through.max(0)],
            )?;
        } else {
            conn.execute(
                "DELETE FROM watched WHERE anilist_id = ?1",
                params![media.id],
            )?;
            conn.execute(
                "DELETE FROM history WHERE anilist_id = ?1",
                params![media.id],
            )?;
            conn.execute(
                "DELETE FROM progress WHERE anilist_id = ?1",
                params![media.id],
            )?;
        }
        enqueue_anilist_sync(&conn, media.id, &current_library_status(&conn, media.id)?)?;
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
             WHERE l.status = 'watching'
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
                trust: row.get(5)?,
                installed_at: row.get(6)?,
                hosts: manifest
                    .map(|manifest| manifest.permissions.hosts)
                    .unwrap_or_default(),
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

    pub fn install_js_extension(
        &self,
        manifest: &Manifest,
        code: &str,
        trust: &str,
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
                trust,
            ],
        )?;
        Ok(())
    }

    pub fn remove_js_extension(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute("DELETE FROM js_extensions WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn source_library_add(
        &self,
        extension_id: &str,
        show_id: &str,
        title: &str,
        cover: Option<&str>,
        year: Option<i32>,
        status: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "INSERT INTO source_library(
                extension_id, show_id, title, cover, year, status, added_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, unixepoch(), unixepoch())
             ON CONFLICT(extension_id, show_id) DO UPDATE SET
                title = excluded.title,
                cover = excluded.cover,
                year = excluded.year,
                status = excluded.status,
                updated_at = unixepoch()",
            params![extension_id, show_id, title, cover, year, status],
        )?;
        Ok(())
    }

    pub fn source_library_list(&self) -> AppResult<Vec<SourceLibraryItem>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT sl.extension_id,
                    COALESCE(je.name, sl.extension_id),
                    sl.show_id, sl.title, sl.cover, sl.year, sl.status, sl.added_at,
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
                added_at: row.get(7)?,
                watched_count: row.get(8)?,
                last_episode_id: row.get(9)?,
                last_episode: row.get(10)?,
                last_position: row.get(11)?,
                last_duration: row.get(12)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn source_library_remove(&self, extension_id: &str, show_id: &str) -> AppResult<()> {
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
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "INSERT INTO source_progress(
                extension_id, show_id, episode_id, episode_number, episode_title,
                position_sec, duration_sec, watched, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, unixepoch())
             ON CONFLICT(extension_id, show_id, episode_id) DO UPDATE SET
                episode_number = excluded.episode_number,
                episode_title = excluded.episode_title,
                position_sec = excluded.position_sec,
                duration_sec = excluded.duration_sec,
                watched = excluded.watched,
                updated_at = unixepoch()",
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
        if episodes.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        let tx = conn.transaction()?;
        if watched {
            let mut statement = tx.prepare(
                "INSERT INTO source_progress(
                    extension_id, show_id, episode_id, episode_number, episode_title,
                    position_sec, duration_sec, watched, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, unixepoch())
                 ON CONFLICT(extension_id, show_id, episode_id) DO UPDATE SET
                    episode_number = excluded.episode_number,
                    episode_title = excluded.episode_title,
                    position_sec = excluded.position_sec,
                    duration_sec = excluded.duration_sec,
                    watched = 1,
                    updated_at = unixepoch()",
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
                let duration = episode.duration_sec.max(0.0);
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
                    sp.show_id, sl.title, sl.cover,
                    sp.episode_id, sp.episode_number, sp.episode_title,
                    sp.position_sec, sp.duration_sec, sp.updated_at
             FROM (
               SELECT latest.*, ROW_NUMBER() OVER (
                 PARTITION BY latest.extension_id, latest.show_id
                 ORDER BY latest.updated_at DESC, latest.episode_number DESC
               ) AS rank
               FROM source_progress latest
               WHERE latest.watched = 0 AND latest.position_sec > 0
             ) sp
             JOIN source_library sl
               ON sl.extension_id = sp.extension_id AND sl.show_id = sp.show_id
             LEFT JOIN js_extensions je ON je.id = sp.extension_id
             WHERE sp.rank = 1
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
                episode_id: row.get(5)?,
                episode_number: row.get(6)?,
                episode_title: row.get(7)?,
                position_sec: row.get(8)?,
                duration_sec: row.get(9)?,
                updated_at: row.get(10)?,
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
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute(
            "DELETE FROM source_history
             WHERE extension_id = ?1 AND show_id = ?2 AND episode_id = ?3",
            params![extension_id, show_id, episode_id],
        )?;
        Ok(())
    }

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

    pub fn remove_source(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        conn.execute("DELETE FROM sources WHERE id = ?1", params![id])?;
        Ok(())
    }

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
        let parent = target
            .parent()
            .ok_or_else(|| AppError::Blocked("backup path has no parent folder".to_string()))?;
        let name = target
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .ok_or_else(|| AppError::Blocked("backup path has no file name".to_string()))?;
        let temp = parent.join(format!(".{name}.kaminari.tmp"));
        let temp_raw = temp.to_string_lossy().into_owned();

        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let outcome = (|| -> AppResult<()> {
            if temp.exists() {
                std::fs::remove_file(&temp)
                    .map_err(|err| AppError::Db(format!("cannot clear temp backup: {err}")))?;
            }
            conn.execute("VACUUM INTO ?1", params![temp_raw])?;
            replace_file(&temp, &target)
                .map_err(|err| AppError::Db(format!("cannot replace backup: {err}")))?;
            Ok(())
        })();

        if outcome.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        outcome?;

        let bytes = std::fs::metadata(&target)
            .map_err(|err| AppError::Db(format!("cannot read backup size: {err}")))?
            .len();
        Ok(bytes)
    }

    pub fn restore_from(&self, path: &str) -> AppResult<()> {
        let source_path = absolute_path(path)?;
        let source = Connection::open_with_flags(&source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|err| AppError::Blocked(format!("cannot open backup: {err}")))?;

        const REQUIRED: [&str; 5] = ["media", "library", "watched", "history", "source_library"];
        let mut missing = Vec::new();
        for table in REQUIRED {
            let found: bool = source
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                    params![table],
                    |row| row.get(0),
                )
                .map_err(|err| AppError::Blocked(format!("not a Kaminari database: {err}")))?;
            if !found {
                missing.push(table);
            }
        }
        if !missing.is_empty() {
            return Err(AppError::Blocked(format!(
                "not a Kaminari database: missing {}",
                missing.join(", ")
            )));
        }

        let mut conn = self.conn.lock().map_err(|_| db_locked())?;
        {
            let backup = Backup::new(&source, &mut conn)?;
            backup.run_to_completion(100, Duration::from_millis(10), None)?;
        }
        apply_pragmas(&conn)?;
        apply_migrations(&conn)?;
        Ok(())
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
    Ok(())
}

fn replace_file(temp: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
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

fn current_library_status(conn: &Connection, anilist_id: i64) -> AppResult<String> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM library WHERE anilist_id = ?1",
            params![anilist_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(status.unwrap_or_else(|| "watching".to_string()))
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

impl ExtStorage for Db {
    fn get(&self, extension_id: &str, key: &str) -> Option<String> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT value FROM js_storage WHERE extension_id = ?1 AND key = ?2",
            params![extension_id, key],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten()
    }

    fn set(&self, extension_id: &str, key: &str, value: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let used: i64 = conn.query_row(
            "SELECT COALESCE(SUM(length(CAST(key AS BLOB)) + length(CAST(value AS BLOB))), 0)
             FROM js_storage WHERE extension_id = ?1",
            params![extension_id],
            |row| row.get(0),
        )?;
        if used as usize + key.len() + value.len() > 1024 * 1024 {
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
            params![extension_id, key, value],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext_host::ExtStorage;
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
        db.install_js_extension(&package.manifest, &package.code, "builtin")
            .unwrap();

        let list = db.list_js_extensions().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "app.kaminari.publicdomain");
        assert_eq!(list[0].hosts, vec!["https://archive.org/*"]);
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

    #[test]
    fn source_library_and_progress() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, "builtin")
            .unwrap();

        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            Some("https://example.com/cover.jpg"),
            Some(1933),
            Some("finished"),
        )
        .unwrap();
        let items = db.source_library_list().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].extension_name, "Public Domain Animation");
        assert_eq!(items[0].watched_count, 0);

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
    fn source_progress_mark_all_updates_library_and_continue() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, "builtin")
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
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
        assert!(db.source_continue(10).unwrap().is_empty());
        assert_eq!(db.source_history(10).unwrap().len(), 3);

        db.source_progress_mark_all(&package.manifest.id, "show-1", &marks, false)
            .unwrap();
        assert!(db
            .source_progress_get(&package.manifest.id, "show-1")
            .unwrap()
            .is_empty());
        assert_eq!(db.source_library_list().unwrap()[0].watched_count, 0);
        assert!(db.source_history(10).unwrap().is_empty());
    }

    #[test]
    fn history_clear_removes_activity_and_keeps_progress() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        db.set_watched(&media, 2, true).unwrap();

        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, "builtin")
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
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
    fn single_history_entries_can_be_removed() {
        let db = Db::new(":memory:").unwrap();
        let media = sample_media();
        db.add_to_library(&media, "watching").unwrap();
        db.set_watched(&media, 1, true).unwrap();
        db.set_watched(&media, 2, true).unwrap();

        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, "builtin")
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
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
    fn source_history_backfill_runs_once() {
        let db = Db::new(":memory:").unwrap();
        let package = crate::ext_host::sample_package().unwrap();
        db.install_js_extension(&package.manifest, &package.code, "builtin")
            .unwrap();
        db.source_library_add(
            &package.manifest.id,
            "show-1",
            "Popeye",
            None,
            Some(1933),
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

        let continue_items = db.continue_watching(10).unwrap();
        assert_eq!(continue_items.len(), 1);
        assert_eq!(continue_items[0].episode, 3);
        assert_eq!(continue_items[0].watched_count, 1);
        assert_eq!(continue_items[0].total_episodes, 28);

        let all = db.mark_all_watched(&media, 5, true).unwrap();
        assert_eq!(all, vec![1, 2, 3, 4, 5]);
        assert_eq!(db.continue_watching(10).unwrap()[0].episode, 5);

        let history = db.history_list(10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].episode, 5);
        assert_eq!(history[0].media.id, 15125);

        db.set_library_status(15125, "completed").unwrap();
        assert_eq!(db.list_library().unwrap()[0].status, "completed");
        assert!(db.set_library_status(15125, "bogus").is_err());
        db.set_library_status(15125, "watching").unwrap();

        db.set_watched(&media, 5, false).unwrap();
        assert_eq!(db.continue_watching(10).unwrap()[0].episode, 4);

        let cleared = db.mark_all_watched(&media, 28, false).unwrap();
        assert!(cleared.is_empty());
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

    #[test]
    fn backup_and_restore_round_trip() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "kaminari-backup-test-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
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
}

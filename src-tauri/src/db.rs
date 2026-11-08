// SQLite helpers. One file, WAL, migrations checked in.

use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::anilist::{CoverImage, Media, MediaTitle};
use crate::error::{AppError, AppResult};
use crate::registry::LegacyExtension;

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub media: Media,
    pub status: String,
    pub added_at: i64,
    pub watched_count: i64,
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

impl Db {
    pub fn new(path: &str) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.execute_batch(include_str!("../migrations/0001_init.sql"))?;
        conn.execute_batch(include_str!("../migrations/0002_watched.sql"))?;
        Ok(Self {
            conn: Mutex::new(conn),
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

#[cfg(test)]
mod tests {
    use super::*;
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

        db.set_watched(&media, 5, false).unwrap();
        assert_eq!(db.continue_watching(10).unwrap()[0].episode, 4);

        let cleared = db.mark_all_watched(&media, 28, false).unwrap();
        assert!(cleared.is_empty());
        assert!(db.continue_watching(10).unwrap().is_empty());
    }
}

// SQLite helpers. One file, WAL, migrations checked in.

use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::anilist::{CoverImage, Media, MediaTitle};
use crate::error::{AppError, AppResult};

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub media: Media,
    pub status: String,
    pub added_at: i64,
}

impl Db {
    pub fn new(path: &str) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.execute_batch(include_str!("../migrations/0001_init.sql"))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn list_library(&self) -> AppResult<Vec<LibraryItem>> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
        let mut statement = conn.prepare(
            "SELECT m.anilist_id, m.title_romaji, m.title_english, m.title_native,
                    m.cover, m.banner, m.year, m.format, m.status, m.episodes,
                    m.score, m.genres, m.synopsis, l.status, l.added_at
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
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn add_to_library(&self, media: &Media, status: &str) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|_| db_locked())?;
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
}

fn db_locked() -> AppError {
    AppError::Db("database lock poisoned".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        db.remove_from_library(15125).unwrap();
        assert!(db.list_library().unwrap().is_empty());
    }
}

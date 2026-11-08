// SQLite WAL helpers. Single file, tests in place.

use rusqlite::{Connection, Result as SqlResult};

pub fn open(path: &str) -> SqlResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(conn)
}

pub fn apply_migrations(conn: &mut Connection) -> SqlResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS media(
            anilist_id INTEGER PRIMARY KEY,
            title_romaji TEXT NOT NULL,
            title_english TEXT,
            cover TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS http_cache(
            key TEXT PRIMARY KEY, body BLOB NOT NULL,
            fetched_at INTEGER NOT NULL, ttl_sec INTEGER NOT NULL
        );",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_and_migrate() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_migrations(&mut conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM sqlite_master WHERE type='table'", [], |r| r.get(0))
            .unwrap();
        assert!(count >= 1);
    }
}

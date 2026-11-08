PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
PRAGMA busy_timeout=5000;

CREATE TABLE IF NOT EXISTS media(
  anilist_id INTEGER PRIMARY KEY,
  title_romaji TEXT NOT NULL,
  title_english TEXT,
  title_native TEXT,
  cover TEXT NOT NULL,
  banner TEXT,
  blurhash TEXT,
  year INTEGER,
  format TEXT,
  status TEXT,
  episodes INTEGER,
  score REAL,
  genres JSON,
  synopsis TEXT,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS episodes(
  anilist_id INTEGER NOT NULL REFERENCES media(anilist_id) ON DELETE CASCADE,
  number INTEGER NOT NULL,
  title TEXT,
  thumb TEXT,
  duration_sec INTEGER,
  PRIMARY KEY(anilist_id, number)
);

CREATE TABLE IF NOT EXISTS library(
  anilist_id INTEGER PRIMARY KEY REFERENCES media(anilist_id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK(status IN ('watching','planning','completed','paused','dropped')),
  added_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS progress(
  anilist_id INTEGER PRIMARY KEY REFERENCES media(anilist_id) ON DELETE CASCADE,
  episode INTEGER NOT NULL DEFAULT 1,
  position_sec INTEGER NOT NULL DEFAULT 0,
  duration_sec INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS history(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  anilist_id INTEGER NOT NULL,
  episode INTEGER NOT NULL,
  watched_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_history_watched ON history(watched_at DESC);

CREATE TABLE IF NOT EXISTS sources(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  manifest JSON NOT NULL,
  installed_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS http_cache(
  key TEXT PRIMARY KEY,
  body BLOB NOT NULL,
  fetched_at INTEGER NOT NULL,
  ttl_sec INTEGER NOT NULL
);

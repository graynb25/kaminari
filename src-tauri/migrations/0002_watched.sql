CREATE TABLE IF NOT EXISTS watched(
  anilist_id INTEGER NOT NULL REFERENCES media(anilist_id) ON DELETE CASCADE,
  episode INTEGER NOT NULL,
  watched_at INTEGER NOT NULL,
  PRIMARY KEY(anilist_id, episode)
);
CREATE INDEX IF NOT EXISTS idx_watched_series ON watched(anilist_id, episode);

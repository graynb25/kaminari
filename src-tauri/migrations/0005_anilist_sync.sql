CREATE TABLE IF NOT EXISTS anilist_sync_queue(
  anilist_id INTEGER PRIMARY KEY,
  status TEXT NOT NULL,
  progress INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT
);

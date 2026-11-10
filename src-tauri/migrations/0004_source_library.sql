CREATE TABLE IF NOT EXISTS source_library(
  extension_id TEXT NOT NULL,
  show_id TEXT NOT NULL,
  title TEXT NOT NULL,
  cover TEXT,
  year INTEGER,
  status TEXT,
  added_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(extension_id, show_id)
);

CREATE TABLE IF NOT EXISTS source_progress(
  extension_id TEXT NOT NULL,
  show_id TEXT NOT NULL,
  episode_id TEXT NOT NULL,
  episode_number INTEGER NOT NULL,
  episode_title TEXT,
  position_sec REAL NOT NULL DEFAULT 0,
  duration_sec REAL NOT NULL DEFAULT 0,
  watched INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL,
  continue_visible INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(extension_id, show_id, episode_id)
);

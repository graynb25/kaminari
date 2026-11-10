CREATE TABLE IF NOT EXISTS app_meta(
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS source_history(
  extension_id TEXT NOT NULL,
  show_id TEXT NOT NULL,
  episode_id TEXT NOT NULL,
  episode_number INTEGER NOT NULL,
  episode_title TEXT,
  watched INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(extension_id, show_id, episode_id)
);

-- Seed the activity list once. The app_meta guard keeps later startups from
-- restoring rows that the user cleared.
INSERT OR IGNORE INTO source_history(
  extension_id, show_id, episode_id, episode_number, episode_title, watched, updated_at
)
SELECT sp.extension_id, sp.show_id, sp.episode_id, sp.episode_number,
       sp.episode_title, sp.watched, sp.updated_at
FROM source_progress sp
WHERE NOT EXISTS (
  SELECT 1 FROM app_meta WHERE key = 'source_history_backfilled'
);

INSERT OR IGNORE INTO app_meta(key, value) VALUES ('source_history_backfilled', '1');

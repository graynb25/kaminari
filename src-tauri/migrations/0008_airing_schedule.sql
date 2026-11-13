CREATE TABLE IF NOT EXISTS airing_schedule(
  anilist_id INTEGER NOT NULL,
  episode INTEGER NOT NULL,
  airing_at INTEGER NOT NULL,
  PRIMARY KEY (anilist_id, airing_at)
);

CREATE INDEX IF NOT EXISTS airing_schedule_airing_at
  ON airing_schedule(airing_at);

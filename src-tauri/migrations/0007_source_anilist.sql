-- Durable AniList identity for a source library row. The column itself is
-- added by apply_migrations when missing: SQLite has no ADD COLUMN IF NOT
-- EXISTS, and this migration file is replayed on every startup.
CREATE INDEX IF NOT EXISTS source_library_anilist
  ON source_library(anilist_id);

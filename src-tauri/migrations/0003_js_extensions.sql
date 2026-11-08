CREATE TABLE IF NOT EXISTS js_extensions(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  api_version TEXT NOT NULL,
  manifest JSON NOT NULL,
  code TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  trust TEXT NOT NULL DEFAULT 'sideloaded',
  installed_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS js_storage(
  extension_id TEXT NOT NULL REFERENCES js_extensions(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(extension_id, key)
);

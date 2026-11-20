CREATE TABLE scene_snapshot(
  id TEXT PRIMARY KEY,
  scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
  label TEXT NOT NULL,
  relative_path TEXT NOT NULL UNIQUE,
  sha256 TEXT NOT NULL,
  byte_length INTEGER NOT NULL CHECK(byte_length>=0),
  created_at TEXT NOT NULL
);
CREATE INDEX scene_snapshot_history ON scene_snapshot(scene_id,created_at DESC,id);

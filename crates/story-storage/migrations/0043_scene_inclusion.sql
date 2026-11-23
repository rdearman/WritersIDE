CREATE TABLE scene_exclusion (
    scene_id TEXT PRIMARY KEY NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    excluded_at TEXT NOT NULL
);

CREATE INDEX scene_exclusion_order ON scene_exclusion(excluded_at, scene_id);

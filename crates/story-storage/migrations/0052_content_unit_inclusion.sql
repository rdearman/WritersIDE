CREATE TABLE content_unit_exclusion (
    content_unit_id TEXT PRIMARY KEY NOT NULL REFERENCES content_unit(id) ON DELETE CASCADE,
    excluded_at TEXT NOT NULL
);

CREATE INDEX content_unit_exclusion_order
    ON content_unit_exclusion(excluded_at, content_unit_id);

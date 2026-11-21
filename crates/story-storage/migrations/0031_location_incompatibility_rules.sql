CREATE TABLE location_incompatibility_rule(
  id TEXT PRIMARY KEY,
  first_location_id TEXT NOT NULL REFERENCES location(id) ON DELETE CASCADE,
  second_location_id TEXT NOT NULL REFERENCES location(id) ON DELETE CASCADE,
  description TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  created_at TEXT NOT NULL,
  CHECK(first_location_id < second_location_id),
  UNIQUE(first_location_id, second_location_id)
);

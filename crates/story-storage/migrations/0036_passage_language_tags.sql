CREATE TABLE passage_language_tag(
  anchor_id TEXT PRIMARY KEY REFERENCES passage_anchor(id) ON DELETE CASCADE,
  language_code TEXT NOT NULL,
  created_at TEXT NOT NULL
);

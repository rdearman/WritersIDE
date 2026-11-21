CREATE TABLE saved_query_expression (
  query_id TEXT PRIMARY KEY NOT NULL REFERENCES saved_query(id) ON DELETE CASCADE,
  expression_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE saved_query_filter_next(
  query_id TEXT NOT NULL REFERENCES saved_query(id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL CHECK(ordinal>=0),
  filter_kind TEXT NOT NULL CHECK(filter_kind IN(
    'status','exclude_status','character','character_any','plotline','plotline_any',
    'time_before','time_after','unresolved_issue'
  )),
  value TEXT NOT NULL,
  PRIMARY KEY(query_id,ordinal)
);
INSERT INTO saved_query_filter_next(query_id,ordinal,filter_kind,value)
SELECT query_id,ordinal,filter_kind,value FROM saved_query_filter;
DROP TABLE saved_query_filter;
ALTER TABLE saved_query_filter_next RENAME TO saved_query_filter;
CREATE INDEX saved_query_filter_lookup ON saved_query_filter(query_id,filter_kind,value);

ALTER TABLE saved_query_filter RENAME TO saved_query_filter_v29;
CREATE TABLE saved_query_filter(
  query_id TEXT NOT NULL REFERENCES saved_query(id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL CHECK(ordinal>=0),
  filter_kind TEXT NOT NULL CHECK(filter_kind IN(
    'status','exclude_status','character','character_any','character_present','character_mentioned','character_absent',
    'plotline','plotline_any','location','event','chapter','text_contains','custom_metadata_equals','custom_metadata_contains_entity','assembly','issue_status',
    'time_before','time_after','time_before_definitely','time_before_possibly','time_after_definitely','time_after_possibly','unresolved_issue'
  )),
  value TEXT NOT NULL,
  PRIMARY KEY(query_id,ordinal)
);
INSERT INTO saved_query_filter(query_id,ordinal,filter_kind,value)
  SELECT query_id,ordinal,filter_kind,value FROM saved_query_filter_v29;
DROP TABLE saved_query_filter_v29;
CREATE INDEX saved_query_filter_lookup ON saved_query_filter(query_id,filter_kind,value);

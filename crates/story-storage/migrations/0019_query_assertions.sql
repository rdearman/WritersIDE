CREATE TABLE saved_query_assertion (
    query_id TEXT NOT NULL REFERENCES saved_query(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    assertion_kind TEXT NOT NULL CHECK(assertion_kind IN ('at_least','at_most','exactly','non_empty','empty')),
    threshold INTEGER CHECK(threshold IS NULL OR threshold >= 0),
    PRIMARY KEY(query_id, ordinal),
    CHECK((assertion_kind IN ('non_empty','empty') AND threshold IS NULL) OR
          (assertion_kind IN ('at_least','at_most','exactly') AND threshold IS NOT NULL))
);

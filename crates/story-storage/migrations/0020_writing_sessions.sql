CREATE TABLE writing_session (
    id TEXT PRIMARY KEY,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    scene_id TEXT REFERENCES scene(id) ON DELETE SET NULL,
    start_words INTEGER NOT NULL CHECK(start_words >= 0),
    end_words INTEGER CHECK(end_words >= 0),
    words_delta INTEGER,
    status TEXT NOT NULL CHECK(status IN ('active','completed','abandoned')),
    CHECK((status='active' AND ended_at IS NULL AND end_words IS NULL AND words_delta IS NULL)
       OR (status IN ('completed','abandoned') AND ended_at IS NOT NULL AND end_words IS NOT NULL AND words_delta IS NOT NULL))
);
CREATE UNIQUE INDEX writing_session_one_active ON writing_session(status) WHERE status='active';
CREATE INDEX writing_session_started ON writing_session(started_at DESC);
CREATE INDEX writing_session_scene ON writing_session(scene_id, started_at DESC);

CREATE TABLE writing_challenge (
    id TEXT PRIMARY KEY,
    started_at INTEGER NOT NULL,
    deadline_at INTEGER NOT NULL CHECK(deadline_at > started_at),
    target_words INTEGER NOT NULL CHECK(target_words > 0),
    mode TEXT NOT NULL CHECK(mode IN ('steady','kamikaze')),
    ended_at INTEGER,
    words_written INTEGER CHECK(words_written IS NULL OR words_written >= 0),
    status TEXT NOT NULL CHECK(status IN ('active','completed','abandoned'))
);
CREATE INDEX writing_challenge_started ON writing_challenge(started_at DESC);

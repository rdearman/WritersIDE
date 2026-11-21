CREATE TABLE writing_goal (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    scope_type TEXT NOT NULL CHECK(scope_type IN ('project','scene')),
    scope_scene_id TEXT REFERENCES scene(id) ON DELETE SET NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    target_words INTEGER NOT NULL CHECK(target_words > 0),
    baseline_words INTEGER NOT NULL DEFAULT 0 CHECK(baseline_words >= 0),
    time_zone TEXT NOT NULL DEFAULT 'UTC',
    word_count_policy TEXT NOT NULL DEFAULT 'unicode-word-boundaries',
    success_metric TEXT NOT NULL CHECK(success_metric IN ('raw','real')),
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
    created_at TEXT NOT NULL,
    CHECK(end_date >= start_date),
    CHECK((scope_type='project' AND scope_scene_id IS NULL)
       OR (scope_type='scene' AND scope_scene_id IS NOT NULL))
);
CREATE INDEX writing_goal_active ON writing_goal(archived,start_date,end_date);

CREATE TABLE writing_goal_event (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL REFERENCES writing_goal(id) ON DELETE CASCADE,
    occurred_at INTEGER NOT NULL,
    origin TEXT NOT NULL CHECK(origin IN ('editor','write-or-die','dictation','import','manual','reconciliation')),
    source_identifier TEXT NOT NULL CHECK(length(trim(source_identifier)) > 0),
    source_fingerprint TEXT NOT NULL CHECK(length(source_fingerprint) > 0),
    scene_id TEXT REFERENCES scene(id) ON DELETE SET NULL,
    raw_delta INTEGER NOT NULL CHECK(raw_delta >= 0),
    real_delta INTEGER NOT NULL,
    reconciliation_state TEXT NOT NULL DEFAULT 'current' CHECK(reconciliation_state IN ('current','superseded','needs-review')),
    detail TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    UNIQUE(goal_id,origin,source_identifier)
);
CREATE INDEX writing_goal_event_time ON writing_goal_event(goal_id,occurred_at,id);
CREATE INDEX writing_goal_event_scene ON writing_goal_event(goal_id,scene_id,occurred_at);

CREATE TABLE writing_goal_snapshot (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL REFERENCES writing_goal(id) ON DELETE CASCADE,
    snapshot_date TEXT NOT NULL,
    recorded_at INTEGER NOT NULL,
    target_words INTEGER NOT NULL CHECK(target_words >= 0),
    raw_words INTEGER NOT NULL CHECK(raw_words >= 0),
    real_words INTEGER NOT NULL,
    success_metric TEXT NOT NULL CHECK(success_metric IN ('raw','real')),
    UNIQUE(goal_id,snapshot_date)
);
CREATE INDEX writing_goal_snapshot_date ON writing_goal_snapshot(goal_id,snapshot_date);

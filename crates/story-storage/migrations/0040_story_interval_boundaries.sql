CREATE TABLE scene_interval_boundary(
    scene_id TEXT PRIMARY KEY REFERENCES scene(id) ON DELETE CASCADE,
    start_boundary TEXT NOT NULL DEFAULT 'inclusive' CHECK(start_boundary IN ('inclusive','exclusive')),
    end_boundary TEXT NOT NULL DEFAULT 'inclusive' CHECK(end_boundary IN ('inclusive','exclusive'))
);
CREATE TABLE story_event_interval_boundary(
    event_id TEXT PRIMARY KEY REFERENCES story_event(id) ON DELETE CASCADE,
    start_boundary TEXT NOT NULL DEFAULT 'inclusive' CHECK(start_boundary IN ('inclusive','exclusive')),
    end_boundary TEXT NOT NULL DEFAULT 'inclusive' CHECK(end_boundary IN ('inclusive','exclusive'))
);

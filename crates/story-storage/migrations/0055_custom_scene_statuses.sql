CREATE TABLE project_scene_status_custom (
    status_code TEXT PRIMARY KEY CHECK(
        status_code IN ('idea','outlined','ready_to_draft','drafting','draft_complete','revising','complete')
        OR (
            status_code GLOB 'custom_[0-9][0-9][0-9][0-9][0-9]'
            AND status_code <> 'custom_00000'
        )
    ),
    label TEXT NOT NULL CHECK(length(trim(label)) BETWEEN 1 AND 40),
    position INTEGER NOT NULL UNIQUE CHECK(position>=0),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1))
);

INSERT INTO project_scene_status_custom(status_code,label,position,enabled)
SELECT status_code,label,position,enabled FROM project_scene_status;

DROP TABLE project_scene_status;
ALTER TABLE project_scene_status_custom RENAME TO project_scene_status;

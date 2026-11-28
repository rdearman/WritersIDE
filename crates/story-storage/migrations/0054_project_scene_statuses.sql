CREATE TABLE project_scene_status (
    status_code TEXT PRIMARY KEY CHECK(status_code IN (
        'idea','outlined','ready_to_draft','drafting','draft_complete','revising','complete'
    )),
    label TEXT NOT NULL CHECK(length(trim(label)) BETWEEN 1 AND 40),
    position INTEGER NOT NULL UNIQUE CHECK(position>=0),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1))
);

INSERT INTO project_scene_status(status_code,label,position,enabled) VALUES
 ('idea','Not Started',0,1),
 ('outlined','Outlined',1,1),
 ('ready_to_draft','Ready to Draft',2,1),
 ('drafting','First Draft',3,1),
 ('draft_complete','Draft Complete',4,1),
 ('revising','Second Draft',5,1),
 ('complete','Final Draft',6,1);

CREATE TABLE personal_comment(
 id TEXT PRIMARY KEY,
 scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
 anchor_id TEXT REFERENCES passage_anchor(id) ON DELETE SET NULL,
 body TEXT NOT NULL,
 state TEXT NOT NULL DEFAULT 'open' CHECK(state IN('open','resolved')),
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE INDEX personal_comment_scene ON personal_comment(scene_id,state,created_at);
CREATE TRIGGER personal_comment_anchor_insert BEFORE INSERT ON personal_comment WHEN NEW.anchor_id IS NOT NULL BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM passage_anchor WHERE id=NEW.anchor_id AND scene_id=NEW.scene_id) THEN RAISE(ABORT,'comment passage anchor belongs to another scene') END;
END;
CREATE TRIGGER personal_comment_anchor_update BEFORE UPDATE OF anchor_id,scene_id ON personal_comment WHEN NEW.anchor_id IS NOT NULL BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM passage_anchor WHERE id=NEW.anchor_id AND scene_id=NEW.scene_id) THEN RAISE(ABORT,'comment passage anchor belongs to another scene') END;
END;

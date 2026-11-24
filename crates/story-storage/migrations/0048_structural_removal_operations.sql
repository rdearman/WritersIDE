CREATE TABLE structural_removal(
  id TEXT PRIMARY KEY,
  root_node_id TEXT NOT NULL REFERENCES structural_node(id) ON DELETE RESTRICT,
  mode TEXT NOT NULL CHECK(mode IN('move_scenes_unplaced','move_scenes_chapter','delete_subtree')),
  target_chapter_id TEXT REFERENCES structural_node(id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL,
  restored_at TEXT,
  CHECK((mode='move_scenes_chapter' AND target_chapter_id IS NOT NULL)
     OR (mode!='move_scenes_chapter' AND target_chapter_id IS NULL))
);
CREATE INDEX structural_removal_root ON structural_removal(root_node_id,created_at);

CREATE TABLE structural_removal_node(
  removal_id TEXT NOT NULL REFERENCES structural_removal(id) ON DELETE RESTRICT,
  node_id TEXT NOT NULL REFERENCES structural_node(id) ON DELETE RESTRICT,
  was_archived INTEGER NOT NULL CHECK(was_archived IN(0,1)),
  PRIMARY KEY(removal_id,node_id)
);

CREATE TABLE structural_removal_scene(
  removal_id TEXT NOT NULL REFERENCES structural_removal(id) ON DELETE RESTRICT,
  scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE RESTRICT,
  original_chapter_id TEXT REFERENCES structural_node(id) ON DELETE RESTRICT,
  original_position INTEGER,
  original_unplaced_position INTEGER,
  was_archived INTEGER NOT NULL CHECK(was_archived IN(0,1)),
  previous_status TEXT NOT NULL,
  applied_chapter_id TEXT REFERENCES structural_node(id) ON DELETE RESTRICT,
  applied_position INTEGER,
  applied_unplaced_position INTEGER,
  PRIMARY KEY(removal_id,scene_id),
  CHECK((original_chapter_id IS NULL)=(original_position IS NULL)),
  CHECK((applied_chapter_id IS NULL)=(applied_position IS NULL))
);

CREATE TABLE structural_removal_content_unit(
  removal_id TEXT NOT NULL REFERENCES structural_removal(id) ON DELETE RESTRICT,
  content_unit_id TEXT NOT NULL REFERENCES content_unit(id) ON DELETE RESTRICT,
  was_archived INTEGER NOT NULL CHECK(was_archived IN(0,1)),
  PRIMARY KEY(removal_id,content_unit_id)
);

DROP TRIGGER structural_node_archive_requires_empty;
CREATE TRIGGER structural_node_archive_requires_empty
BEFORE UPDATE OF archived ON structural_node
WHEN OLD.archived=0 AND NEW.archived=1 AND (
  EXISTS(SELECT 1 FROM structural_node child WHERE child.parent_id=OLD.id AND child.archived=0)
  OR EXISTS(SELECT 1 FROM scene_placement placement JOIN scene ON scene.id=placement.scene_id
            WHERE placement.chapter_id=OLD.id AND scene.archived=0)
  OR EXISTS(SELECT 1 FROM content_unit unit WHERE unit.parent_node_id=OLD.id AND unit.archived=0)
)
BEGIN
  SELECT RAISE(ABORT,'move or remove active dependent content before removing this section or chapter');
END;

CREATE TRIGGER scene_restore_requires_active_chapter
BEFORE UPDATE OF archived ON scene
WHEN OLD.archived=1 AND NEW.archived=0 AND EXISTS(
  SELECT 1 FROM scene_placement placement JOIN structural_node node ON node.id=placement.chapter_id
  WHERE placement.scene_id=OLD.id AND node.archived=1
)
BEGIN
  SELECT RAISE(ABORT,'restore the chapter before restoring this scene');
END;

CREATE TRIGGER content_unit_restore_requires_active_parent
BEFORE UPDATE OF archived ON content_unit
WHEN OLD.archived=1 AND NEW.archived=0 AND NEW.parent_node_id IS NOT NULL AND EXISTS(
  SELECT 1 FROM structural_node node WHERE node.id=NEW.parent_node_id AND node.archived=1
)
BEGIN
  SELECT RAISE(ABORT,'restore the section or chapter before restoring this prose document');
END;

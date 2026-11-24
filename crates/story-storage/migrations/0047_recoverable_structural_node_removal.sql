ALTER TABLE structural_node ADD COLUMN archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN(0,1));

CREATE INDEX structural_node_archived_order
  ON structural_node(archived,parent_id,position,id);

CREATE TRIGGER structural_node_archive_requires_empty
BEFORE UPDATE OF archived ON structural_node
WHEN OLD.archived=0 AND NEW.archived=1 AND (
  EXISTS(SELECT 1 FROM structural_node child WHERE child.parent_id=OLD.id)
  OR EXISTS(SELECT 1 FROM scene_placement placement WHERE placement.chapter_id=OLD.id)
  OR EXISTS(SELECT 1 FROM content_unit unit WHERE unit.parent_node_id=OLD.id)
)
BEGIN
  SELECT RAISE(ABORT,'move or remove dependent content before removing this section or chapter');
END;

CREATE TRIGGER structural_node_cannot_use_archived_parent_insert
BEFORE INSERT ON structural_node
WHEN NEW.parent_id IS NOT NULL AND EXISTS(
  SELECT 1 FROM structural_node parent WHERE parent.id=NEW.parent_id AND parent.archived=1
)
BEGIN
  SELECT RAISE(ABORT,'cannot place a section under a removed section');
END;

CREATE TRIGGER structural_node_cannot_parent_archived
BEFORE UPDATE OF parent_id ON structural_node
WHEN NEW.parent_id IS NOT NULL AND EXISTS(
  SELECT 1 FROM structural_node parent WHERE parent.id=NEW.parent_id AND parent.archived=1
)
BEGIN
  SELECT RAISE(ABORT,'cannot place a section under a removed section');
END;

CREATE TRIGGER structural_node_restore_requires_active_parent
BEFORE UPDATE OF archived ON structural_node
WHEN OLD.archived=1 AND NEW.archived=0 AND NEW.parent_id IS NOT NULL AND EXISTS(
  SELECT 1 FROM structural_node parent WHERE parent.id=NEW.parent_id AND parent.archived=1
)
BEGIN
  SELECT RAISE(ABORT,'restore the parent section before restoring this section or chapter');
END;

CREATE TRIGGER content_unit_cannot_use_archived_parent
BEFORE INSERT ON content_unit
WHEN NEW.parent_node_id IS NOT NULL AND EXISTS(
  SELECT 1 FROM structural_node node WHERE node.id=NEW.parent_node_id AND node.archived=1
)
BEGIN
  SELECT RAISE(ABORT,'cannot add prose to a removed section or chapter');
END;

CREATE TRIGGER content_unit_cannot_move_to_archived_parent
BEFORE UPDATE OF parent_node_id ON content_unit
WHEN NEW.parent_node_id IS NOT NULL AND EXISTS(
  SELECT 1 FROM structural_node node WHERE node.id=NEW.parent_node_id AND node.archived=1
)
BEGIN
  SELECT RAISE(ABORT,'cannot move prose under a removed section or chapter');
END;

CREATE TRIGGER scene_cannot_place_in_archived_chapter
BEFORE INSERT ON scene_placement
WHEN EXISTS(SELECT 1 FROM structural_node node WHERE node.id=NEW.chapter_id AND node.archived=1)
BEGIN
  SELECT RAISE(ABORT,'cannot place a scene in a removed chapter');
END;

CREATE TRIGGER scene_cannot_move_to_archived_chapter
BEFORE UPDATE OF chapter_id ON scene_placement
WHEN EXISTS(SELECT 1 FROM structural_node node WHERE node.id=NEW.chapter_id AND node.archived=1)
BEGIN
  SELECT RAISE(ABORT,'cannot place a scene in a removed chapter');
END;

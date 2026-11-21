CREATE TABLE planning_board(
 id TEXT PRIMARY KEY,
 title TEXT NOT NULL,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE TABLE planning_board_item(
 id TEXT PRIMARY KEY,
 board_id TEXT NOT NULL REFERENCES planning_board(id) ON DELETE CASCADE,
 entity_kind TEXT NOT NULL CHECK(entity_kind IN('scene','character','plotline','location','event','object')),
 entity_id TEXT NOT NULL,
 x REAL NOT NULL DEFAULT 0,
 y REAL NOT NULL DEFAULT 0,
 width REAL NOT NULL DEFAULT 24 CHECK(width>0),
 height REAL NOT NULL DEFAULT 8 CHECK(height>0),
 note TEXT NOT NULL DEFAULT '',
 UNIQUE(board_id,entity_kind,entity_id)
);
CREATE INDEX planning_board_item_order ON planning_board_item(board_id,y,x,id);
CREATE TABLE planning_board_connector(
 id TEXT PRIMARY KEY,
 board_id TEXT NOT NULL REFERENCES planning_board(id) ON DELETE CASCADE,
 source_item_id TEXT NOT NULL REFERENCES planning_board_item(id) ON DELETE CASCADE,
 target_item_id TEXT NOT NULL REFERENCES planning_board_item(id) ON DELETE CASCADE,
 label TEXT NOT NULL DEFAULT '',
 CHECK(source_item_id<>target_item_id),
 UNIQUE(board_id,source_item_id,target_item_id)
);
CREATE TRIGGER planning_board_entity_insert BEFORE INSERT ON planning_board_item BEGIN
 SELECT CASE WHEN NOT (
  (NEW.entity_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='location' AND EXISTS(SELECT 1 FROM location WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='event' AND EXISTS(SELECT 1 FROM story_event WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='object' AND EXISTS(SELECT 1 FROM story_object WHERE id=NEW.entity_id))
 ) THEN RAISE(ABORT,'planning board entity does not exist') END;
END;
CREATE TRIGGER planning_board_entity_update BEFORE UPDATE OF entity_kind,entity_id ON planning_board_item BEGIN
 SELECT CASE WHEN NOT (
  (NEW.entity_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='location' AND EXISTS(SELECT 1 FROM location WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='event' AND EXISTS(SELECT 1 FROM story_event WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='object' AND EXISTS(SELECT 1 FROM story_object WHERE id=NEW.entity_id))
 ) THEN RAISE(ABORT,'planning board entity does not exist') END;
END;
CREATE TRIGGER planning_board_connector_scope_insert BEFORE INSERT ON planning_board_connector BEGIN
 SELECT CASE WHEN NOT (
  EXISTS(SELECT 1 FROM planning_board_item WHERE id=NEW.source_item_id AND board_id=NEW.board_id) AND
  EXISTS(SELECT 1 FROM planning_board_item WHERE id=NEW.target_item_id AND board_id=NEW.board_id)
 ) THEN RAISE(ABORT,'board connector endpoints must belong to the same board') END;
END;

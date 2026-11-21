CREATE TABLE story_object(
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL,
 description TEXT NOT NULL DEFAULT ''
);
CREATE INDEX story_object_name ON story_object(name COLLATE NOCASE);

CREATE TABLE detected_mention(
 id TEXT PRIMARY KEY,
 scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
 start_byte INTEGER NOT NULL CHECK(start_byte>=0),
 end_byte INTEGER NOT NULL CHECK(end_byte>start_byte),
 quote TEXT NOT NULL,
 source_sha256 TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN('detected','ambiguous','confirmed','dismissed','needs_review')),
 entity_kind TEXT CHECK(entity_kind IS NULL OR entity_kind IN('character','location','object')),
 entity_id TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 CHECK((state IN('confirmed','needs_review') AND entity_kind IS NOT NULL AND entity_id IS NOT NULL) OR
       (state NOT IN('confirmed','needs_review') AND entity_id IS NULL))
);
CREATE INDEX detected_mention_scene ON detected_mention(scene_id,start_byte);
CREATE TABLE detected_mention_candidate(
 mention_id TEXT NOT NULL REFERENCES detected_mention(id) ON DELETE CASCADE,
 entity_kind TEXT NOT NULL CHECK(entity_kind IN('character','location','object')),
 entity_id TEXT NOT NULL,
 matched_name TEXT NOT NULL,
 PRIMARY KEY(mention_id,entity_kind,entity_id,matched_name)
);
CREATE TRIGGER mention_entity_exists_insert BEFORE INSERT ON detected_mention WHEN NEW.state='confirmed' BEGIN
 SELECT CASE WHEN NOT (
  (NEW.entity_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='location' AND EXISTS(SELECT 1 FROM location WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='object' AND EXISTS(SELECT 1 FROM story_object WHERE id=NEW.entity_id))
 ) THEN RAISE(ABORT,'confirmed mention target does not exist') END;
END;
CREATE TRIGGER mention_entity_exists_update BEFORE UPDATE OF state,entity_kind,entity_id ON detected_mention WHEN NEW.state='confirmed' BEGIN
 SELECT CASE WHEN NOT (
  (NEW.entity_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='location' AND EXISTS(SELECT 1 FROM location WHERE id=NEW.entity_id)) OR
  (NEW.entity_kind='object' AND EXISTS(SELECT 1 FROM story_object WHERE id=NEW.entity_id))
 ) THEN RAISE(ABORT,'confirmed mention target does not exist') END;
END;
CREATE TABLE scratchpad_note(
 id TEXT PRIMARY KEY,
 title TEXT NOT NULL,
 body TEXT NOT NULL DEFAULT '',
 pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN(0,1)),
 position INTEGER NOT NULL CHECK(position>=0),
 state TEXT NOT NULL DEFAULT 'active' CHECK(state IN('active','promoted','archived')),
 destination_kind TEXT,
 destination_id TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 CHECK((destination_kind IS NULL)=(destination_id IS NULL)),
 CHECK(destination_kind IS NULL OR destination_kind IN('scene','character','location','object','plotline','research'))
);
CREATE UNIQUE INDEX scratchpad_position ON scratchpad_note(position) WHERE state='active';
CREATE TRIGGER scratchpad_destination_insert BEFORE INSERT ON scratchpad_note WHEN NEW.destination_kind IS NOT NULL BEGIN
 SELECT CASE WHEN NOT (
  (NEW.destination_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='location' AND EXISTS(SELECT 1 FROM location WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='object' AND EXISTS(SELECT 1 FROM story_object WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.destination_id))
 ) THEN RAISE(ABORT,'scratchpad destination does not exist') END;
END;
CREATE TRIGGER scratchpad_destination_update BEFORE UPDATE OF destination_kind,destination_id ON scratchpad_note WHEN NEW.destination_kind IS NOT NULL BEGIN
 SELECT CASE WHEN NOT (
  (NEW.destination_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='location' AND EXISTS(SELECT 1 FROM location WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='object' AND EXISTS(SELECT 1 FROM story_object WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.destination_id)) OR
  (NEW.destination_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.destination_id))
 ) THEN RAISE(ABORT,'scratchpad destination does not exist') END;
END;
CREATE TRIGGER scratchpad_scene_delete AFTER DELETE ON scene BEGIN
 UPDATE scratchpad_note SET state='active',destination_kind=NULL,destination_id=NULL,updated_at=CAST(strftime('%s','now') AS TEXT) WHERE destination_kind='scene' AND destination_id=OLD.id;
END;
CREATE TRIGGER scratchpad_character_delete AFTER DELETE ON character BEGIN
 UPDATE scratchpad_note SET state='active',destination_kind=NULL,destination_id=NULL,updated_at=CAST(strftime('%s','now') AS TEXT) WHERE destination_kind='character' AND destination_id=OLD.id;
END;
CREATE TRIGGER scratchpad_location_delete AFTER DELETE ON location BEGIN
 UPDATE scratchpad_note SET state='active',destination_kind=NULL,destination_id=NULL,updated_at=CAST(strftime('%s','now') AS TEXT) WHERE destination_kind='location' AND destination_id=OLD.id;
END;
CREATE TRIGGER scratchpad_object_delete AFTER DELETE ON story_object BEGIN
 UPDATE scratchpad_note SET state='active',destination_kind=NULL,destination_id=NULL,updated_at=CAST(strftime('%s','now') AS TEXT) WHERE destination_kind='object' AND destination_id=OLD.id;
END;
CREATE TRIGGER scratchpad_plotline_delete AFTER DELETE ON plotline BEGIN
 UPDATE scratchpad_note SET state='active',destination_kind=NULL,destination_id=NULL,updated_at=CAST(strftime('%s','now') AS TEXT) WHERE destination_kind='plotline' AND destination_id=OLD.id;
END;
CREATE TRIGGER scratchpad_research_delete AFTER DELETE ON research_item BEGIN
 UPDATE scratchpad_note SET state='active',destination_kind=NULL,destination_id=NULL,updated_at=CAST(strftime('%s','now') AS TEXT) WHERE destination_kind='research' AND destination_id=OLD.id;
END;

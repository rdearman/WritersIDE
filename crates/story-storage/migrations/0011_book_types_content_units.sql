ALTER TABLE project ADD COLUMN book_type TEXT NOT NULL DEFAULT 'fiction'
  CHECK(book_type IN('fiction','nonfiction','custom'));

CREATE TABLE content_unit(
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK(kind IN('front_matter','section_body','chapter_body','back_matter')),
  title TEXT NOT NULL,
  parent_node_id TEXT REFERENCES structural_node(id) ON DELETE RESTRICT,
  position INTEGER NOT NULL CHECK(position>=0),
  target_words INTEGER CHECK(target_words IS NULL OR target_words>=0),
  relative_path TEXT NOT NULL UNIQUE,
  sha256 TEXT NOT NULL,
  byte_length INTEGER NOT NULL CHECK(byte_length>=0),
  word_count INTEGER NOT NULL DEFAULT 0 CHECK(word_count>=0),
  generation INTEGER NOT NULL DEFAULT 0 CHECK(generation>=0),
  archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN(0,1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK((kind IN('section_body','chapter_body') AND parent_node_id IS NOT NULL)
     OR (kind IN('front_matter','back_matter') AND parent_node_id IS NULL))
);
CREATE UNIQUE INDEX content_unit_sibling_position
  ON content_unit(parent_node_id,position) WHERE parent_node_id IS NOT NULL;
CREATE UNIQUE INDEX content_unit_root_position
  ON content_unit(kind,position) WHERE parent_node_id IS NULL;
CREATE UNIQUE INDEX content_unit_one_body_per_node
  ON content_unit(parent_node_id,kind) WHERE kind IN('section_body','chapter_body');
CREATE INDEX content_unit_parent_order
  ON content_unit(parent_node_id,position,id);

CREATE TABLE content_unit_search_content(
  content_unit_id TEXT PRIMARY KEY REFERENCES content_unit(id) ON DELETE CASCADE,
  content TEXT NOT NULL DEFAULT ''
);

CREATE TRIGGER content_unit_kind_insert BEFORE INSERT ON content_unit BEGIN
  SELECT CASE WHEN EXISTS(SELECT 1 FROM scene WHERE id=NEW.id)
    THEN RAISE(ABORT,'content unit ID is already used by a scene') END;
  SELECT CASE WHEN NEW.kind='chapter_body' AND NOT EXISTS(
    SELECT 1 FROM structural_node WHERE id=NEW.parent_node_id AND kind='chapter')
    THEN RAISE(ABORT,'chapter body must belong to a chapter') END;
  SELECT CASE WHEN NEW.kind='section_body' AND NOT EXISTS(
    SELECT 1 FROM structural_node WHERE id=NEW.parent_node_id AND kind='section')
    THEN RAISE(ABORT,'section body must belong to a section') END;
END;
CREATE TRIGGER content_unit_kind_update BEFORE UPDATE OF kind,parent_node_id ON content_unit BEGIN
  SELECT CASE WHEN NEW.kind='chapter_body' AND NOT EXISTS(
    SELECT 1 FROM structural_node WHERE id=NEW.parent_node_id AND kind='chapter')
    THEN RAISE(ABORT,'chapter body must belong to a chapter') END;
  SELECT CASE WHEN NEW.kind='section_body' AND NOT EXISTS(
    SELECT 1 FROM structural_node WHERE id=NEW.parent_node_id AND kind='section')
    THEN RAISE(ABORT,'section body must belong to a section') END;
END;
CREATE TRIGGER scene_id_not_content_unit BEFORE INSERT ON scene BEGIN
  SELECT CASE WHEN EXISTS(SELECT 1 FROM content_unit WHERE id=NEW.id)
    THEN RAISE(ABORT,'scene ID is already used by a content unit') END;
END;
CREATE TRIGGER scene_update_id_not_content_unit BEFORE UPDATE OF id ON scene BEGIN
  SELECT CASE WHEN EXISTS(SELECT 1 FROM content_unit WHERE id=NEW.id)
    THEN RAISE(ABORT,'scene ID is already used by a content unit') END;
END;
CREATE TRIGGER structural_node_kind_not_content_unit BEFORE UPDATE OF kind ON structural_node BEGIN
  SELECT CASE WHEN EXISTS(SELECT 1 FROM content_unit WHERE parent_node_id=OLD.id AND
    ((kind='chapter_body' AND NEW.kind<>'chapter') OR (kind='section_body' AND NEW.kind<>'section')))
    THEN RAISE(ABORT,'structural kind conflicts with existing direct prose') END;
END;

CREATE TRIGGER content_unit_search_insert AFTER INSERT ON content_unit BEGIN
  INSERT INTO project_search_content(entity_kind,entity_id,title,content)
  VALUES('content_unit',NEW.id,NEW.title,'');
END;
CREATE TRIGGER content_unit_search_update AFTER UPDATE OF title,kind,archived ON content_unit BEGIN
  UPDATE project_search_content SET title=NEW.title,content=COALESCE(
    (SELECT content FROM content_unit_search_content WHERE content_unit_id=NEW.id),'')
  WHERE entity_kind='content_unit' AND entity_id=NEW.id;
END;
CREATE TRIGGER content_unit_search_delete AFTER DELETE ON content_unit BEGIN
  DELETE FROM project_search_content WHERE entity_kind='content_unit' AND entity_id=OLD.id;
END;
CREATE TRIGGER content_unit_search_text_insert AFTER INSERT ON content_unit_search_content BEGIN
  UPDATE project_search_content SET content=NEW.content
  WHERE entity_kind='content_unit' AND entity_id=NEW.content_unit_id;
END;
CREATE TRIGGER content_unit_search_text_update AFTER UPDATE OF content ON content_unit_search_content BEGIN
  UPDATE project_search_content SET content=NEW.content
  WHERE entity_kind='content_unit' AND entity_id=NEW.content_unit_id;
END;

CREATE TABLE content_unit_write_journal(
  tx_id TEXT PRIMARY KEY,
  content_unit_id TEXT NOT NULL REFERENCES content_unit(id) ON DELETE CASCADE,
  target_path TEXT NOT NULL,
  temp_path TEXT NOT NULL,
  candidate_path TEXT NOT NULL,
  old_hash TEXT,
  new_hash TEXT NOT NULL,
  new_length INTEGER NOT NULL,
  next_generation INTEGER NOT NULL,
  state TEXT NOT NULL CHECK(state IN('prepared','db_committed')),
  created_at TEXT NOT NULL
);
CREATE TABLE content_unit_write_conflict(
  tx_id TEXT PRIMARY KEY,
  content_unit_id TEXT NOT NULL REFERENCES content_unit(id) ON DELETE CASCADE,
  recovery_path TEXT NOT NULL,
  expected_hash TEXT,
  actual_hash TEXT,
  candidate_hash TEXT,
  created_at TEXT NOT NULL,
  resolved INTEGER NOT NULL DEFAULT 0,
  decision INTEGER NOT NULL DEFAULT 0 CHECK(decision IN(0,1,2))
);

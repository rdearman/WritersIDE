CREATE TABLE reading_document(
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  media_type TEXT NOT NULL CHECK(media_type IN('text/plain','text/markdown','application/epub+zip')),
  relative_path TEXT NOT NULL UNIQUE,
  source_identifier TEXT NOT NULL,
  source_sha256 TEXT NOT NULL,
  extracted_text TEXT NOT NULL,
  imported_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE reading_annotation(
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES reading_document(id) ON DELETE CASCADE,
  annotation_kind TEXT NOT NULL CHECK(annotation_kind IN('highlight','note')),
  start_byte INTEGER NOT NULL CHECK(start_byte>=0),
  end_byte INTEGER NOT NULL CHECK(end_byte>=start_byte),
  quote TEXT NOT NULL,
  prefix_context TEXT NOT NULL DEFAULT '',
  suffix_context TEXT NOT NULL DEFAULT '',
  source_sha256 TEXT NOT NULL,
  body TEXT NOT NULL DEFAULT '',
  state TEXT NOT NULL DEFAULT 'current' CHECK(state IN('current','needs_review','broken')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX reading_annotation_doc ON reading_annotation(document_id,start_byte);
CREATE TABLE reading_bookmark(
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES reading_document(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  offset_byte INTEGER NOT NULL CHECK(offset_byte>=0),
  quote TEXT NOT NULL DEFAULT '',
  source_sha256 TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'current' CHECK(state IN('current','needs_review')),
  created_at TEXT NOT NULL
);
CREATE TABLE reading_checkpoint(
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES reading_document(id) ON DELETE CASCADE,
  progress_byte INTEGER NOT NULL CHECK(progress_byte>=0),
  source_sha256 TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'current' CHECK(state IN('current','needs_review')),
  updated_at TEXT NOT NULL,
  UNIQUE(document_id)
);
CREATE TABLE reading_term(
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES reading_document(id) ON DELETE CASCADE,
  term_kind TEXT NOT NULL CHECK(term_kind IN('character','species','term')),
  name TEXT NOT NULL,
  aliases TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT '',
  UNIQUE(document_id,term_kind,name)
);
CREATE TABLE reading_entity_link(
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES reading_document(id) ON DELETE CASCADE,
  annotation_id TEXT REFERENCES reading_annotation(id) ON DELETE CASCADE,
  target_kind TEXT NOT NULL CHECK(target_kind IN('scene','character','plotline','research')),
  target_id TEXT NOT NULL,
  relation TEXT NOT NULL DEFAULT 'references'
);
CREATE INDEX reading_entity_link_target ON reading_entity_link(target_kind,target_id);
CREATE VIRTUAL TABLE reading_fts USING fts5(document_id UNINDEXED,title,content,tokenize='unicode61');
INSERT INTO reading_fts(document_id,title,content) SELECT id,title,extracted_text FROM reading_document;
CREATE TRIGGER reading_fts_insert AFTER INSERT ON reading_document BEGIN
  INSERT INTO reading_fts(document_id,title,content) VALUES(NEW.id,NEW.title,NEW.extracted_text);
END;
CREATE TRIGGER reading_fts_update AFTER UPDATE OF title,extracted_text ON reading_document BEGIN
  DELETE FROM reading_fts WHERE document_id=OLD.id;
  INSERT INTO reading_fts(document_id,title,content) VALUES(NEW.id,NEW.title,NEW.extracted_text);
END;
CREATE TRIGGER reading_fts_delete AFTER DELETE ON reading_document BEGIN
  DELETE FROM reading_fts WHERE document_id=OLD.id;
END;
CREATE TRIGGER reading_entity_target_insert BEFORE INSERT ON reading_entity_link BEGIN
  SELECT CASE WHEN NOT (
    (NEW.target_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.target_id)) OR
    (NEW.target_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.target_id)) OR
    (NEW.target_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.target_id)) OR
    (NEW.target_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.target_id))
  ) THEN RAISE(ABORT,'reading entity target does not exist') END;
END;

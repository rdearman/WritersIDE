CREATE TABLE approved_knowledge_claim(
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK(length(trim(kind)) > 0),
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  claim TEXT NOT NULL CHECK(length(trim(claim)) > 0),
  approximate_story_time TEXT,
  source_kind TEXT NOT NULL CHECK(source_kind IN ('scene','reading')),
  source_id TEXT NOT NULL,
  source_fingerprint TEXT NOT NULL CHECK(length(source_fingerprint) = 64),
  source_length_bytes INTEGER NOT NULL CHECK(source_length_bytes >= 0),
  evidence_quote TEXT NOT NULL CHECK(length(evidence_quote) > 0),
  evidence_start_byte INTEGER NOT NULL CHECK(evidence_start_byte >= 0),
  evidence_end_byte INTEGER NOT NULL CHECK(evidence_end_byte > evidence_start_byte),
  model TEXT NOT NULL,
  approved_at TEXT NOT NULL,
  UNIQUE(source_kind,source_id,source_fingerprint,evidence_start_byte,evidence_end_byte,kind,name,claim)
);
CREATE INDEX approved_knowledge_source_idx
  ON approved_knowledge_claim(source_kind,source_id,source_fingerprint);
CREATE TRIGGER approved_knowledge_source_insert BEFORE INSERT ON approved_knowledge_claim BEGIN
  SELECT CASE
    WHEN NEW.source_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=NEW.source_id)
      THEN RAISE(ABORT,'approved knowledge source scene does not exist')
    WHEN NEW.source_kind='reading' AND NOT EXISTS(SELECT 1 FROM reading_document WHERE id=NEW.source_id)
      THEN RAISE(ABORT,'approved knowledge source reading does not exist')
  END;
END;
CREATE TRIGGER approved_knowledge_source_update BEFORE UPDATE OF source_kind,source_id ON approved_knowledge_claim BEGIN
  SELECT CASE
    WHEN NEW.source_kind='scene' AND NOT EXISTS(SELECT 1 FROM scene WHERE id=NEW.source_id)
      THEN RAISE(ABORT,'approved knowledge source scene does not exist')
    WHEN NEW.source_kind='reading' AND NOT EXISTS(SELECT 1 FROM reading_document WHERE id=NEW.source_id)
      THEN RAISE(ABORT,'approved knowledge source reading does not exist')
  END;
END;

CREATE TRIGGER project_search_knowledge_insert AFTER INSERT ON approved_knowledge_claim BEGIN
  INSERT INTO project_search_content(entity_kind,entity_id,title,content)
  VALUES('knowledge_claim',NEW.id,NEW.name,NEW.kind||' '||NEW.claim||' '||COALESCE(NEW.approximate_story_time,''));
END;
CREATE TRIGGER project_search_knowledge_update AFTER UPDATE OF kind,name,claim,approximate_story_time ON approved_knowledge_claim BEGIN
  UPDATE project_search_content SET title=NEW.name,
    content=NEW.kind||' '||NEW.claim||' '||COALESCE(NEW.approximate_story_time,'')
  WHERE entity_kind='knowledge_claim' AND entity_id=NEW.id;
END;
CREATE TRIGGER project_search_knowledge_delete AFTER DELETE ON approved_knowledge_claim BEGIN
  DELETE FROM project_search_content WHERE entity_kind='knowledge_claim' AND entity_id=OLD.id;
END;

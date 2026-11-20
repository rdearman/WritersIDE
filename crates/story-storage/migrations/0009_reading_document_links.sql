DROP TRIGGER story_link_source_exists_insert;
DROP TRIGGER story_link_source_exists_update;
DROP TRIGGER story_link_no_transclusion_cycle;
DROP TRIGGER story_link_inclusion_documents_insert;
DROP TRIGGER story_link_inclusion_documents_update;
DROP INDEX story_link_source;
DROP INDEX story_link_target;

CREATE TABLE story_link_new(
  id TEXT PRIMARY KEY,
  source_kind TEXT NOT NULL CHECK(source_kind IN('scene','character','plotline','research','reading')),
  source_id TEXT NOT NULL,
  source_anchor_id TEXT,
  target_kind TEXT NOT NULL CHECK(target_kind IN('scene','character','plotline','research','reading')),
  target_id TEXT NOT NULL,
  target_anchor_id TEXT,
  link_kind TEXT NOT NULL CHECK(link_kind IN('related','cites','transcludes','includes','navigates')),
  label TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  CHECK(source_kind<>target_kind OR source_id<>target_id OR link_kind NOT IN('transcludes','includes'))
);
INSERT INTO story_link_new(id,source_kind,source_id,source_anchor_id,target_kind,target_id,target_anchor_id,link_kind,label,created_at)
SELECT id,source_kind,source_id,source_anchor_id,target_kind,target_id,target_anchor_id,link_kind,label,created_at FROM story_link;
DROP TABLE story_link;
ALTER TABLE story_link_new RENAME TO story_link;
CREATE INDEX story_link_source ON story_link(source_kind,source_id);
CREATE INDEX story_link_target ON story_link(target_kind,target_id);

CREATE TRIGGER story_link_source_exists_insert BEFORE INSERT ON story_link BEGIN
  SELECT CASE WHEN NOT (
    (NEW.source_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.source_id)) OR
    (NEW.source_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.source_id)) OR
    (NEW.source_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.source_id)) OR
    (NEW.source_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.source_id)) OR
    (NEW.source_kind='reading' AND EXISTS(SELECT 1 FROM reading_document WHERE id=NEW.source_id))
  ) THEN RAISE(ABORT,'story link source does not exist') END;
  SELECT CASE WHEN NOT (
    (NEW.target_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.target_id)) OR
    (NEW.target_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.target_id)) OR
    (NEW.target_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.target_id)) OR
    (NEW.target_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.target_id)) OR
    (NEW.target_kind='reading' AND EXISTS(SELECT 1 FROM reading_document WHERE id=NEW.target_id))
  ) THEN RAISE(ABORT,'story link target does not exist') END;
  SELECT CASE WHEN NEW.source_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor a WHERE a.id=NEW.source_anchor_id AND NEW.source_kind='scene' AND a.scene_id=NEW.source_id) THEN RAISE(ABORT,'source passage anchor mismatch') END;
  SELECT CASE WHEN NEW.target_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor a WHERE a.id=NEW.target_anchor_id AND NEW.target_kind='scene' AND a.scene_id=NEW.target_id) THEN RAISE(ABORT,'target passage anchor mismatch') END;
END;

CREATE TRIGGER story_link_source_exists_update BEFORE UPDATE OF source_kind,source_id,target_kind,target_id,source_anchor_id,target_anchor_id ON story_link BEGIN
  SELECT CASE WHEN NOT (
    (NEW.source_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.source_id)) OR
    (NEW.source_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.source_id)) OR
    (NEW.source_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.source_id)) OR
    (NEW.source_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.source_id)) OR
    (NEW.source_kind='reading' AND EXISTS(SELECT 1 FROM reading_document WHERE id=NEW.source_id))
  ) THEN RAISE(ABORT,'story link source does not exist') END;
  SELECT CASE WHEN NOT (
    (NEW.target_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.target_id)) OR
    (NEW.target_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.target_id)) OR
    (NEW.target_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.target_id)) OR
    (NEW.target_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.target_id)) OR
    (NEW.target_kind='reading' AND EXISTS(SELECT 1 FROM reading_document WHERE id=NEW.target_id))
  ) THEN RAISE(ABORT,'story link target does not exist') END;
  SELECT CASE WHEN NEW.source_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor a WHERE a.id=NEW.source_anchor_id AND NEW.source_kind='scene' AND a.scene_id=NEW.source_id) THEN RAISE(ABORT,'source passage anchor mismatch') END;
  SELECT CASE WHEN NEW.target_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor a WHERE a.id=NEW.target_anchor_id AND NEW.target_kind='scene' AND a.scene_id=NEW.target_id) THEN RAISE(ABORT,'target passage anchor mismatch') END;
END;

CREATE TRIGGER story_link_inclusion_documents_insert BEFORE INSERT ON story_link
WHEN NEW.link_kind IN ('transcludes','includes') BEGIN
  SELECT CASE WHEN NEW.source_kind NOT IN ('scene','research','reading') THEN RAISE(ABORT,'inclusion source must be a document') END;
  SELECT CASE WHEN NEW.target_kind NOT IN ('scene','research','reading') THEN RAISE(ABORT,'inclusion target must be a document') END;
END;
CREATE TRIGGER story_link_inclusion_documents_update BEFORE UPDATE OF source_kind,target_kind,link_kind ON story_link
WHEN NEW.link_kind IN ('transcludes','includes') BEGIN
  SELECT CASE WHEN NEW.source_kind NOT IN ('scene','research','reading') THEN RAISE(ABORT,'inclusion source must be a document') END;
  SELECT CASE WHEN NEW.target_kind NOT IN ('scene','research','reading') THEN RAISE(ABORT,'inclusion target must be a document') END;
END;
CREATE TRIGGER story_link_no_transclusion_cycle BEFORE INSERT ON story_link WHEN NEW.link_kind IN('transcludes','includes') BEGIN
  SELECT CASE WHEN EXISTS(
    WITH RECURSIVE reachable(kind,id) AS (
      SELECT NEW.target_kind,NEW.target_id
      UNION
      SELECT l.target_kind,l.target_id FROM story_link l JOIN reachable r
       ON l.source_kind=r.kind AND l.source_id=r.id WHERE l.link_kind IN('transcludes','includes')
    ) SELECT 1 FROM reachable WHERE kind=NEW.source_kind AND id=NEW.source_id
  ) THEN RAISE(ABORT,'transclusion cycle') END;
END;

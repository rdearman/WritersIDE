CREATE TABLE research_item(
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  body TEXT NOT NULL DEFAULT '',
  media_type TEXT NOT NULL DEFAULT 'text/plain',
  source_path TEXT,
  source_sha256 TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE passage_anchor(
  id TEXT PRIMARY KEY,
  scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
  start_byte INTEGER NOT NULL CHECK(start_byte>=0),
  end_byte INTEGER NOT NULL CHECK(end_byte>=start_byte),
  quote TEXT NOT NULL,
  prefix_context TEXT NOT NULL DEFAULT '',
  suffix_context TEXT NOT NULL DEFAULT '',
  source_sha256 TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'current' CHECK(state IN('current','needs_review','broken')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX passage_anchor_scene ON passage_anchor(scene_id,start_byte);
CREATE TABLE story_link(
  id TEXT PRIMARY KEY,
  source_kind TEXT NOT NULL CHECK(source_kind IN('scene','character','plotline','research')),
  source_id TEXT NOT NULL,
  source_anchor_id TEXT,
  target_kind TEXT NOT NULL CHECK(target_kind IN('scene','character','plotline','research')),
  target_id TEXT NOT NULL,
  target_anchor_id TEXT,
  link_kind TEXT NOT NULL CHECK(link_kind IN('related','cites','transcludes','includes','navigates')),
  label TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  CHECK(source_kind<>target_kind OR source_id<>target_id OR link_kind NOT IN('transcludes','includes'))
);
CREATE INDEX story_link_source ON story_link(source_kind,source_id);
CREATE INDEX story_link_target ON story_link(target_kind,target_id);
CREATE TRIGGER story_link_source_exists_insert BEFORE INSERT ON story_link BEGIN
  SELECT CASE WHEN NOT (
    (NEW.source_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.source_id)) OR
    (NEW.source_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.source_id)) OR
    (NEW.source_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.source_id)) OR
    (NEW.source_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.source_id))
  ) THEN RAISE(ABORT,'story link source does not exist') END;
  SELECT CASE WHEN NOT (
    (NEW.target_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.target_id)) OR
    (NEW.target_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.target_id)) OR
    (NEW.target_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.target_id)) OR
    (NEW.target_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.target_id))
  ) THEN RAISE(ABORT,'story link target does not exist') END;
  SELECT CASE WHEN NEW.source_anchor_id IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM passage_anchor a WHERE a.id=NEW.source_anchor_id AND NEW.source_kind='scene' AND a.scene_id=NEW.source_id
  ) THEN RAISE(ABORT,'source passage anchor mismatch') END;
  SELECT CASE WHEN NEW.target_anchor_id IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM passage_anchor a WHERE a.id=NEW.target_anchor_id AND NEW.target_kind='scene' AND a.scene_id=NEW.target_id
  ) THEN RAISE(ABORT,'target passage anchor mismatch') END;
END;
CREATE TRIGGER story_link_source_exists_update BEFORE UPDATE OF source_kind,source_id,target_kind,target_id,source_anchor_id,target_anchor_id ON story_link BEGIN
  SELECT CASE WHEN NOT (
    (NEW.source_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.source_id)) OR
    (NEW.source_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.source_id)) OR
    (NEW.source_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.source_id)) OR
    (NEW.source_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.source_id))
  ) THEN RAISE(ABORT,'story link source does not exist') END;
  SELECT CASE WHEN NOT (
    (NEW.target_kind='scene' AND EXISTS(SELECT 1 FROM scene WHERE id=NEW.target_id)) OR
    (NEW.target_kind='character' AND EXISTS(SELECT 1 FROM character WHERE id=NEW.target_id)) OR
    (NEW.target_kind='plotline' AND EXISTS(SELECT 1 FROM plotline WHERE id=NEW.target_id)) OR
    (NEW.target_kind='research' AND EXISTS(SELECT 1 FROM research_item WHERE id=NEW.target_id))
  ) THEN RAISE(ABORT,'story link target does not exist') END;
  SELECT CASE WHEN NEW.source_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor a WHERE a.id=NEW.source_anchor_id AND NEW.source_kind='scene' AND a.scene_id=NEW.source_id) THEN RAISE(ABORT,'source passage anchor mismatch') END;
  SELECT CASE WHEN NEW.target_anchor_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM passage_anchor a WHERE a.id=NEW.target_anchor_id AND NEW.target_kind='scene' AND a.scene_id=NEW.target_id) THEN RAISE(ABORT,'target passage anchor mismatch') END;
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
CREATE TABLE saved_query(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE saved_query_filter(
  query_id TEXT NOT NULL REFERENCES saved_query(id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL CHECK(ordinal>=0),
  filter_kind TEXT NOT NULL CHECK(filter_kind IN('status','character','plotline','time_before','time_after','unresolved_issue')),
  value TEXT NOT NULL,
  PRIMARY KEY(query_id,ordinal)
);
CREATE INDEX saved_query_filter_lookup ON saved_query_filter(query_id,filter_kind,value);
CREATE TABLE story_issue(
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN('open','in_progress','resolved','deferred','closed')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE scene_issue(
  scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
  issue_id TEXT NOT NULL REFERENCES story_issue(id) ON DELETE CASCADE,
  PRIMARY KEY(scene_id,issue_id)
);
CREATE INDEX scene_issue_issue ON scene_issue(issue_id,scene_id);
CREATE TABLE scene_search_content(scene_id TEXT PRIMARY KEY REFERENCES scene(id) ON DELETE CASCADE,content TEXT NOT NULL DEFAULT '');
INSERT INTO scene_search_content(scene_id,content) SELECT id,'' FROM scene;
CREATE VIRTUAL TABLE scene_fts USING fts5(scene_id UNINDEXED,title,synopsis,content,tokenize='unicode61');
INSERT INTO scene_fts(scene_id,title,synopsis,content)
  SELECT s.id,s.title,s.synopsis,'' FROM scene s;
CREATE TRIGGER scene_fts_insert AFTER INSERT ON scene BEGIN
  INSERT INTO scene_search_content(scene_id,content) VALUES(NEW.id,'');
  INSERT INTO scene_fts(scene_id,title,synopsis,content) VALUES(NEW.id,NEW.title,NEW.synopsis,'');
END;
CREATE TRIGGER scene_fts_update AFTER UPDATE OF title,synopsis ON scene BEGIN
  DELETE FROM scene_fts WHERE scene_id=OLD.id;
  INSERT INTO scene_fts(scene_id,title,synopsis,content) VALUES(NEW.id,NEW.title,NEW.synopsis,COALESCE((SELECT content FROM scene_search_content WHERE scene_id=NEW.id),''));
END;
CREATE TRIGGER scene_fts_delete AFTER DELETE ON scene BEGIN
  DELETE FROM scene_fts WHERE scene_id=OLD.id;
  DELETE FROM scene_search_content WHERE scene_id=OLD.id;
END;

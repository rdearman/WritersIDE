CREATE TABLE project_search_content(
  row_id INTEGER PRIMARY KEY,
  entity_kind TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT NOT NULL DEFAULT '',
  UNIQUE(entity_kind,entity_id)
);
CREATE VIRTUAL TABLE project_search_fts USING fts5(entity_kind UNINDEXED,entity_id UNINDEXED,title,content,tokenize='unicode61 remove_diacritics 1');
CREATE TRIGGER project_search_content_insert AFTER INSERT ON project_search_content BEGIN
  INSERT INTO project_search_fts(rowid,entity_kind,entity_id,title,content) VALUES(NEW.row_id,NEW.entity_kind,NEW.entity_id,NEW.title,NEW.content);
END;
CREATE TRIGGER project_search_content_update AFTER UPDATE ON project_search_content BEGIN
  DELETE FROM project_search_fts WHERE rowid=OLD.row_id;
  INSERT INTO project_search_fts(rowid,entity_kind,entity_id,title,content) VALUES(NEW.row_id,NEW.entity_kind,NEW.entity_id,NEW.title,NEW.content);
END;
CREATE TRIGGER project_search_content_delete AFTER DELETE ON project_search_content BEGIN
  DELETE FROM project_search_fts WHERE rowid=OLD.row_id;
END;

INSERT INTO project_search_content(entity_kind,entity_id,title,content)
SELECT 'scene',s.id,s.title,s.synopsis||' '||s.purpose||' '||s.goal||' '||s.outcome||' '||s.status||' '||COALESCE(c.content,'')
FROM scene s LEFT JOIN scene_search_content c ON c.scene_id=s.id;
INSERT INTO project_search_content(entity_kind,entity_id,title,content) SELECT 'character',id,name,'' FROM character;
INSERT INTO project_search_content(entity_kind,entity_id,title,content) SELECT 'location',id,name,'' FROM location;
INSERT INTO project_search_content(entity_kind,entity_id,title,content) SELECT 'plotline',id,name,'' FROM plotline;
INSERT INTO project_search_content(entity_kind,entity_id,title,content) SELECT 'event',id,title,description||' '||source_time_text FROM story_event;
INSERT INTO project_search_content(entity_kind,entity_id,title,content) SELECT 'research',id,title,body FROM research_item;
INSERT INTO project_search_content(entity_kind,entity_id,title,content) SELECT 'issue',id,title,status FROM story_issue;
INSERT INTO project_search_content(entity_kind,entity_id,title,content) SELECT 'reading',id,title,extracted_text FROM reading_document;

CREATE TRIGGER project_search_scene_insert AFTER INSERT ON scene BEGIN
  INSERT INTO project_search_content(entity_kind,entity_id,title,content) VALUES('scene',NEW.id,NEW.title,NEW.synopsis||' '||NEW.purpose||' '||NEW.goal||' '||NEW.outcome||' '||NEW.status||' '||COALESCE((SELECT content FROM scene_search_content WHERE scene_id=NEW.id),''));
END;
CREATE TRIGGER project_search_scene_update AFTER UPDATE OF title,synopsis,purpose,goal,outcome,status ON scene BEGIN
  UPDATE project_search_content SET title=NEW.title,content=NEW.synopsis||' '||NEW.purpose||' '||NEW.goal||' '||NEW.outcome||' '||NEW.status||' '||COALESCE((SELECT content FROM scene_search_content WHERE scene_id=NEW.id),'') WHERE entity_kind='scene' AND entity_id=NEW.id;
END;
CREATE TRIGGER project_search_scene_delete AFTER DELETE ON scene BEGIN
  DELETE FROM project_search_content WHERE entity_kind='scene' AND entity_id=OLD.id;
END;
CREATE TRIGGER project_search_scene_text_insert AFTER INSERT ON scene_search_content BEGIN
  UPDATE project_search_content SET content=content||' '||NEW.content WHERE entity_kind='scene' AND entity_id=NEW.scene_id;
END;
CREATE TRIGGER project_search_scene_text_update AFTER UPDATE OF content ON scene_search_content BEGIN
  UPDATE project_search_content SET content=(SELECT synopsis||' '||purpose||' '||goal||' '||outcome||' '||status||' '||NEW.content FROM scene WHERE id=NEW.scene_id) WHERE entity_kind='scene' AND entity_id=NEW.scene_id;
END;
CREATE TRIGGER project_search_scene_text_delete AFTER DELETE ON scene_search_content BEGIN
  UPDATE project_search_content SET content=(SELECT synopsis||' '||purpose||' '||goal||' '||outcome||' '||status FROM scene WHERE id=OLD.scene_id) WHERE entity_kind='scene' AND entity_id=OLD.scene_id;
END;

CREATE TRIGGER project_search_character_insert AFTER INSERT ON character BEGIN INSERT INTO project_search_content(entity_kind,entity_id,title) VALUES('character',NEW.id,NEW.name); END;
CREATE TRIGGER project_search_character_update AFTER UPDATE OF name ON character BEGIN UPDATE project_search_content SET title=NEW.name WHERE entity_kind='character' AND entity_id=NEW.id; END;
CREATE TRIGGER project_search_character_delete AFTER DELETE ON character BEGIN DELETE FROM project_search_content WHERE entity_kind='character' AND entity_id=OLD.id; END;
CREATE TRIGGER project_search_location_insert AFTER INSERT ON location BEGIN INSERT INTO project_search_content(entity_kind,entity_id,title) VALUES('location',NEW.id,NEW.name); END;
CREATE TRIGGER project_search_location_update AFTER UPDATE OF name ON location BEGIN UPDATE project_search_content SET title=NEW.name WHERE entity_kind='location' AND entity_id=NEW.id; END;
CREATE TRIGGER project_search_location_delete AFTER DELETE ON location BEGIN DELETE FROM project_search_content WHERE entity_kind='location' AND entity_id=OLD.id; END;
CREATE TRIGGER project_search_plotline_insert AFTER INSERT ON plotline BEGIN INSERT INTO project_search_content(entity_kind,entity_id,title) VALUES('plotline',NEW.id,NEW.name); END;
CREATE TRIGGER project_search_plotline_update AFTER UPDATE OF name ON plotline BEGIN UPDATE project_search_content SET title=NEW.name WHERE entity_kind='plotline' AND entity_id=NEW.id; END;
CREATE TRIGGER project_search_plotline_delete AFTER DELETE ON plotline BEGIN DELETE FROM project_search_content WHERE entity_kind='plotline' AND entity_id=OLD.id; END;
CREATE TRIGGER project_search_event_insert AFTER INSERT ON story_event BEGIN INSERT INTO project_search_content(entity_kind,entity_id,title,content) VALUES('event',NEW.id,NEW.title,NEW.description||' '||NEW.source_time_text); END;
CREATE TRIGGER project_search_event_update AFTER UPDATE OF title,description,source_time_text ON story_event BEGIN UPDATE project_search_content SET title=NEW.title,content=NEW.description||' '||NEW.source_time_text WHERE entity_kind='event' AND entity_id=NEW.id; END;
CREATE TRIGGER project_search_event_delete AFTER DELETE ON story_event BEGIN DELETE FROM project_search_content WHERE entity_kind='event' AND entity_id=OLD.id; END;
CREATE TRIGGER project_search_research_insert AFTER INSERT ON research_item BEGIN INSERT INTO project_search_content(entity_kind,entity_id,title,content) VALUES('research',NEW.id,NEW.title,NEW.body); END;
CREATE TRIGGER project_search_research_update AFTER UPDATE OF title,body ON research_item BEGIN UPDATE project_search_content SET title=NEW.title,content=NEW.body WHERE entity_kind='research' AND entity_id=NEW.id; END;
CREATE TRIGGER project_search_research_delete AFTER DELETE ON research_item BEGIN DELETE FROM project_search_content WHERE entity_kind='research' AND entity_id=OLD.id; END;
CREATE TRIGGER project_search_issue_insert AFTER INSERT ON story_issue BEGIN INSERT INTO project_search_content(entity_kind,entity_id,title,content) VALUES('issue',NEW.id,NEW.title,NEW.status); END;
CREATE TRIGGER project_search_issue_update AFTER UPDATE OF title,status ON story_issue BEGIN UPDATE project_search_content SET title=NEW.title,content=NEW.status WHERE entity_kind='issue' AND entity_id=NEW.id; END;
CREATE TRIGGER project_search_issue_delete AFTER DELETE ON story_issue BEGIN DELETE FROM project_search_content WHERE entity_kind='issue' AND entity_id=OLD.id; END;
CREATE TRIGGER project_search_reading_insert AFTER INSERT ON reading_document BEGIN INSERT INTO project_search_content(entity_kind,entity_id,title,content) VALUES('reading',NEW.id,NEW.title,NEW.extracted_text); END;
CREATE TRIGGER project_search_reading_update AFTER UPDATE OF title,extracted_text ON reading_document BEGIN UPDATE project_search_content SET title=NEW.title,content=NEW.extracted_text WHERE entity_kind='reading' AND entity_id=NEW.id; END;
CREATE TRIGGER project_search_reading_delete AFTER DELETE ON reading_document BEGIN DELETE FROM project_search_content WHERE entity_kind='reading' AND entity_id=OLD.id; END;

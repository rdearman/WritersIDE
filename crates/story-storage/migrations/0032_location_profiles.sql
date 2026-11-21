ALTER TABLE location ADD COLUMN parent_id TEXT REFERENCES location(id) ON DELETE RESTRICT;
ALTER TABLE location ADD COLUMN description TEXT NOT NULL DEFAULT '';

CREATE TRIGGER location_parent_insert BEFORE INSERT ON location
WHEN NEW.parent_id IS NOT NULL
BEGIN
  SELECT CASE WHEN NEW.parent_id=NEW.id THEN RAISE(ABORT,'location cannot parent itself') END;
END;

CREATE TRIGGER location_parent_update BEFORE UPDATE OF parent_id ON location
WHEN NEW.parent_id IS NOT NULL
BEGIN
  SELECT CASE WHEN NEW.parent_id=NEW.id THEN RAISE(ABORT,'location cannot parent itself') END;
  SELECT CASE WHEN EXISTS(
    WITH RECURSIVE ancestors(id) AS (
      SELECT NEW.parent_id
      UNION ALL
      SELECT location.parent_id FROM location JOIN ancestors ON location.id=ancestors.id
      WHERE location.parent_id IS NOT NULL
    ) SELECT 1 FROM ancestors WHERE id=NEW.id
  ) THEN RAISE(ABORT,'location hierarchy cycle') END;
END;

UPDATE project_search_content
SET content=(SELECT description FROM location WHERE location.id=project_search_content.entity_id)
WHERE entity_kind='location';

CREATE TRIGGER project_search_location_profile_update AFTER UPDATE OF name,parent_id,description ON location
BEGIN
  UPDATE project_search_content
  SET title=NEW.name,content=NEW.description
  WHERE entity_kind='location' AND entity_id=NEW.id;
END;

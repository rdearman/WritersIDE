ALTER TABLE character ADD COLUMN preferred_name TEXT NOT NULL DEFAULT '';
ALTER TABLE character ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE character ADD COLUMN background TEXT NOT NULL DEFAULT '';
ALTER TABLE character ADD COLUMN goals TEXT NOT NULL DEFAULT '';
ALTER TABLE character ADD COLUMN motivations TEXT NOT NULL DEFAULT '';
ALTER TABLE character ADD COLUMN secrets TEXT NOT NULL DEFAULT '';
ALTER TABLE character ADD COLUMN internal_conflict TEXT NOT NULL DEFAULT '';
ALTER TABLE character ADD COLUMN external_conflict TEXT NOT NULL DEFAULT '';
ALTER TABLE character ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
UPDATE character SET preferred_name=name;

CREATE TABLE character_alias(
  character_id TEXT NOT NULL REFERENCES character(id) ON DELETE CASCADE,
  alias TEXT NOT NULL CHECK(length(trim(alias))>0),
  PRIMARY KEY(character_id,alias COLLATE NOCASE)
);

UPDATE project_search_content
SET content=(SELECT c.preferred_name||' '||c.description||' '||c.background||' '||c.goals||' '||c.motivations||' '||c.secrets||' '||c.internal_conflict||' '||c.external_conflict||' '||c.status||' '||COALESCE((SELECT group_concat(alias,' ') FROM character_alias a WHERE a.character_id=c.id),'') FROM character c WHERE c.id=project_search_content.entity_id)
WHERE entity_kind='character';

CREATE TRIGGER project_search_character_profile_update
AFTER UPDATE OF preferred_name,description,background,goals,motivations,secrets,internal_conflict,external_conflict,status ON character
BEGIN
  UPDATE project_search_content SET title=NEW.name,content=NEW.preferred_name||' '||NEW.description||' '||NEW.background||' '||NEW.goals||' '||NEW.motivations||' '||NEW.secrets||' '||NEW.internal_conflict||' '||NEW.external_conflict||' '||NEW.status||' '||COALESCE((SELECT group_concat(alias,' ') FROM character_alias a WHERE a.character_id=NEW.id),'')
  WHERE entity_kind='character' AND entity_id=NEW.id;
END;

CREATE TRIGGER project_search_character_alias_insert
AFTER INSERT ON character_alias
BEGIN
  UPDATE project_search_content SET content=(SELECT preferred_name||' '||description||' '||background||' '||goals||' '||motivations||' '||secrets||' '||internal_conflict||' '||external_conflict||' '||status||' '||COALESCE((SELECT group_concat(alias,' ') FROM character_alias a WHERE a.character_id=character.id),'') FROM character WHERE id=NEW.character_id)
  WHERE entity_kind='character' AND entity_id=NEW.character_id;
END;
CREATE TRIGGER project_search_character_alias_delete
AFTER DELETE ON character_alias
BEGIN
  UPDATE project_search_content SET content=(SELECT preferred_name||' '||description||' '||background||' '||goals||' '||motivations||' '||secrets||' '||internal_conflict||' '||external_conflict||' '||status||' '||COALESCE((SELECT group_concat(alias,' ') FROM character_alias a WHERE a.character_id=character.id),'') FROM character WHERE id=OLD.character_id)
  WHERE entity_kind='character' AND entity_id=OLD.character_id;
END;

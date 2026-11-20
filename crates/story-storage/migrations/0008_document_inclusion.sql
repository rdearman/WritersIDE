-- Inclusion links are limited to manuscript scenes and text-bearing research documents.
-- All other link kinds retain their broader entity relationship behavior.
CREATE TRIGGER story_link_inclusion_documents_insert
BEFORE INSERT ON story_link
WHEN NEW.link_kind IN ('transcludes','includes')
BEGIN
  SELECT CASE WHEN NEW.source_kind NOT IN ('scene','research')
    THEN RAISE(ABORT,'inclusion source must be a scene or research document') END;
  SELECT CASE WHEN NEW.target_kind NOT IN ('scene','research')
    THEN RAISE(ABORT,'inclusion target must be a scene or research document') END;
END;

CREATE TRIGGER story_link_inclusion_documents_update
BEFORE UPDATE OF source_kind,target_kind,link_kind ON story_link
WHEN NEW.link_kind IN ('transcludes','includes')
BEGIN
  SELECT CASE WHEN NEW.source_kind NOT IN ('scene','research')
    THEN RAISE(ABORT,'inclusion source must be a scene or research document') END;
  SELECT CASE WHEN NEW.target_kind NOT IN ('scene','research')
    THEN RAISE(ABORT,'inclusion target must be a scene or research document') END;
END;

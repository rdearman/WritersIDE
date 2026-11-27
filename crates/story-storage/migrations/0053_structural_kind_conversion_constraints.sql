DROP TRIGGER IF EXISTS structural_node_kind_conversion_guard;
CREATE TRIGGER structural_node_kind_conversion_guard
BEFORE UPDATE OF kind ON structural_node
WHEN NEW.kind<>OLD.kind
BEGIN
  SELECT CASE WHEN NEW.kind='chapter' AND EXISTS(
    SELECT 1 FROM structural_node WHERE parent_id=OLD.id
  ) THEN RAISE(ABORT,'a chapter cannot contain structural child documents') END;
  SELECT CASE WHEN NEW.kind<>'chapter' AND EXISTS(
    SELECT 1 FROM scene_placement WHERE chapter_id=OLD.id
  ) THEN RAISE(ABORT,'move or unplace the chapter scenes before converting it') END;
  SELECT CASE WHEN NEW.kind<>'chapter' AND EXISTS(
    SELECT 1 FROM content_unit WHERE parent_node_id=OLD.id AND kind='chapter_body'
  ) THEN RAISE(ABORT,'move or preserve the chapter prose before converting it') END;
  SELECT CASE WHEN NEW.kind<>'section' AND EXISTS(
    SELECT 1 FROM content_unit WHERE parent_node_id=OLD.id AND kind='section_body'
  ) THEN RAISE(ABORT,'move or preserve the section prose before converting it') END;
END;

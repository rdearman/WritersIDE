ALTER TABLE reading_document ADD COLUMN language_code TEXT;

CREATE TRIGGER reading_document_language_insert
BEFORE INSERT ON reading_document
WHEN NEW.language_code IS NOT NULL
 AND (length(NEW.language_code)=0 OR NEW.language_code GLOB '*[^A-Za-z0-9-]*'
      OR substr(NEW.language_code,1,1) GLOB '[^A-Za-z]'
      OR substr(NEW.language_code,-1,1)='-'
      OR NEW.language_code LIKE '%--%')
BEGIN
    SELECT RAISE(ABORT, 'invalid reading document language tag');
END;

CREATE TRIGGER reading_document_language_update
BEFORE UPDATE OF language_code ON reading_document
WHEN NEW.language_code IS NOT NULL
 AND (length(NEW.language_code)=0 OR NEW.language_code GLOB '*[^A-Za-z0-9-]*'
      OR substr(NEW.language_code,1,1) GLOB '[^A-Za-z]'
      OR substr(NEW.language_code,-1,1)='-'
      OR NEW.language_code LIKE '%--%')
BEGIN
    SELECT RAISE(ABORT, 'invalid reading document language tag');
END;

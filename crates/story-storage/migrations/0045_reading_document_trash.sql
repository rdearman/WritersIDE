ALTER TABLE reading_document
  ADD COLUMN archived INTEGER NOT NULL DEFAULT 0
  CHECK(archived IN (0,1));

CREATE INDEX reading_document_archive_title
  ON reading_document(archived,title COLLATE NOCASE,id);

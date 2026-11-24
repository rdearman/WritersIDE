ALTER TABLE scene ADD COLUMN status_before_archive TEXT;

UPDATE scene
SET status_before_archive = CASE WHEN status='archived' THEN 'idea' ELSE status END
WHERE archived=1;

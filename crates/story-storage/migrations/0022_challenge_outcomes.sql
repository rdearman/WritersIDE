ALTER TABLE writing_challenge ADD COLUMN words_deleted INTEGER NOT NULL DEFAULT 0 CHECK(words_deleted >= 0);
ALTER TABLE writing_challenge ADD COLUMN pauses_consumed INTEGER NOT NULL DEFAULT 0 CHECK(pauses_consumed >= 0);
ALTER TABLE writing_challenge ADD COLUMN outcome TEXT NOT NULL DEFAULT 'pending' CHECK(outcome IN ('pending','success','timeout','quit'));
ALTER TABLE writing_challenge ADD COLUMN display_mode TEXT NOT NULL DEFAULT 'steady' CHECK(display_mode IN ('steady','pressure','kamikaze'));
UPDATE writing_challenge SET outcome=CASE WHEN status='abandoned' THEN 'quit' WHEN status='completed' THEN 'timeout' ELSE 'pending' END;
UPDATE writing_challenge SET display_mode=mode;

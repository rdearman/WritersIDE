ALTER TABLE project ADD COLUMN word_count_policy TEXT NOT NULL DEFAULT 'unicode-words'
  CHECK(word_count_policy IN ('unicode-words','script-graphemes'));

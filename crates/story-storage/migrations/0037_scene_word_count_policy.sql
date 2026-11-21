ALTER TABLE scene ADD COLUMN word_count_policy TEXT
  CHECK(word_count_policy IS NULL OR word_count_policy IN ('unicode-words','script-graphemes'));

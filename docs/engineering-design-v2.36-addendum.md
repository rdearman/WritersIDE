# Engineering Design v2.36 Addendum — Editor Selection Commands

## Scope

The Edit menu now provides explicit **Select Word** and **Select Paragraph**
commands, extending the current source-based manuscript editor without changing
its storage format or introducing a second selection model. This is a focused
increment for GitHub issue #61; it does not claim the complete word-processor
requirements are finished.

## Selection contract

- Selection endpoints remain editor positions backed by canonical UTF-8 source
  offsets and grapheme-aware row/column conversion.
- Select Word selects the word containing the cursor. If the cursor is on
  whitespace, it selects the next word; at the end of the document it falls
  back to the previous word. Alphanumeric graphemes and underscores form words;
  apostrophes between alphanumeric graphemes remain internal.
- Select Paragraph selects contiguous non-empty source lines bounded by blank
  lines. A blank current line has no paragraph to select.
- Both commands clear extra cursors, respect narrowed editing ranges, report
  boundary failures, and leave manuscript bytes untouched.
- Selection is transient editor state and is not added to project persistence.

## Verification

Automated coverage exercises both actions through the Edit command dispatcher,
including accented text, apostrophes, multiline paragraphs, blank-line
boundaries, narrowed passages, real Linux PTY menu dispatch, and save/reopen
source integrity. Human UAT remains required for discoverability and expected
word-boundary behavior.

## Remaining #61 scope

These selection actions do not implement rendered/source editing-coordinate
translation, paragraph/selection duplication, explicit deduplication,
transposition, block selection, full formatting vocabulary, configurable
writing-canvas telemetry, or safe continuous chapter/manuscript editing.

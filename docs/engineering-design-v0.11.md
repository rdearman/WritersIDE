# Writer's IDE — Engineering Design v0.11

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.10

## Saved-query evidence in scene metadata

Saved literal text predicates now return evidence ranges for all three searched scene sources: the canonical Markdown body, scene title, and scene synopsis. `QueryEvidence.passage_bytes` remains reserved for offsets into the canonical manuscript file and is validated against the current content hash. Title/synopsis matches use `field_bytes`, which names the field and supplies a UTF-8 byte range relative to that field's stored value. The terminal displays each range with its source, so metadata offsets cannot be mistaken for manuscript offsets.

Evidence is returned only while the scene is still a current result of the saved query. Literal matching is bounded to 32 matches per source and retains the query's case-insensitive ASCII behavior. General FTS syntax and matches in non-scene project documents still have no exact source mapping; the implementation does not claim arbitrary FTS expressions can be traced to a specific passage.

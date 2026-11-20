# Writer's IDE — Engineering Design v0.15

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.14

## Reading-to-story navigation

Reading documents can link either the full document or a specific annotation to a scene, character, plotline, or research entity. The application exposes these relationships as stable target IDs. In the Reading workspace, `reading-links` lists them and `reading-follow <zero-based-index>` routes the selection to the relevant story workspace. Scene links open the scene editor; character, plotline, and research links switch to the corresponding workspace and select the matching record.

These links remain separate from source bytes and persist across document reload/reanchoring. `jump <byte-offset>` navigates within a reading passage; annotations and bookmarks use the same stable byte-oriented anchor policy described in Engineering Design v0.4.

Acceptance tests persist a reading annotation-to-character relationship and exercise terminal navigation from imported material to the character workspace.

# Engineering Design v2.20 Addendum — Manuscript Editor Audit for #61

Date: 24 November 2026  
Status: Audit complete; implementation plan recorded; no editor behavior or storage format changed.

## Existing architecture

- Manuscript files remain canonical UTF-8 Markdown, saved by the existing journalled, hash-checked file service. SQLite stores scene/content-unit metadata and paths.
- `story-document::SemanticDocument` parses Markdown with `pulldown-cmark` and UTF-8 source byte ranges. Its source string is authoritative and the semantic projection is rebuildable. The compile pipeline already consumes a renderer-neutral IR.
- The terminal editor currently loads canonical source into `Vec<String>`. Cursor and selection coordinates are line plus grapheme index; operations convert those positions to UTF-8 byte offsets. Search, replacement, proofing, mentions, undo and save validation also address canonical source offsets.
- The drawing path emits those editable source lines directly to Ratatui. Markdown delimiters therefore appear in the writing canvas. The editor wraps source lines for display, but has no display-to-source map for hidden markup, semantic formatting, or a centered prose column.
- Formatting marks are a separate display preference. They display tabs and paragraph/line endings; they do not parse or hide Markdown syntax. No formatting toolbar or semantic bold/italic editor action is currently connected to the manuscript buffer.
- `story-document` already depends on `pulldown-cmark`, `sha2` and Unicode segmentation. The terminal crate uses Unicode grapheme segmentation and cell-width support. The audit found no need for a database migration or a replacement manuscript storage format.

## Invariants to preserve

1. Canonical source bytes, scene identity, journal/recovery behavior and compile output are authoritative.
2. Display-only rendering, wrapping and alignment never insert or remove source bytes.
3. Every editable display position maps to an unambiguous UTF-8 source boundary; hidden delimiters must not create ambiguous cursor positions.
4. Search, selection, undo, clipboard, proofing and stale-hash checks continue to operate on source ranges and are translated only at the editor boundary.
5. Unsupported Markdown constructs stay lossless and discoverable in source view; rendering must not silently reinterpret or discard them.
6. Formatting marks and Markdown source visibility remain independent controls.

## Incremental implementation plan

1. Add a renderer-neutral editor projection in `story-document`: display text/semantic runs plus a per-grapheme source-range map, built from the existing parser and offset ranges. Cover paragraphs, headings, emphasis, strong text, links, lists, code and Unicode. Preserve unsupported syntax as visible source text.
2. Prove round-trip and mapping contracts with model tests, including cursor positions at hidden delimiter boundaries, nested formatting, malformed Markdown, combining marks, emoji/wide graphemes and line wrapping. Do not change the active editor until this map is complete.
3. Add an explicit Source/Rendered view toggle and route mouse-free cursor movement, selection, insertion/deletion, undo, clipboard, find/replace and proofing through the projection map. Initially keep unsupported constructs source-visible and provide immediate fallback to Source view.
4. Add semantic formatting commands for supported constructs only, operating on selections or insertion state and represented in Markdown. Test save/reopen, undo and compile output. Do not invent underline/alignment semantics.
5. Add configurable writing-column presentation and a concise two-row location/count/save telemetry area, using terminal cell widths rather than bytes. Keep all display preferences project/user presentation settings, not manuscript content.
6. Defer editable chapter/whole-book scopes until boundary, selection, paste, deletion and undo semantics are specified. A future continuous view may be read-only first.
7. Run PTY tests at 80x24 and larger sizes plus human UAT with an existing manuscript before marking #61 complete.

## Risks and explicit deferrals

- The current editor's pervasive source-coordinate contract makes a direct "strip markup before drawing" change unsafe; it would break cursor and editing offsets.
- Terminal width and grapheme-cell width differ from UTF-8 byte offsets. The projection must keep all three coordinate systems explicit.
- Markdown has ambiguous/incomplete constructs while being typed. The source view must remain immediately available, and incomplete syntax must not make editing impossible.
- This audit does not implement rendered editing, formatting commands, writing-column preferences, or continuous chapter/manuscript editing. No acceptance criterion for #61 is claimed complete.


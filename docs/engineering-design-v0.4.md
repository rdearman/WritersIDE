# Writer's IDE — Engineering Design v0.4

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.3 and SRS v0.2 addendum  
**Migration level:** 3

This version records the first end-to-end full-screen terminal and reference-reading implementation. It retains all Phase 1 invariants and the links/query contracts in v0.3.

## Reading domain and storage

`story-reading` imports UTF-8 text and Markdown directly and reads EPUB spine items through the offline `epub` parser. Its output includes title, media type, original source bytes, extracted searchable text and SHA-256. Parsing is local and does not change the source.

Storage copies original bytes to a project-relative, content-addressed name under `research/readings/`. `reading_document` keeps the stable document UUID, source identifier, relative copy path, fingerprint and extracted text. Access verifies the copy hash and returns a conflict if it was externally modified. Reload writes a new fingerprinted copy and switches the database pointer transactionally; the old version remains available as a file.

Annotations store byte offsets, exact quote, context, source hash, annotation type/body and review state separately from reference bytes. Creation validates UTF-8 boundaries and exact quote correspondence. On reload, a unique exact quote is reanchored to its new position; absent or duplicated quote text is marked `needs_review`. Bookmarks use the same unique-quote policy. Reading checkpoints without a quote are marked for review after source changes. All such states are explicit and do not mutate the reference copy.

`reading_fts` indexes extracted text and titles. `reading_term` stores per-document character, species and terminology entries. `reading_entity_link` connects a document or annotation to stable story entity IDs with a target integrity trigger. The annotations, bookmark, term and entity-link APIs belong to the reusable core/application path; the terminal Reading workspace exposes import, reading, text search, passage selection, annotation, bookmarks, checkpoints, index entries and entity links.

## Terminal implementation boundary

`apps/terminal` is a Ratatui/Crossterm alternate-screen application. It has 13 workspace routes, function-key switching, `?` help, a `:` command palette, `/` search, Tab switching, Alt-Left/Right navigation history, narrow-terminal stacked panes, keyboard editing, bracketed paste, word deletion, save state and conflict reporting. The editor edits the selected scene through the existing optimistic hash and durable journal service. Dirty content is never discarded by scene selection or quit; Ctrl-S is required before opening a different scene or exiting.

The current terminal implementation is an early functional shell: Outline/Cards/Search and manuscript editing work; plot/character/story-bible/timeline/analytics/research/issue views expose current project summaries. It does not yet implement all workspace-specific operations, configurable keymaps, mouse-driven editing, robust cursor/selection rendering, or every SRS function-key/workspace contract. Those remain outstanding terminal requirements, not implied complete by the workspace names.

## Migration 0003 schema

Migration 0003 adds:

- `reading_document`, `reading_annotation`, `reading_bookmark`, `reading_checkpoint`, and `reading_term`.
- `reading_entity_link` with a target type check and insert-time existence validation.
- FTS5 `reading_fts` plus maintenance triggers.
- Annotation/bookmark review state to make reload ambiguity visible.

Version 1 projects migrate through 0002 and 0003 on open; version 2 projects apply 0003; newer versions are rejected. Reading originals are copied into project-relative directories and do not replace manuscript storage or write-journal semantics.

## Failure contracts

- Unsupported extension, unreadable source, invalid UTF-8 text, malformed EPUB, or bad fingerprint fails without a reading document record.
- A failed DB insertion removes the newly written copy when safe; a failed reload transaction leaves the old DB pointer and source data intact. Newly written unreferenced copies are harmless and may be cleaned by later maintenance.
- An externally modified managed copy yields a conflict and is not silently reimported.
- A refreshed source preserves annotations and marks exact/unique passage matches current; non-unique/missing text is `needs_review`.
- Passage ranges outside the extracted UTF-8 string or splitting a code point are rejected.
- FTS search errors are returned to the caller; search never edits source content.
- Terminal raw mode and alternate screen are restored on normal application exit or returned error.

## Acceptance coverage

Automated tests cover plain/Markdown parsing without source mutation, common EPUB HTML text extraction, copied-reference persistence, FTS search, annotation range validation, reanchoring after reload, bookmark/checkpoint review, terminology persistence and version-1-to-3 migrations. Terminal tests cover keyboard save and protection against losing dirty scene text when selection changes.

No real EPUB fixture exists in the repository; end-to-end EPUB package compatibility remains unverified. The EPUB parser dependency and spine extraction path are integrated, but a real sample should be added before making a broad compatibility claim.

## Remaining decisions

- Reader rendering fidelity for rich Markdown and complex EPUB XHTML/CSS.
- Accessible styling for highlights and inline annotations in the terminal.
- Managing/removing obsolete content-addressed source copies.
- Persisted configurable key bindings and consistent pane focus.
- How reading indexes should interact with authoritative character/species/worldbuilding records.
- Whether reading source revisions should become a user-visible immutable revision list.

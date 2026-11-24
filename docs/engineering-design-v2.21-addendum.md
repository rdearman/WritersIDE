# Engineering Design v2.21 Addendum — Recoverable Removal (#63)

Date: 24 November 2026  
Status: Partial implementation; human verification remains open.

## Existing behavior reused

- Scene identity, prose, placement and relationships are already stored independently. The `scene.archived` flag excludes archived scenes from the normal scene listing and compile input.
- Reading documents are project-owned copies. Their source path is provenance only; removal must not delete or modify the source file.
- Structural section/chapter removal is recoverable and preserves the original node identity and position. Nodes with descendants or attached manuscript content remain protected until explicit rehoming/deletion choices are implemented.

## Implemented contracts

- Migration 0045 adds `reading_document.archived`. Active reference listings and reading/project search omit removed references; the copied document, annotation anchors and stable ID remain available for restoration.
- Migration 0046 adds `scene.status_before_archive`. Archiving a scene records its prior status in the same SQLite transaction that marks it archived and changes its visible status to `archived`. Restoration reactivates the same scene ID, restores the prior status and preserves placement, prose, relationships, metadata and snapshots.
- `Edit > Move Reference to Removed` and `Tools > Show Removed References / Restore Selected Reference` expose the reading-document workflow. A named confirmation distinguishes removal from source deletion.
- `Scene > Move Scene to Removed / Show Removed Scenes / Restore Selected Scene` provides the corresponding scene workflow in Scene Cards. The confirmation names the scene and states which data is retained.
- Removed scenes are omitted from normal scene lists, project search and compilation. Restored scenes return to their preserved manuscript position.
- Chapter and section prose documents reuse the pre-existing `content_unit.archived` field. `Edit > Move Prose Document to Removed / Show Removed Prose Documents / Restore Selected Prose Document` keeps the project-owned Markdown file, stable ID, parent, word metadata and text; archived documents disappear from active outline/search/compile results and can be restored in place. No schema migration is needed for this slice.
- Migration 0047 adds `structural_node.archived`. Empty sections and chapters can be moved to a separate Removed Sections and Chapters view and restored in place using visible Edit commands. SQL triggers reject removal while a node has child nodes, scenes or prose documents and reject new content under removed nodes. The menu confirmation states the empty-node constraint; this intentionally avoids silent cascade or scene rehoming.
- Existing archived scenes whose former status was not recorded before migration are assigned `idea` as their restoration status. The previous status was already lost by the older archive operation; the migration does not invent it.

## Failure and consistency rules

- Archive and restore status changes are single SQLite transactions. Repeated archive/restore requests are rejected as invalid state transitions.
- A dirty editor buffer for the selected object prevents removal. A clean open scene buffer is detached after the author confirms removal.
- Removal is recoverable and retains content files. It is not permanent erasure. Structural headings are removable only when empty; descendant rehoming and cascading removal remain unimplemented.
- Database migration and application services remain independent of the terminal UI.

## Verification

- Storage tests cover reopen, source/prose preservation, prior scene status, placement, relationships, search visibility and compilation visibility for scenes and prose documents.
- Terminal state tests exercise confirmation, cancellation, Scene/Edit menu actions, restoration, stable identity and compiler exclusion/restoration.
- The full workspace suite and terminal suite must pass before this milestone is committed.
- #63 remains open for human UAT and for the unimplemented scopes below.

## Explicitly outstanding in #63

- Descendant-aware chapter/section removal with explicit scene rehoming or content removal choices; removal/restore workflows for books and project-owned imported documents beyond chapter/section prose units and the reading-library case.
- A series-level removal workflow and explicit unlink-versus-delete choice.
- A real PTY acceptance test for the reference and scene removal dialogues.
- Permanent deletion with dependency reporting, if requested; current operations only move objects to recoverable removed collections.

# Engineering Design v2.21 Addendum — Recoverable Removal (#63)

Date: 24 November 2026  
Status: Partial implementation; human verification remains open.

## Existing behavior reused

- Scene identity, prose, placement and relationships are already stored independently. The `scene.archived` flag excludes archived scenes from the normal scene listing and compile input.
- Reading documents are project-owned copies. Their source path is provenance only; removal must not delete or modify the source file.
- Structural section/chapter removal is recoverable and preserves original node identities and positions. Populated fiction chapters can rehome scenes or move the full subtree to Removed Documents. Non-fiction sections can promote child headings one level and retain the removed heading's own prose in Removed Documents.

## Implemented contracts

- Migration 0045 adds `reading_document.archived`. Active reference listings and reading/project search omit removed references; the copied document, annotation anchors and stable ID remain available for restoration.
- Migration 0046 adds `scene.status_before_archive`. Archiving a scene records its prior status in the same SQLite transaction that marks it archived and changes its visible status to `archived`. Restoration reactivates the same scene ID, restores the prior status and preserves placement, prose, relationships, metadata and snapshots.
- `Edit > Move Reference to Removed` and `Tools > Show Removed References / Restore Selected Reference` expose the reading-document workflow. A named confirmation distinguishes removal from source deletion.
- `Scene > Move Scene to Removed / Show Removed Scenes / Restore Selected Scene` provides the corresponding scene workflow in Scene Cards. The confirmation names the scene and states which data is retained.
- Removed scenes are omitted from normal scene lists, project search and compilation. Restored scenes return to their preserved manuscript position.
- Chapter and section prose documents reuse the pre-existing `content_unit.archived` field. `Edit > Move Prose Document to Removed / Show Removed Prose Documents / Restore Selected Prose Document` keeps the project-owned Markdown file, stable ID, parent, word metadata and text; archived documents disappear from active outline/search/compile results and can be restored in place. No schema migration is needed for this slice.
- Migration 0047 adds `structural_node.archived`. Migration 0048 adds a recoverable structural-removal operation and membership tables. Migration 0049 records original/applied parent and position for promoted headings. Empty headings and populated subtrees can be retained in Removed Sections and Chapters, then restored without replacing their IDs, hierarchy, relative order, metadata or manuscript files.
- The chapter removal dialogue reports scene and prose counts plus included/excluded scene counts. For a fiction chapter with scenes and direct prose, the default is to move scenes to Unplaced and retain the prose in Removed Documents; the author can instead choose another chapter or delete the chapter and subtree recoverably. The selected action states its Compile effect: unplaced scenes are omitted, included scenes append to a destination chapter in their existing order, or removed scenes/prose are omitted until restoration. Rehoming keeps scene IDs, prose bytes, metadata and inclusion state. Restoration verifies that rehomed scenes have not subsequently moved; it refuses to overwrite a changed placement.
- In non-fiction, a populated section offers promotion to its current parent level, promotion to a selected compatible section, or `Delete the whole subtree recoverably`. Promotion preserves each child's ID, nested subtree and relative order, while the removed heading and directly owned prose remain recoverable together. Restoring the removal restores the original hierarchy and prose; if a promoted child was moved after removal, restoration fails atomically instead of overwriting that move. SQL triggers prevent removing an active node until its active descendants, scenes and prose have been moved or archived, and prevent adding content under removed headings.
- Existing archived scenes whose former status was not recorded before migration are assigned `idea` as their restoration status. The previous status was already lost by the older archive operation; the migration does not invent it.

## Failure and consistency rules

- Archive, rehome and restore operations are single SQLite transactions. Removal records are project-local, idempotent restorations are rejected, and a restore with a moved scene fails without partial updates.
- A dirty editor buffer for the selected object prevents removal. A clean open scene buffer is detached after the author confirms removal.
- Removal is recoverable and retains content files. It is not permanent erasure. Scene inclusion/order changes are described in the dialogue; unplaced scenes no longer compile. No descendant is silently discarded.
- Database migration and application services remain independent of the terminal UI.

## Verification

- Storage tests cover reopen, source/prose preservation, prior scene status, placement, relationships, search visibility, subtree deletion/restoration, chapter rehoming with direct prose, section promotion to parent or selected destination, ordering and refusal to overwrite a scene or heading moved after removal.
- Terminal state tests exercise confirmation, default rehoming to Unplaced, recoverable subtree deletion, non-fiction section promotion, restoration, stable identity and compiler exclusion/restoration.
- The full workspace suite and terminal suite must pass before this milestone is committed.
- #63 remains open for human UAT and for the unimplemented scopes below.

## Explicitly outstanding in #63

- A complete before/after numbered Compile-order listing for large structural removals; current confirmation reports included/excluded counts and the selected action's order/inclusion effect without listing every affected title.
- Directly owned prose is retained with its heading in Removed Documents, but is not yet offered as a separate move into another active compatible content unit.
- A multi-selection or operation-history browser for choosing among several structural removal records; restoration currently selects the latest unrestored operation for the chosen root.
- Removal/restore workflows for books and project-owned imported documents beyond chapter/section prose units and the reading-library case.
- A series-level removal workflow and explicit unlink-versus-delete choice.
- A real PTY acceptance test for the reference and scene removal dialogues.
- Permanent deletion with dependency reporting, if requested; current operations only move objects to recoverable removed collections.

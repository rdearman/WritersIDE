# Engineering Design v2.22 Addendum — Contextual Recoverable Removal

Date: 24 November 2026  
Status: Implemented; human verification remains open under #63 and #54.

## Context

This addendum extends [Engineering Design v2.21](engineering-design-v2.21-addendum.md). It does not change the project, scene, structural-node, content-unit, or series database schemas. It reuses the archive and restoration services established in migrations 0045–0049.

## Terminal interaction contract

- Delete in the active Outline routes the selected row to its existing recoverable removal confirmation: scene, chapter/section prose document, or structural heading. A heading uses the populated-removal dialogue and cannot silently cascade its dependants.
- Delete on an active scene card opens the existing scene removal confirmation. The Removed Scenes collection explains that restoration is available; Delete does not permanently erase an archived scene.
- Delete in the Reference Library retains its existing list-focus behavior. When reference prose has focus, Delete reports that the source is read-only and directs the author back to the list before removal.
- Delete in the Series Books list invokes recoverable membership removal. It does not delete or modify the independent book project. Delete in Removed Books leaves the membership recoverable and reports how to restore it.
- Delete in the manuscript editor remains the ordinary forward-delete text operation. Structural removal handling is limited to object-list workspaces and unmodified Delete.
- Contextual series actions appear in File only in the active Series Books or Removed Books view. The general File menu omits them. `menu.md` reflects the same visibility contract.

## Operation feedback

After a populated chapter or section action, the status message names the action actually performed and the destination. It distinguishes scenes moved to Unplaced, scenes moved to a selected chapter, child sections promoted or rehomed, and a full recoverable subtree removal. If the source heading owns prose, the message states when that prose remains in Removed Documents. This feedback does not imply that a rehomed descendant was archived.

## Verification

- Terminal regression coverage exercises Delete on chapter prose, a chapter, and a scene in Outline; Delete on an active card; cancellation; and ordinary editor deletion.
- Existing regressions cover Reference Library deletion, series membership removal/restoration, populated chapter rehoming, non-fiction section promotion/rehoming, prose retention, identity preservation, compile effects and restore conflicts.
- Formatting and workspace Clippy pass. The full workspace suite passed after the related series-menu work; after the contextual Delete implementation, the full terminal suite passed (298 unit tests, 3 author-workflow tests, 3 PTY tests). The additional row-type assertions passed in a focused rerun.
- No migration was required.

## Remaining work

The outstanding #63 work listed in v2.21 remains open: full before/after Compile order preview, optional movement of heading-owned prose into an active content unit, broader lifecycle operations and dependency handling, explicit book/project deletion policy, and PTY/human acceptance of removal dialogs. #54 remains open for human menu acceptance. Automated tests do not establish author usability.

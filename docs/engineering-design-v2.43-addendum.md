# Engineering Design v2.43 Addendum — Structural Undo and Visible Feedback

Date: 25 November 2026

This addendum records the approved UAT-35–39 correction for recoverable structural removal in issues #55 and #63. It preserves the existing project database and removal journal; no schema migration is needed.

## Structural Undo and Redo

- Edit → Undo and Edit → Redo remain available in the outline and object-list workspaces, independently of whether a manuscript document is open.
- When no editor text operation is available, Undo restores the newest recoverable structural removal from its existing `structural_removal` record. It restores the original nodes, scenes, prose documents, IDs, order and recorded state in one SQLite transaction.
- Redo reapplies the stored removal operation. It checks current scene, prose and heading placements, and destination positions before mutation. If later work conflicts with the recorded operation, the transaction rolls back and reports a recovery conflict; it does not overwrite that work.
- Text Undo/Redo takes precedence while the editor has text history. Scoped replacement undo remains editor-only. Removed Items remains a separate explicit recovery workflow.
- Structural removal history is project-persistent. The latest eligible record determines the operation; a restored removal is eligible for Redo, and a pending removal is eligible for Undo.

## Feedback

At narrow terminal widths, the footer prioritises the latest action result before workspace and save-state hints. This keeps structural recovery, publishing and other command outcomes visible instead of truncating them behind static navigation text.

## Verification added

- Storage tests cover subtree deletion and restoration, chapter scene rehoming to Unplaced and another chapter, persistent IDs/order/prose, redo, and a conflicting destination position that must roll back without overwriting later work.
- Terminal unit coverage invokes Undo and Redo through the Edit menu while no manuscript is open.
- Linux PTY coverage drives chapter removal, Edit → Undo, Edit → Redo and project reopen through the real keyboard UI.

Human UAT remains necessary for discoverability and comfort. This correction does not complete the broader structural-operation undo requirements (such as arbitrary moves/reorders) or issue #63's full object lifecycle matrix.

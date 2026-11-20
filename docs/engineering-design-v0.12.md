# Writer's IDE — Engineering Design v0.12

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.11

## Terminal story-link workflows

The command palette exposes typed link creation (`link <source-kind> <source-id> <target-kind> <target-id> <kind> [label]`), outgoing link inspection, backlinks, broken-link diagnostics, and scene transclusion preview. Endpoint values are parsed as entity kinds plus stable UUIDs; changing a scene title or chapter placement does not rewrite links. `open <scene-uuid>` navigates directly to a linked scene after its UUID is selected from an outgoing/backlink result. The CLI exposes `outgoing` alongside `link`, `backlinks`, and `broken-links`.

Passage anchors remain created through the application API, and selection-linked link creation is not yet available in the TUI. The transclusion preview is a read-only expansion; canonical prose remains in its original scene files. Export inclusion semantics are still deferred.

## Acceptance coverage

Terminal tests create a link through the command palette and confirm outgoing and backlink results show the opposite stable scene ID. Existing storage tests cover rename/move stability, deleted endpoint reporting, and cycle rejection. The implementation does not silently duplicate or mutate the linked scene's text.

# Engineering Design v2.19 Addendum — Scene card positional planning

## Status

Implemented for the Alpha UAT 18 findings in GitHub #55. Human discoverability and usability review remain open.

## Existing model and constraint

`scene_placement` intentionally stores only a scene's optional chapter assignment and position within that chapter. An unplaced scene has no `scene_placement` row, as required for independent scene creation. Before this change, unplaced scenes were displayed by creation time and had no durable reorder operation. Converting `scene_placement.chapter_id` to nullable would weaken the chapter-only placement invariant and mix two separate concepts.

## Decision

Keep `scene_placement` chapter-only. Migration 0044 adds `unplaced_scene_order(scene_id, position)` with a scene foreign key, non-negative unique integer position and a migration backfill ordered by creation time and stable ID. This gives unplaced scenes an explicit persistent order without assigning an arbitrary chapter. Moving a scene into a chapter removes its unplaced-order row in the same transaction; unplacing it removes its chapter placement and inserts its unplaced-order row atomically. Scene content, identity, inclusion state and metadata remain attached to the scene.

## Scene Cards behavior

- Up/Down selects within the current Included or Excluded collection; Tab switches collections.
- Left/Right moves the selected scene by one position, including across chapter boundaries and between Unplaced and the first chapter.
- Shift+Left/Right moves the selected scene's containing chapter as a structural block while preserving its child scene order.
- Insert creates a scene; Shift+Insert creates a chapter at the selected boundary and places the selected scene in it.
- Space retains the existing reversible Include/Exclude action.
- The Scene menu exposes named Move Scene Earlier and Move Scene Later commands alongside the existing chapter chooser.
- Stable scene IDs remain the selection anchor across moves. Compilation follows the canonical scene ordering and continues to omit unplaced and excluded scenes.

## Verification contracts

1. Migration from schema 43 preserves all existing scenes and backfills every unplaced scene exactly once.
2. Unplaced scene reordering is transactional, ordered deterministically and survives project reopen.
3. Placing/unplacing changes only placement/order relationships; text, metadata, identity and inclusion state remain unchanged.
4. Card keyboard tests cover both collections, empty/boundary behavior, unplaced movement, chapter-block moves and selected-scene retention.
5. Compilation and reopen tests confirm that only placed, included scenes appear in the requested order.
6. Direct persistence tests cover foreign keys and unique non-negative unplaced positions.

Automated coverage does not establish that the controls are discoverable. Keep #55 open for human UAT.

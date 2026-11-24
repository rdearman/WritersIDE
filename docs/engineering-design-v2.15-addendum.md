# Engineering Design v2.15 Addendum — Non-fiction Outline Reordering

**Status:** Implemented and regression-tested; human workflow acceptance outstanding  
**Date:** 2026-11-24

## Contract

The terminal outline lets an author move a selected structural chapter or section one sibling earlier or later through **Edit → Move Outline Item Up/Down**. The same commands are registered and available through command search. Commands are disabled when the selection is not a structural row or has no sibling in the requested direction.

Reordering calls the existing `move_structural_node` application service with the node's current parent and the adjacent sibling's position. SQLite remains responsible for transactional position compaction and uniqueness. The operation changes only structural position; IDs, content-unit parents, prose, scenes, links and metadata remain attached to their stable entities. No schema migration was required.

The short Edit menu remains within ten rows including command search. The UUID-oriented scene metadata command is still registered and searchable but is no longer among the short-menu actions.

## Verification

A terminal regression creates two chapters inside a section, attaches and writes direct chapter prose, reorders the chapters through the registered action, and verifies the resulting order, retained prose, zero scene objects and persistence after reopen. It also verifies the move commands are visible in the Edit menu and that an unavailable direction is disabled. The menu curation test enforces the compact row limit.

## Limitations

This operation reorders only among siblings of the same structural kind. Moving chapters between sections remains a separate move operation and is not provided by these menu commands. Human UAT is required to assess discoverability and terminal behavior.

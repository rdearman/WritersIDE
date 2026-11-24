# Engineering Design v2.18 Addendum — Concise direct-action menus

## Status

Implemented in the current development batch, superseding the hierarchical interaction from v2.10 for the primary terminal menus. Human acceptance remains pending under GitHub issues #54 and #49.

## Decision

Issue #54 revises the interaction requested in #49. The menu should be a directly navigable list of author-facing actions. Do not require users to open nested command groups, and do not use Tab or Shift+Tab to switch hidden menu modes. The command palette remains the access point for less frequent registered actions.

## Implementation contract

- Retain the shared command registry as the canonical source for command IDs, labels, categories, shortcuts and dispatch.
- Keep the primary menu lists in `MENU_COMMON_COMMANDS`, containing a deliberate selection of frequent writing and project actions.
- Render a direct list without synthetic group rows or internal task-group vocabulary.
- Up/Down selects a row; Enter dispatches its registered command; Left/Right changes the top-level menu; Esc closes the menu and restores prior focus.
- Keep `Search All Commands…` in primary work menus and retain all other registered commands in searchable command-palette candidates.
- Do not remove command IDs, keybindings, macros or functionality as part of menu curation.
- Put Scene Cards directly in Navigate so the fiction planning view is discoverable.
- Keep menu labels concise. Descriptions belong in command search and contextual help.
- Preserve Ctrl+/ as the preferred menu shortcut with F10 fallback.

## Testable invariants

1. Every curated menu action maps to a registered command in the matching category.
2. Curated primary lists contain action IDs only and do not synthesize group entries.
3. Every registered, non-menu command remains discoverable through command search.
4. Tab and Shift+Tab do not switch menu views or modes.
5. Enter runs a selected action directly; Left/Right changes top-level menus and Esc closes.
6. Existing registry identifiers and shortcut configuration remain intact.
7. Terminal menu labels do not append command descriptions to self-explanatory actions.

## Verification

The terminal regression tests cover registry uniqueness, curated menu/category consistency, command-palette coverage, flat direct action rendering, keyboard traversal, command dispatch and compact terminal row widths. Full workspace formatting, Clippy and tests are required before committing this batch.

## Human acceptance

Automated tests establish the interaction contract, not whether the curated commands are easy for authors to find. Keep #54 open for review of terminology, command placement and discoverability. Issue #49 records the earlier request and should remain available for human review of the revised direction.

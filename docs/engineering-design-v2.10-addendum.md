# Engineering Design v2.10 Addendum — Curated Menu Inventory

**Date:** 23 November 2026  
**Related requirement:** GitHub issue #54; the flat-menu direction supersedes issue #49's earlier request for hierarchical groups.

## Approved menu behavior

- The menu bar contains flat, directly navigable action lists. Up/Down selects, Enter dispatches the selected command, and Esc returns. Tab and Shift+Tab do not switch menu modes.
- Common actions appear in curated primary lists. Other registered commands remain available through Search All Commands and the shared command palette; menu rationalisation does not remove command IDs, shortcuts, macros, or dispatcher behavior.
- Insert Snippet remains a single chooser action. Personal snippet management and text-expansion settings remain under Tools/preferences workflows.
- Command descriptions are not persistently rendered below self-explanatory menu labels. Unavailable commands may show a concise reason.
- Ctrl+/ is the preferred menu binding and F10 remains the fallback; `/` remains ordinary manuscript input.

## Inventory and frequency limits

[`menu-audit-v0.1.md`](menu-audit-v0.1.md) records all 322 registered command IDs with current and proposed placement and disposition. Its visibility tier is qualitative: the application does not collect command-use telemetry, so actual usage frequency is unknown and is not inferred from code. The generated inventory is checked against the registry and curated lists by the menu regression tests.

## Verification boundary

Automated tests verify that every direct menu action resolves to a registry entry, direct lists contain no duplicate command IDs, visible menus stay within their configured size, the flat list is navigable without Tab modes, concise labels are shown, and the full categorized command inventory remains searchable. Human UAT remains necessary to approve the labels, categorization, and discoverability; issue #54 therefore remains open until the author verifies it.

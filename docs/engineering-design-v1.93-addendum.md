# Engineering Design v1.93 Addendum — Curated Terminal Menus

This addendum records the first menu-presentation increment for GitHub Issue #39. It supplements the active [Engineering Design v1.92](engineering-design-v1.92.md) and preserves its history.

## Approved behavior implemented

- The existing typed command registry remains the source of menu actions and dispatch.
- Each top-level menu opens in a curated common-actions view containing at most seven registered commands.
- `Tab` switches that menu to the full set of registered commands in the selected category. `Shift+Tab` returns to the common-actions view.
- Category changes return to common actions. Command identity and dispatch are unchanged between the two views.
- The popup labels the active view and its toggle keys. Context Help reports whether the common or full command list is active.
- Existing commands remain available through the full category view; this increment does not remove command-registry entries.

The curated lists are code-owned presentation metadata, validated against stable command IDs and their actual menu categories. They currently prioritize project/series actions, editing and insertion, navigation, review, publishing, research/tools, and Help.

## Acceptance evidence

Automated tests verify each curated command exists in the registry and belongs to the intended category, every common menu has no more than seven entries, `Tab` exposes the complete registry category, and `Shift+Tab` restores the curated set. Existing import, series, structural, and story-link menu tests remain applicable.

A local 80×24 Linux PTY smoke check opened File with F10, showed the short common list, switched to the full registered list with Tab, returned with Shift+Tab, and closed with Esc. It confirmed visible mode labels and retained the blank-project dashboard. This is not acceptance in the author's terminal emulator, SSH, or tmux.

## Remaining Issue #39 work

This is not the complete menu redesign. The full category view is still a flat list; searchable command-palette integration, nested expert submenus, command priority/availability metadata, fully keymap-derived menu behavior, wireframes, monochrome/SSH/tmux acceptance, and hands-on usability review remain open. The menu lists preserve reachability, but the command palette is not yet a fuzzy searchable registry browser. Do not close Issue #39 based on this increment.

Actual terminal-emulator, SSH, and tmux acceptance remains separate from the automated TestBackend checks and prior local PTY smoke test.

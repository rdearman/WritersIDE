# Engineering Design v1.93 Addendum — Curated Terminal Menus

This addendum records the first menu-presentation increment for GitHub Issue #39. It supplements the active [Engineering Design v1.92](engineering-design-v1.92.md) and preserves its history.

## Approved behavior implemented

- The existing typed command registry remains the source of menu actions and dispatch.
- Each top-level menu opens in a curated common-actions view containing at most seven registered commands.
- `Tab` switches that menu to the full set of registered commands in the selected category. `Shift+Tab` returns to the common-actions view.
- Category changes return to common actions. Command identity and dispatch are unchanged between the two views.
- The popup labels the active view and its toggle keys. Context Help reports whether the common or full command list is active.
- Existing commands remain available through the full category view; this increment does not remove command-registry entries.
- The command palette uses the same registry and dispatcher. `Ctrl+P` is its default command shortcut; `:` opens it through stable command `command.palette-open` outside the manuscript editor, and users may bind the command in the keymap. Search matches command labels, stable IDs, aliases, and descriptions, using exact token matches followed by compact alphanumeric subsequence matches for partial/fuzzy lookup. Up/Down selects, Enter dispatches, Tab prepares an argument prompt for parameterized actions, and Esc closes. In manuscript editing, `:` remains ordinary prose text.
- The palette displays effective bindings and disabled reasons, and Help can be opened and closed without losing the query or selection.

The curated lists are code-owned presentation metadata, validated against stable command IDs and their actual menu categories. They currently prioritize project/series actions, editing and insertion, navigation, review, publishing, research/tools, and Help.

## Acceptance evidence

Automated tests verify each curated command exists in the registry and belongs to the intended category, every common menu has no more than seven entries, `Tab` exposes the complete registry category, and `Shift+Tab` restores the curated set. Command-palette tests cover exact and subsequence search by ID, alias and description; dispatch; argument prompts; Help preservation; default and custom keybindings; no-document entry; colon prose; and rendering from 20×6 through 142×60. Existing import, series, structural, and story-link menu tests remain applicable.

A local 80×24 Linux PTY smoke check opened File with F10, showed the short common list, switched to the full registered list with Tab, returned with Shift+Tab, and closed with Esc. It confirmed visible mode labels and retained the blank-project dashboard. This is not acceptance in the author's terminal emulator, SSH, or tmux.

## Remaining Issue #39 work

This is not the complete menu redesign. The full category view is still a flat list; typo-tolerant edit-distance search, recent/frequent commands, nested expert submenus, command priority metadata, a complete command availability model, keymap-derived bindings for all menu operations, wireframes, monochrome/SSH/tmux acceptance, and hands-on usability review remain open. Palette fuzzy matching currently uses compact alphanumeric subsequences, not edit-distance ranking. Do not close Issue #39 based on this increment.

Actual terminal-emulator, SSH, and tmux acceptance remains separate from the automated TestBackend checks and prior local PTY smoke test.

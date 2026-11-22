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
- Successfully invoked menu and palette commands are recorded by stable command ID in a small project-local history file. The palette puts the most recent commands first when opened with an empty query, and exact search ranking takes precedence while filtering. Removed command IDs and menu-opening commands are ignored when loading history.
- Menu rows fit the available terminal width using display-cell-aware, grapheme-safe truncation. Rows keep effective shortcuts visible, while the selected-command details area reports the full description and the reason an unavailable command is disabled.
- The full command view opens a task-group list before individual commands. Groups are derived from stable command IDs and menu categories (Projects & Books, Find & Replace, Manuscript Structure, Proofreading & Diagnostics, Compile & Export, Story Bible, Backup & Recovery, and related groups). Registry entries carry the resolved group, menu priority, and advanced classification. Group rows show the number of commands and how many are advanced. Enter opens a group; Left/Esc returns to groups; Shift+Tab returns to common actions. Selection and dispatch continue to use the original command IDs.
- Pull-down geometry now follows the actual rendered menu-bar labels, including effective shortcut labels, so a configurable root binding cannot shift the menu away from its category. At normal widths the dropdown is at most 48 cells wide and its height follows the visible item count; below 60 columns it uses the available width and stays at the left edge. This leaves more of the writing canvas visible while retaining the contextual description area.

The curated lists are code-owned presentation metadata, validated against stable command IDs and their actual menu categories. They currently prioritize project/series actions, editing and insertion, navigation, review, publishing, research/tools, and Help.

## Acceptance evidence

Automated tests verify each curated command exists in the registry and belongs to the intended category, every common menu has no more than seven entries, `Tab` exposes every category command through task groups without loss or duplication, and `Shift+Tab` restores the curated set. Nested menu tests open a group and return to the group list. Menu-row tests verify 36-cell layouts, visible effective shortcuts, and grapheme-safe truncation. Dropdown-geometry tests verify alignment against the rendered menu bar after key remapping, compact dimensions, narrow-terminal placement and resizing bounds. Command-palette tests cover exact and subsequence search by ID, alias and description; recency persistence and ranking; dispatch; argument prompts; Help preservation; default and custom keybindings; no-document entry; colon prose; and rendering from 20×6 through 142×60. Existing import, series, structural, and story-link menu tests remain applicable.

A local 80×24 Linux PTY smoke check opened File with F10 and closed it with Esc on a blank project. The test showed that first-run project creation remained visible and no phantom editor buffer appeared. The menu geometry was then reduced and is covered by automated viewport tests. This is not acceptance in the author's terminal emulator, SSH, or tmux.

## Remaining Issue #39 work

This is not the complete menu redesign. Task groups now provide a second level in the full command view, but groups are currently derived from IDs rather than individually curated registry metadata and some command labels remain developer-oriented. Frequency-based ranking, typo-tolerant edit-distance search, a complete command availability model, keymap-derived bindings for all menu operations, wireframes, monochrome/SSH/tmux acceptance, and hands-on usability review remain open. The broader alpha blockers in Issues #37 and #38 remain release gates even though their keyboard regression and quit-without-saving tests currently pass. Palette fuzzy matching currently uses compact alphanumeric subsequences, not edit-distance ranking. Do not close Issue #39 based on this increment.

Actual terminal-emulator, SSH, and tmux acceptance remains separate from the automated TestBackend checks and prior local PTY smoke test.

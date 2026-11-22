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

## Alpha blocker validation in a local Linux PTY

On 22 Nov 2026, a rebuilt terminal executable was exercised at 80×24 in a local PTY using a disposable project. The blank-project dashboard showed Start Writing and no editable, unattached manuscript buffer. Enter created a chapter and scene; text was saved and verified in the scene's Markdown file. F10 opened the File dropdown, Esc returned to the dashboard, and `q` exited cleanly when no edits were pending.

For the Issue #38 conflict path, the same scene was changed through the CLI while the terminal editor still held its earlier saved hash. The editor then held additional unsaved text. `q` displayed Save and Quit / Quit Without Saving / Cancel; `s` left the process in the quit dialogue after the save could not replace the externally changed file; `d` exited promptly. The scene file still contained the external version after exit. This directly exercises the reported liveness failure and the author's explicit discard decision in a running terminal process.

The follow-up UI test exposed that the save failure text was stored in the modal state but hidden by the dialog renderer. Modal footers now show the prompt, the save/recovery message, current focus, saved/unsaved state, and effective Save/Menu/Help shortcuts. A TestBackend regression test reproduces the stale-save conflict and checks that the failure and the `Quit Without Saving` choice are visible together while the editor remains dirty. This closes the reporting gap without changing the three-choice quit policy.

The keyboard audit also found that the conventional navigation key `q` was intercepted before the manuscript editor could receive it. It is now ordinary manuscript text while the Editor workspace has focus. `Esc` returns to Outline without discarding buffered edits; `q` from navigation opens the existing save/discard choices for dirty text and exits directly when clean. Context Help and command help now state this focus-dependent behavior. Regression coverage verifies that `q` edits a clean scene and that leaving to Outline still reaches the quit flow.

A rebuilt 80×24 local Linux PTY smoke check created a blank project, started the first scene, typed `q`, saved with Ctrl+S, then left the editor with Esc and quit from Outline. Reopening the Markdown file showed exactly `q`. This verifies real terminal delivery for this key path; it does not establish acceptance in the author's terminal emulator, SSH, tmux, or Linux TTY.

This check used a local PTY, not the author's terminal emulator, Linux TTY, SSH or tmux. It did not exercise read-only/full-disk recovery, SIGTERM, EOF, or every focus transition. Ctrl+/ delivery remains terminal-dependent; F10 was verified as the fallback. Issues #37 and #38 therefore remain open for their full cross-terminal acceptance even though the blank-buffer, F10, and conflict-discard paths passed locally.

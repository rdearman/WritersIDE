# Engineering Design v1.97 Addendum — Recent Project Picker

This addendum implements the Recent Projects action requested by GitHub Issue #39.

## Storage and behavior

- The curated File menu now contains New Project, Open Project, Recent Projects, Import Scrivener Book, Save and Quit. The importer stays visibly unavailable until a series is selected or the current book is already in a series; all import and series actions remain reachable in the complete grouped command menu and command palette.
- The recent-project list is stored outside novel projects at the user's `XDG_CONFIG_HOME/writers-ide/recent-projects.json`, falling back to `$HOME/.config/writers-ide/recent-projects.json`.
- The versioned JSON file stores at most twelve canonical project paths. Opening a project moves it to the front; duplicates are removed. Writes use the existing same-directory atomic JSON writer.
- The picker is keyboard accessible with Up/Down, Enter and Esc. Missing or relocated projects remain visible and are never opened. Selecting a project uses the normal project-switch operation, including unsaved-buffer and active-writing-challenge guards.
- If recent-history storage is unavailable or invalid, the project remains usable and the UI reports the issue. It does not modify project contents.

## Verification

Automated tests cover bounded ordering, deduplication, selection and reopening, history updates, missing-project handling, and the curated File menu contents. Terminal workspace verification is recorded with the implementation commit.

## Remaining Issue #39 work

This implements the File menu's Recent Projects action. A standalone first-launch project picker, menu-wide IA review, tutorial/help integration, and author validation in SSH/tmux and the author's terminal remain open.

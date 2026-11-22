# Engineering Design v1.95 Addendum — Create a Project from the Terminal

This addendum supplements [Engineering Design v1.92](engineering-design-v1.92.md) and the [v1.93 curated-menu](engineering-design-v1.93-addendum.md) and [v1.94 project-opening](engineering-design-v1.94-addendum.md) addenda. It records a task-first new-project path for GitHub Issue #39.

## Implemented behavior

- The curated File menu now starts with New Project, Open Project, Save, Quit, Import Scrivener Book, Create Series and Open Series Books.
- Series creation, selection and new-book actions remain reachable in the full File menu's Projects & Files group.
- New Project is command `file.new-project`, with configurable Ctrl+N as its default shortcut. Selecting it opens a guided form for a project directory and title.
- The operation refuses an existing destination, refuses to switch away from unsaved manuscript text or an active writing challenge, and cleans up a newly created destination if opening it fails.
- Successful creation opens the new project through the existing project-switch service, restoring the new project's session and keybindings.
- Project snapshots and history remain available in their existing task groups.

## Verification

Automated tests cover the Ctrl+N route, creating and opening a directory and title containing spaces and capitals, refusing an existing destination without changing it, and preserving access to all category commands through the nested full menu. The full terminal suite passed: 187 tests. Terminal Clippy and formatting checks passed. A local 80×24 PTY check entered Ctrl+N, typed a path and title, selected Start Writing, and verified a manuscript file in the new project after clean exit. SSH, tmux, and the author's terminal emulator remain unverified.

## Remaining Issue #39 work

This adds New Project to the existing in-project Dashboard and File menu. It does not provide a standalone first-launch project picker, project browser/recent-project list, complete curated-menu review, first-run tutorial, or author acceptance in SSH/tmux and the author's terminal emulator. Keep Issue #39 open.

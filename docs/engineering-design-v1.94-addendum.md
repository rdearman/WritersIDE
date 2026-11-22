# Engineering Design v1.94 Addendum — Direct Project Opening

This addendum supplements [Engineering Design v1.92](engineering-design-v1.92.md) and the [v1.93 curated-menu addendum](engineering-design-v1.93-addendum.md). It records the direct project-opening action added to the task-first File menu for GitHub Issue #39.

## Implemented behavior

- The File menu provides a registered `file.open-project` action, also available through the conventional, configurable Ctrl+O shortcut.
- Selecting it requests a project directory through the existing command prompt (`open-project PROJECT_DIRECTORY`). Paths containing spaces and uppercase characters are accepted by the prompt.
- The action uses the existing project-switch service, which validates the project through `WriterApp::open_project`, refuses to switch while manuscript text is dirty or a writing challenge is active, and restores the destination project's session and keybindings.
- The blank-project Dashboard's Open or Import action opens the File menu with Open Project selected. Import actions remain available in that menu.
- The common File menu stays within its seven-action limit; series selection remains available in the full File command list.

## Verification

Automated terminal tests cover opening a project from a path containing spaces, refusal to abandon a dirty buffer, the blank-project Dashboard route, and Ctrl+O opening the project-path prompt. The full terminal test suite passed: 183 tests, and terminal Clippy passed with warnings denied. A local 80×24 Linux PTY check typed a path containing spaces and capitals, opened the target project, switched to Outline to verify its chapter and scene, and exited with Ctrl+Q. SSH, tmux, and the author's terminal emulator remain unverified.

## Remaining Issue #39 work

This adds a basic project-open route; it does not provide a graphical/project-directory browser or finish the task-first redesign. The full issue still requires curated common actions with nested expert groups, first-run flow, truthful capability presentation, and user acceptance in real terminal, SSH and tmux environments. Do not close Issue #39 based on this increment.

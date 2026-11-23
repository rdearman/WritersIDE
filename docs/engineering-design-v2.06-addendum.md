# Engineering Design v2.06 Addendum — State-verified tutorial navigation

This addendum records the implementation response to GitHub Issue #59.

## Navigation lesson state

The first tutorial lesson remains backed by the real terminal application and its disposable sample project. Its navigation practice is now an ordered state machine persisted through the existing `TutorialProgress.verified_actions` set:

1. Open the menu.
2. Move the top-level menu selection to Navigate.
3. Highlight Outline.
4. Open Outline.
5. Open the sample scene.
6. Make a manuscript edit.
7. Save it through File > Save or the configured save shortcut.
8. Return to Outline.
9. Resume through Help > Resume Tutorial.

Each step is recorded only after the corresponding application state is reached. Menu steps check the active category and selected command; writing and saving check the editor's dirty state; return checks the resulting workspace. Actions out of order or from outside the disposable tutorial project do not advance progress. While the lesson panel has focus, unrelated keys and commands are rejected with a short hint.

The panel presents one current instruction and compact Back, Retry, Skip, Exit, Pause and Resume guidance. Resume reconstructs the menu selection or manuscript workspace needed for the next incomplete navigation step. It stores the sample scene's stable identifier so a resumed editing step targets the same document. Older tutorial preference files remain readable because the new identifier is optional.

## Verification and remaining acceptance

Regression tests exercise the navigation route through F10, arrow keys and Enter; reject wrong actions without leaving the current lesson context; save through the File menu; and resume through the Help menu. Existing tests continue to verify that sample content is isolated from the opened manuscript and that tutorial progress survives restart.

Automated terminal-buffer tests also cover contiguous manuscript/reference text at 201x60 and 80x24, view transitions, menu repaint, tutorial rendering and unchanged saved prose (Issue #56). Menu mnemonic and Ctrl+N tests cover New Project dispatch from the File menu (Issue #58).

These automated checks do not establish first-time usability. Issues #59, #56 and #58 remain open for the author's required human verification.

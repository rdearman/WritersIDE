# Writer's IDE — Engineering Design v0.10

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.9

## Terminal interaction checks

The TUI changes its split-pane direction at a width of 100 columns: narrower terminals use stacked panes; wider terminals use side-by-side panes. This keeps the scene/reference list and active content pane available in SSH, tmux, and narrow terminal windows.

The editor's save path uses the expected content hash. If a second writer changes a scene after it was opened, save reports a conflict, retains the user's dirty buffer, and does not silently replace the newer file. The header and footer expose the dirty state. Tests exercise stale-save conflict visibility and layout threshold behavior.

These tests are behavioral unit checks; terminal emulator rendering and real SSH/tmux sessions still require manual acceptance testing. The terminal remains a partial implementation of the full writing environment while outline editing, entity forms, clipboard/selection, and some workspace operations remain outstanding.

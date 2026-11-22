# Engineering Design v1.100 Addendum — Help access from quit dialogue

This addendum records a navigation acceptance increment for GitHub Issue #37.

The persistent F10 menu shortcut remains available while the dirty-quit modal
is open. From that modal, the author can open the Help menu and invoke
contextual Help. Closing Help returns to the same quit dialogue with its prompt
and unsaved state intact. This provides a discoverable keyboard route to Help
without assigning F1 away from its configured workspace action or intercepting
ordinary manuscript characters.

An automated terminal-state test exercises the complete F10 → Help menu →
context Help → Esc sequence from a dirty-quit dialogue. Related tests cover
Ctrl+/ menu access, ordinary slash entry, F10 fallback, modal state restoration,
blank-project behavior and terminal resizing. Existing local Linux PTY smoke
checks cover project creation, writing, saving, reopening, F10 menu access and
clean quit. A rebuilt 80×24 PTY run also exercised first-run welcome dismissal,
F10 navigation to Help, contextual Help, closing the panel, the updated Help
fallback footer, and clean exit. They do not replace testing in the author's
terminal emulator, SSH/tmux, or a Linux TTY. Issues #37–#39 remain open for
human alpha acceptance.

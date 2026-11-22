# Engineering Design v1.96 Addendum — Contextual Series Command Availability

This addendum records a menu-state correction related to the task-first workflows in GitHub Issue #39.

## Implemented behavior

- Series actions that require a series (`Add Current Book`, `Import Scrivener Book`, `New Book in Series`, and `Read Series References`) are unavailable until a series is selected or the current project is already registered in one.
- Opening an existing series book discovers its series context at application startup, so the same actions remain available after a direct project reopen.
- Attempting an unavailable menu action explains the missing prerequisite and leaves the menu open for the next choice.
- Creating or selecting a series remains available without an existing series context.

## Verification

The regression test verifies each unavailable action, the enabled series-creation action, and menu retention after attempting `New Book in Series`. The full terminal test suite passed: 189 tests. Terminal Clippy and formatting checks passed.

This is a focused interaction correction. The remaining first-launch and broader Issue #39 requirements remain open.

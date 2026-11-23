# Engineering Design v2.04 Addendum — Tutorial continuation action

This addendum records the Issue #41 continuation affordance update.

- A completed first lesson displays a highlighted `Continue to Lesson 2 · press Enter` action.
- The separate Escape instruction describes returning to navigation.
- Enter remains the only progression key shown; tutorial focus consumes F6 so the Characters workspace binding cannot intercept tutorial progression.
- Progress remains gated on the seven verified navigation and writing actions, and is stored before advancing.

Automated tests cover the visible action, F6 focus priority, persisted step advancement, and compact and wide terminal rendering. Human UAT is still required before closing Issue #41.

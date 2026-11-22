# Engineering Design v1.99 Addendum — Conflict-aware quit recovery

This addendum records a focused Issue #38 improvement to the terminal quit
flow.

When saving from the dirty-project quit dialogue fails, the application keeps
the author in an explicit quit state and presents short, visible choices to
retry the save, review the conflicting versions, write a separate recovery
copy, quit without saving, or return to editing. The explanatory message names
the project and document, reports whether the in-memory text is unsaved, and
includes the on-disk modification time when available.

Saving a recovery copy writes the in-memory editor text to a new UUID-named
Markdown file under the project recovery directory using a staged, synced
write. It does not replace the canonical manuscript and does not clear the
editor's dirty state. Explicit quit without saving leaves the current on-disk
manuscript unchanged. Conflict review enters the existing version-resolution
flow; the author can return to editing or explicitly choose a resolution.

Tests exercise a concurrent on-disk edit, failed Save and Quit, recovery-copy
creation, preservation of both divergent versions, and explicit discard and
quit. Existing conflict-resolution and quit tests continue to cover recovery
write failure and normal dirty-quit behavior. This change improves the exit
route but does not complete every Issue #38 acceptance case, including
first-run usability validation across real terminals and process signals.

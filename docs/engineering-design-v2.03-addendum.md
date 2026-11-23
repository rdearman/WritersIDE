# Engineering Design v2.03 Addendum — Author-facing command names

This addendum records the Issue #45 terminology cleanup.

- Command discovery views display command labels, shortcuts, and availability; internal command IDs remain implementation details.
- The command palette describes searchable actions rather than a registry and accepts visible action names when preparing a command.
- The keybinding manager displays author-facing command labels and binding sequences without exposing registry identifiers.
- Internal IDs remain valid for dispatch, persistence, and advanced filtering; this change does not alter command identity or configurable bindings.

Rendered-screen regression tests verify that command palette and keybinding manager views omit implementation IDs. Human review is still needed to confirm the wording is understandable in normal use.

# Writer's IDE — Engineering Design v0.10

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.9

## Author-managed event dependency assertions

The existing `story_event_dependency` relation stores explicit `precedes` and `requires` assertions between stable event IDs. Authors can now add, list, update the explanatory note on, remove, and evaluate these assertions through the application API, CLI, and terminal command registry. Event ordering and prerequisite evaluation continue to use the uncertainty-aware chronology inspector; missing or indeterminate dates produce `UNKNOWN` rather than an inferred ordering. SQLite remains responsible for rejecting invalid event references, self-links, and dependency cycles.

The CLI commands are `event-dependency-add`, `event-dependency-list`, `event-dependency-update`, `event-dependency-remove`, and `event-dependency-check`. The terminal exposes the same operations through the shared command registry. Editing a note does not change the dependency edge; removing an assertion requires its predecessor, successor, and kind. The persistence test verifies that removing one edge leaves unrelated edges intact and that the final deletion persists.

This is a concrete author-editable deterministic assertion family, not a general-purpose assertion language. General query-cardinality assertions, saved query expressions, coverage requirements, and location incompatibility rules remain separate typed mechanisms. A generic configurable watchpoint/assertion model and general full-text/title/synopsis passage source maps remain open Issue #15 work.

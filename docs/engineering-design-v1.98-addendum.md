# Engineering Design v1.98 Addendum — Onboarding State Foundation

This addendum records the first implementation foundation for GitHub Issue #36.

## Portable preference model

The terminal crate now has a versioned onboarding preference model that is
independent of a novel project database. It tracks whether the welcome has been
seen, whether the tutorial was dismissed or completed, and resumable progress
including the tutorial track, step, disposable demonstration-project path and
return-project path. Unsupported versions are rejected instead of being
silently rewritten.

Preference writes stage a file beside the destination, sync its contents,
atomically persist it, then sync the containing directory. This follows the
existing user-configuration pattern and does not touch manuscript files.

## Verification

Tests cover first-launch defaults, round-trip persistence of active progress,
and unsupported-version rejection. Formatting, terminal-crate Clippy and the
three focused onboarding tests pass.

## Remaining Issue #36 work

This is storage groundwork only. First-run presentation, Guided Tour and Quick
Start execution, safe demo-project lifecycle, interactive lessons, context help
navigation, searchable offline documentation, CLI help, configurable help
binding, and terminal acceptance testing remain unimplemented. Integration
must account for current focus, conflict and menu blockers tracked in #37–#39.

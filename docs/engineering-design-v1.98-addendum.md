# Engineering Design v1.98 Addendum — Onboarding State Foundation

This addendum records the onboarding implementation foundation for GitHub Issue #36.

## Portable preference model

The terminal crate now has a versioned onboarding preference model that is
independent of a novel project database. It tracks whether the welcome has been
seen, whether the tutorial was dismissed or completed, and resumable progress
including the tutorial track, step, disposable demonstration-project path and
return-project path. Unsupported versions are rejected instead of being
silently rewritten.

On first interactive startup, the terminal presents Guided Tour, Quick Start
and Skip. Active progress adds Resume Tutorial. The Dashboard also exposes a
start/resume entry. Tutorials create or reopen a dedicated sample project under
the user's application data directory; the original project is retained as the
return destination and the sample is omitted from Recent Projects. The tutorial
panel can be paused, resumed, advanced or exited back to the original project.
Completion returns to the original project. The sample directory is not
automatically deleted. Help includes registered Show Welcome, Resume Tutorial,
Restart Tutorial and Search Help and Commands actions. Search Help opens the
offline command palette. The default `help.context` binding is empty so F1
continues to navigate to its configured workspace; authors can assign Help
through the existing command keymap and can use the Help menu at any time.
An explicit user binding such as F1 → `help.context` opens and closes the
context panel; the default F1 binding still navigates to its configured
workspace.

Preference writes stage a file beside the destination, sync its contents,
atomically persist it, then sync the containing directory. This follows the
existing user-configuration pattern and does not touch manuscript files.

## Verification

Tests cover first-launch defaults, round-trip persistence of active progress,
unsupported-version rejection, welcome choices, sample-project creation,
resumable track metadata, preservation of the original manuscript text, and
ordinary `q` text entry with the tutorial panel open, and F1 remaining a
workspace shortcut while Help is reachable through registered actions.
Formatting, terminal-crate Clippy and the terminal test suite pass.
An 80×24 PTY run exercised first-run Guided Tour selection, creation of the
isolated sample, return to the original project, quit/relaunch, and resume of
the saved tutorial step. Both projects lived in an auto-cleaned temporary
directory; the source book remained unchanged.

## Remaining Issue #36 work

The implemented tutorial route currently provides step text and navigation but
does not yet verify that the author performed each lesson action. Import,
reference-pane, search/replace, proofread, compile-result source navigation and
EPUB steps need end-to-end lesson verification. Context-sensitive help
navigation, searchable offline documentation beyond the command registry, CLI
command help, detailed configurable help-binding guidance, resize and
interrupted-session tests, and broader real-terminal onboarding validation
remain open.
Issue #36 must be coordinated with the focus, conflict and task-first menu
blockers tracked in #37–#39; this addendum does not claim those blockers are
closed.

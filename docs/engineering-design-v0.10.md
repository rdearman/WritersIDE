# Writer's IDE — Engineering Design v0.10

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.9

## Terminal interaction checks

The TUI changes its split-pane direction at a width of 100 columns: narrower terminals use stacked panes; wider terminals use side-by-side panes. This keeps the scene/reference list and active content pane available in SSH, tmux, and narrow terminal windows.

The editor's save path uses the expected content hash. If a second writer changes a scene after it was opened, save reports a conflict, retains the user's dirty buffer, and does not silently replace the newer file. The header and footer expose the dirty state. Tests exercise stale-save conflict visibility and layout threshold behavior.

These tests are behavioral unit checks; terminal emulator rendering and real SSH/tmux sessions still require manual acceptance testing. The terminal remains a partial implementation of the full writing environment while outline editing, entity forms, clipboard/selection, and some workspace operations remain outstanding.

## Issue #5 — Custom writing goals and event ledger

Migration 0024 adds project and scene-scoped writing goals, an append-oriented activity ledger, and daily snapshots. Goal dates are inclusive `YYYY-MM-DD` values. The target is the intended total word count at the end date; `baseline_words` is the starting manuscript count. The target trajectory interpolates from baseline to target over all inclusive days. Real progress is baseline plus the signed sum of real deltas; raw progress is baseline plus the non-negative sum of reported raw deltas. The selected `success_metric` names the series to compare with the target. Daily values are persisted as snapshots and can be reconstructed from current ledger events after a correction.

Every event has a stable UUID, origin, source identifier and fingerprint, timestamp, optional scene, raw and real deltas, reconciliation state and explanatory detail. A repeated goal/origin/source with the same fingerprint returns the original event ID; a changed fingerprint is rejected. `goal-reconcile` performs an optimistic fingerprint check, marks the original event superseded and appends a traceable correction in one transaction. Editor writing-session completion adds matching events. Isolated Write-or-Die drafts add their retained-plus-deleted count to raw activity and zero real manuscript words; explicitly saving the draft to canonical prose is a separate action. Imports and manually reported activity can use the same event API/`goal-record` command.

The CLI and terminal command surface includes `goal-create`, `goal-list`, `goal-record`, `goal-events`, `goal-progress`, `goal-weekly`, `goal-archive` and `goal-reconcile`. The Analytics workspace includes a target/actual daily comparison for each active goal. Weekly summaries are available through `goal-weekly`. Progress projections are derived from the ledger, and no analytics operation mutates goal or manuscript state.

The initial time-zone implementation accepts UTC and fixed offsets only. IANA zone databases and daylight-saving transitions are deferred; a fixed offset is consequently required for stable historical day boundaries. Raw word activity uses available source measurements: completed editor sessions report positive net session deltas as raw activity (the editor does not count every keystroke); timed isolated drafts count retained plus deleted words. Import adapters must explicitly state their count semantics. Migration and storage tests cover goal constraints, date/window validation, event idempotency/fingerprint conflict, scene scope, baseline projection, optimistic reconciliation, persistence, completed session attribution and Write-or-Die isolation.

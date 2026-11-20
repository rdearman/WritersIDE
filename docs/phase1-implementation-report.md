# Writer's IDE — Phase 1 Implementation Report

**Status:** Phase 1 core vertical slice implemented  
**Baseline:** SRS v0.1 supplied in the project brief; Engineering Design v0.1 preserved; approved decisions incorporated in Engineering Design v0.2.

## Delivered

- Updated the design in [engineering-design-v0.2.md](engineering-design-v0.2.md); retained [engineering-design-v0.1.md](engineering-design-v0.1.md) unchanged as the review baseline.
- Added a compilable Rust workspace with `story-core`, `story-storage`, `story-app`, `story-import`, and the `writers-ide` CLI.
- Added the numbered schema migration at `crates/story-storage/migrations/0001_initial.sql` and transactional migration application.
- Implemented the project/scene/structural domain types, typed IDs, `StoryTime`, relationship types, word-attribution policies and custom-field validation.
- Implemented one SQLite database per project, immutable project identity, relative Markdown paths, root and sibling ordering constraints, chapter-only placement triggers, hierarchy cycle checks, and transactional scene/structural movement.
- Implemented SHA-256 manuscript persistence with a synced temp file, synced recovery candidate, durable journal, optimistic hash checks, startup recovery, conflict preservation, explicit `external`/`prepared` conflict choices and path/symlink validation.
- Added typed custom-field persistence and equality filters, character/plotline/event relationships, project metadata and fictional epoch storage, provenance records, synthetic import inventory/mapping interfaces, and consistent project backup via SQLite's online backup API.
- Added a small CLI for project creation/inspection, chapter and scene creation, scene inspection/writing, conflict resolution and backups.

## Architectural decisions applied

All eight approved decisions are represented in the schema, core or storage contract: single-project database ownership; integer ordering with a root partial unique index and two-range reorder staging; durable manuscript journaling and conflict recovery; signed 64-bit story ticks; Rust and SQL placement validation; basis-point AuthorShares validation and deterministic remainder allocation; typed core fields with typed custom metadata; and Phase 1 import provenance.

The prepared manuscript candidate remains on disk while a write journal is pending. Recovery advances a save only when hashes make the outcome clear. When a divergent file is found, both the external version and prepared version are retained under `.writers-ide/recovery/`; the project remains visibly conflicted until an explicit choice is made.

## Verification results

All requested verification gates passed:

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | Passed: 25 unit tests, 0 failures |
| Fresh database migration tests | Passed as part of project creation and storage tests |
| Write recovery fault-injection tests | Passed at partial temp, synced temp, journal intent, pre/post replacement, post-metadata commit and cleanup boundaries |
| Project relocation | Passed in storage and application vertical-slice tests |
| Direct SQL placement and structural constraints | Passed for non-chapter placements, root duplicates, chapter retyping/deletion and hierarchy cycles |
| Network independence of tests/runtime | Tests contain no network calls; runtime code has no network dependency |

The recovery tests inject interruption returns at protocol boundaries and reopen the project to exercise recovery. They do not simulate power loss or terminate a separate OS process at arbitrary CPU instructions.

## Requirement status

| Area | Status | Notes |
|---|---|---|
| Project create/open/identity/relocation | Implemented | A newer schema is rejected; project creation refuses a non-empty destination. |
| Structural nodes and scenes | Implemented | Create, get, metadata update, archive, reorder and move operations are available through application services. |
| Portable manuscript persistence | Implemented | UTF-8 Markdown scene files, SHA-256 checks, journal recovery and explicit conflict resolution. |
| Relationships | Implemented foundation | Scene-character, scene-plotline and scene-event joins support multiple links and participation roles. |
| Chronology | Implemented foundation | Integer-second ticks, epoch/calendar labels, uncertainty bounds, checked duration and ambiguous comparison. |
| Word attribution | Implemented in core | Overlap, EqualShare and AuthorShares calculation; reports/plot-grid UI remain later work. |
| Custom scene metadata | Implemented foundation | Typed values, defaults, required-field validation, enums, entity references and equality filtering. |
| Import provenance | Implemented foundation | Runs, source items, field-level provenance, fingerprint repeat checks and synthetic inventory/mapping tests. |
| Backup | Implemented foundation | Online SQLite snapshot plus copied/validated project files. |
| Terminal editor and full-screen TUI | Deferred | Phase 2; no terminal UI work was started. |
| Full Scrivener importer | Deferred | Requires format investigation and a representative project. |
| Ranperen XLSX/ODS/CSV adapters | Deferred | Requires the source workbooks and sheet-level mapping investigation. |
| Analytics, outliner, plotting grid, FTS, research/issues, continuity | Deferred | Later SRS phases. |
| Real importer compatibility | Blocked by missing fixtures | No Ranperen workbooks or Scrivener project are present in this repository. No compatibility claim is made. |

The SRS was provided in the task text, not as a repository file. The design records that fact; no replacement SRS file was fabricated.

## Deviations and known limitations

- The Phase 1 importer is a generic inventory/mapping foundation tested with synthetic records. It does not parse Scrivener packages or spreadsheets.
- The database currently has migration 0001 only. Opening a newer schema is rejected; schema upgrade migrations will be added with later releases.
- The core has typed character, plotline, location and event models, while Phase 1 persistence stores only fields needed for current relationships and chronology. Full SRS character/location/worldbuilding records remain deferred.
- Custom entity references currently support character, location, plotline, event and scene targets. Custom-field equality filtering exists; general query language and FTS remain future work.
- Directory synchronization errors fail the save operation. There is no degraded-durability warning mode for filesystems that do not support directory `fsync`.
- Hash comparison is repeated immediately before replacement, but an unrelated external process can theoretically modify a file in the tiny interval between that final check and the atomic rename. The implementation has no cross-process lock protocol for arbitrary external editors.
- Conflict handling is exposed through CLI commands rather than a graphical reconciliation UI. `writers-ide inspect <path>` lists unresolved conflict IDs; `writers-ide resolve <path> <transaction-uuid> external|prepared` applies an explicit choice.
- Backups refuse a project whose stored manuscript hashes do not match its files; resolve external edits before creating a consistent backup.
- Performance targets, crash behavior on network filesystems, and compatibility with non-Linux platforms have not been measured.

## Recommended next step

Obtain and inventory one representative Scrivener project and the three Ranperen workbooks. Use those real records to validate the current entity model and provenance mapping before adding source-specific importer adapters or starting the Phase 2 terminal interface.

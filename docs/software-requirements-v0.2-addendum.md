# Writer's IDE — SRS v0.2 Addendum

**Status:** Approved scope for GitHub issue #1 and its authorized implementation follow-on  
**Baseline:** SRS v0.1 (provided in the project brief; source file was not present in the repository)

This addendum extends SRS v0.1 without replacing its scene-centred, local-first scope or the eight approved Phase 1 invariants. It records the issue's new requirements and the user's subsequent authorization to implement the feature slices. The v0.1 SRS should be checked in when its source document is available; this addendum does not claim to reproduce it.

## Requirement register and traceability

| ID | Requirement | Relationship to SRS v0.1 | Allocation | Current state |
|---|---|---|---|---|
| HYP-001 | Typed links use stable entity IDs across scenes, characters, plotlines and research; backlinks resolve after title/placement changes. | New | Domain/storage/API/CLI/TUI | Implemented with outgoing/backlink CLI and terminal commands; stable IDs survive rename/move |
| HYP-002 | Passage anchors retain stable IDs, selected text, byte range, surrounding context and source hash. | Strengthened | Domain/storage; editor integration | Implemented: save/recovery revalidates exact quote and context; unique matches move automatically; missing/ambiguous quotes are flagged; author can inspect and explicitly reattach through application, CLI, or TUI |
| HYP-003 | Broken endpoints and anchors are reportable; invalid endpoints are rejected at write time. | New | Storage/diagnostics | Implemented for persisted links |
| HYP-004 | Include/transclude links are navigable references, do not copy canonical prose, and cannot form cycles. | New | Domain/storage/reader/compiler | Cycle guard, terminal preview, and opt-in derived compilation implemented; general document inclusion remains deferred |
| QRY-001 | Named typed scene queries combine status, character, plotline, chronology and unresolved-issue filters. | Strengthened | Domain/storage/CLI/TUI | Save, list, and dynamic run exposed in CLI and terminal as a live virtual collection; current metadata changes refresh results without changing manuscript order |
| QRY-002 | Query results refresh from current story data and never change manuscript order. | New | Application/analytics | Implemented and tested at storage and terminal view boundaries; terminal preserves the full scene cache and maps navigation by stable scene ID |
| IDX-001 | Full-text and relationship indexes support search while preserving stable passage references. | Strengthened | Storage/search/CLI/TUI | Implemented project-wide rebuildable FTS5 index across scenes, custom scene fields, characters, locations, plotlines, events, research notes, issues and reading documents; typed hits navigate from terminal; richer query syntax remains |
| TUI-001 | Full-screen keyboard-first Ratatui/Crossterm workspaces include all SRS workspaces, help, bindings, palette, history, split panes and conflict state. | Existing + strengthened | Terminal application | Partial: configurable F-keys; scene/section/chapter and basic entity creation; typed character/event/issue/plotline linking, issue status transitions and core scene purpose fields; validated basis-point allocation; multiline editing, selection, clipboard and conflict indicators; live plot and character matrices; entity-aware project search routing and record inspection are implemented for characters, locations, plotlines, events, issues, and research; richer workspace-specific editing forms and OS clipboard integration remain |
| TUI-002 | Central typed command registry and dispatcher provide stable command IDs and shared keyboard/palette/menu actions. | New (Issue #8) | Terminal command architecture | Foundation implemented for workspace, save, help, and Compile actions; registry owns Compile palette aliases and argument contracts, while structural/entity/reading/link/extraction handlers remain on the legacy parameterized parser for incremental migration |
| TUI-003 | Ctrl+/ main-menu shortcut with F10 fallback; ordinary slash remains manuscript input. | New (Issue #8) | Terminal key handling/menu | Shortcuts reserved and routed to one registered action; visual hierarchy, configurable chord bindings, capability negotiation and menu navigation deferred |
| READ-001 | Read text/Markdown/EPUB references with persistent highlights, annotations, checkpoints and indexes without modifying source files. | New | Reading workspace | Text/Markdown/EPUB import, persistent highlights/notes/bookmarks/checkpoints/terms, linked entities, global search and direct annotation/bookmark jumps and in-place current highlight/note styling are implemented; synthetic EPUB is tested, real-source compatibility and richer reading interactions remain |
| LLM-001 | Optional local Ollama extraction is provisional, passage-linked, provenance-bearing and never silently promotes facts or edits prose. | Existing + strengthened | Optional integration | Partial: CLI/TUI localhost extraction accepts scene or reading sources and stores provisional hashed quote-anchored proposals; explicit per-claim approval revalidates source fingerprint and evidence before creating a searchable canonical knowledge record with source/model provenance; offline mock verifies the HTTP contract; rich proposal management and live Ollama compatibility remain unverified |
| REV-001 | Snapshots, diffs, selective restore and history preserve durable journal behavior and Git compatibility. | Strengthened | Storage/revision UI | Implemented foundation: verified snapshots, line diffs, range restore through the journal, CLI and TUI history view; visual hunk selection remains |
| PIPE-001 | Dependency-aware processing runs compilation, analyses, diagnostics, export and reports with visible failures and practical invalidation. | New | Processing service | Partial: local compiler, word count, plot/character analysis, classified character chronology diagnostics (definite/possible/unverified with stable character IDs), export, revision summary, verified artifact cache, transitive dependency selection, explicit NotSelected/Skipped/Failed reports, and CLI/TUI stage selection implemented; wider continuity constraints and richer processing configuration remain |
| ANA-001 | Character heatmaps/interactions, subplot gaps, budget previews and uncertainty-aware chronology reports are read-only projections. | Existing + strengthened | Analysis/TUI | Partial: read-only DTOs, full terminal heatmap rows, named co-appearance pairs, boundary-labelled subplot gaps, deterministic per-scene budget previews, and same-character chronology findings distinguishing definite overlap, bounded possible overlap, unknown/unverified time, and exact adjacency implemented; the scene timeline orders by story time and labels uncertainty; event timeline rows retain typed uncertainty and original time labels; interactive event interval editing, graphical heatmaps, richer policy controls and other visualization modes remain |

## Domain and storage contracts

- A link is directed and typed. Its endpoints are `(kind, stable UUID)` pairs; titles and paths are display data only. Backlinks query the reverse index over the same canonical link row.
- Passage anchors belong to a scene and record UTF-8 byte offsets, exact quote, context, content hash and an explicit state. An edit must not silently retarget an anchor; until re-anchoring is implemented, stale anchors require review.
- Transclusion and inclusion are distinct link kinds. They never duplicate source text. The initial database rejects cycles among include/transclude edges. Rendering and manuscript export must state whether referenced content is expanded.
- Saved query filters are typed records, combined with AND semantics in filter order. Query evaluation is a read-only view ordered by current manuscript placement; absent placement sorts after placed scenes. Saving a query or refreshing results never persists a new placement.
- Full-text indexes are rebuildable caches. SQLite rows/files remain authoritative.
- Reading annotations refer to immutable source fingerprints plus stable passage anchors. When source bytes change, annotations become unresolved/reviewable rather than being applied to changed text.
- LLM output is an untrusted proposal with source evidence and provenance. Approval is an explicit application operation; inferred facts remain separate from author-defined records.
- Pipeline stages declare inputs, outputs and dependencies. A failed or skipped stage is visible and cannot be reported as a successful complete manuscript build.

## Phase allocation

| Slice | Requirements |
|---|---|
| B1: hypertext and saved-query foundation | HYP-001–004, QRY-001–002, initial IDX-001 |
| B2: terminal application and editing | TUI-001, query/link access, search integration |
| B3: reading and annotation | READ-001, anchor revalidation and source indexing |
| B4: revisions | REV-001 |
| B5: analysis and processing | ANA-001, PIPE-001 |
| B6: optional local extraction | LLM-001 |

The user authorized implementation beyond the original issue's documentation-only recommendation. Each slice must still have working behavior, tests and a commit; later slices are not complete merely because their architecture is documented.

### Reading rendering milestone

The Reading workspace visually marks current highlights (yellow background) and notes (cyan underline) directly over extracted text. Rendering is a projection: it never changes the source document, and annotations with stale or invalid anchors are not painted. The terminal regression test checks the styled span and direct passage navigation.


### Analytics workspace milestone

The terminal Analytics workspace now presents the complete scene heatmap, named character interaction counts, subplot gap locations, uncertain chronology pairs, and budget redistribution suggestions. All output is a read-only projection; budget suggestions are marked as previews and never alter scene targets. A regression test exercises these report sections and verifies no target mutation.


### Chronology timeline milestone

The terminal Timeline workspace now orders scenes by story-time values, displays start/end ranges for approximate and bounded values, places unknown dates last, and opens the correct scene through its stable ID. Workspace list navigation now follows the actual visible rows, including expanded analytics reports. Event-level timeline interaction and visual overlap grouping remain incomplete.


### Story-event timeline milestone

Scene and typed event records appear together in the terminal Timeline workspace, sorted by canonical time for display with unknown intervals last. Event start/end uncertainty and original source-time text remain visible. The `event-time` command creates exact, bounded, approximate, or unknown event times. Selecting an event navigates to its Story Bible row. Mixed timeline ordering and persistence are regression-tested.


### Saved-query terminal collection milestone

Saved queries now run as a live virtual collection in the terminal Search workspace. Results refresh from current story data, retain manuscript order, and do not replace the application's full scene listing. Enter opens the selected matching scene by stable ID; `query-clear` returns to the complete scene list. Regression coverage checks dynamic membership after metadata changes and verifies scene order and identity are preserved.


### Story entity navigation milestone

Project-wide search and reading links now navigate characters, locations, plotlines, events, and issues to the correct record row. Enter inspects the selected record rather than opening an unrelated scene. The Story Bible supports creating and listing locations, and entity creation selects the new record. Terminal tests cover each search destination and prevent scene-index confusion. Full character/worldbuilding editing forms remain incomplete.


### Transclusion-aware compilation milestone

The incremental processing pipeline now supports opt-in scene transclusion expansion. CLI `process <project> --expand-transclusions` and terminal `:process-expand` generate a derived compile artifact while leaving canonical manuscript files untouched. The compile cache fingerprints the selected mode. Integration tests verify expansion and unchanged source text.


### Issue #8 command registry and shortcut foundation

A typed terminal registry now provides stable command IDs and metadata for workspace navigation, save, help and Compile actions. Keyboard workspace dispatch and exact command-palette entries use the shared typed dispatcher. Ctrl+/ (plus Ctrl+_ where emitted by terminal protocols) and F10 route to the same reserved main-menu action. F10 has been removed from the default Research workspace binding; Research remains accessible through Tab/BackTab. Ordinary `/` inserts into the manuscript editor. The current action reports that hierarchical menu rendering is pending and preserves editor state.

Deferred to the Issue #8 menu milestone: hierarchical popup UI, menu navigation and mnemonics, disabled actions, full registration of parameterized commands, configurable menu chords and conflict reporting, and terminal enhanced-keyboard capability negotiation. Ctrl+/ remains terminal-dependent; F10 is the fallback.


### Evidence-checked extracted-knowledge approval milestone

Approved knowledge claims now persist in schema migration 0007 with candidate kind/name/statement, optional story-time text, stable source identity, source fingerprint/length, exact quoted passage and byte offsets, model, and approval time. Promotion rejects stale sources and invalid evidence offsets, is idempotent for a claim already promoted from the same evidence, and updates the rebuildable FTS index. The CLI `approve-extraction <project> <proposal-json-file> <claim-index>` and terminal `approve <proposal-json-path> <claim-index>` promote one claim after explicit author action. The `knowledge` CLI command, Story Bible rows, and project search expose approved records and their evidence. Tests cover scene/reading references, source immutability, staleness, persistence, idempotency, indexing, and terminal navigation. Proposal editing/rejection/retraction and promotion into entity-specific character/event records remain deferred.


### Selective dependency-aware processing milestone

The pipeline now exposes `run_selected`, adding transitive dependencies for requested stages and reporting unrequested stages as `NotSelected`. Failed dependencies still mark dependent stages `Skipped`; failures remain visible. Stable stage names support CLI `process --only` and terminal `process-only`/`process-expand-only`. The full `process` command remains unchanged. Tests cover Export's Compile dependency, cache reuse, excluded stages, dependency failure, stable stage names, CLI validation and terminal invocation. Declarative profiles and richer build configurations remain deferred.

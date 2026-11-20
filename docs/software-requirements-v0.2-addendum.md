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
| HYP-004 | Include/transclude links are navigable references, do not copy canonical prose, and cannot form cycles. | New | Domain/storage/reader/compiler | Cycle guard and terminal transclusion preview implemented; compile option remains deferred |
| QRY-001 | Named typed scene queries combine status, character, plotline, chronology and unresolved-issue filters. | Strengthened | Domain/storage/CLI/TUI | Save, list, and dynamic run exposed in CLI and terminal command palette |
| QRY-002 | Query results refresh from current story data and never change manuscript order. | New | Application/analytics | Implemented and tested at storage query boundary |
| IDX-001 | Full-text and relationship indexes support search while preserving stable passage references. | Strengthened | Storage/search/CLI/TUI | Implemented project-wide rebuildable FTS5 index across scenes, custom scene fields, characters, locations, plotlines, events, research notes, issues and reading documents; typed hits navigate from terminal; richer query syntax remains |
| TUI-001 | Full-screen keyboard-first Ratatui/Crossterm workspaces include all SRS workspaces, help, bindings, palette, history, split panes and conflict state. | Existing + strengthened | Terminal application | Partial: configurable F-keys; scene/section/chapter and basic entity creation; typed character/event/issue/plotline linking, issue status transitions and core scene purpose fields; validated basis-point allocation; multiline editing, selection, clipboard and conflict indicators; live plot and character matrices; richer workspace-specific actions and OS clipboard integration remain |
| READ-001 | Read text/Markdown/EPUB references with persistent highlights, annotations, checkpoints and indexes without modifying source files. | New | Reading workspace | Text/Markdown/EPUB import, persistent highlights/notes/bookmarks/checkpoints/terms, linked entities, global search and direct annotation/bookmark jumps and in-place current highlight/note styling are implemented; synthetic EPUB is tested, real-source compatibility and richer reading interactions remain |
| LLM-001 | Optional local Ollama extraction is provisional, passage-linked, provenance-bearing and never silently promotes facts or edits prose. | Existing + strengthened | Optional integration | Partial: CLI/TUI localhost extraction accepts scene or reading sources, stores hashed quote-anchored proposals and has explicit review flag; offline mock verifies real HTTP request/response contract; canonical promotion and rich proposal management remain, and live Ollama compatibility is unverified |
| REV-001 | Snapshots, diffs, selective restore and history preserve durable journal behavior and Git compatibility. | Strengthened | Storage/revision UI | Implemented foundation: verified snapshots, line diffs, range restore through the journal, CLI and TUI history view; visual hunk selection remains |
| PIPE-001 | Dependency-aware processing runs compilation, analyses, diagnostics, export and reports with visible failures and practical invalidation. | New | Processing service | Partial: local compiler, word count, plot/character/chronology analysis, export, revision summary, verified artifact cache and CLI/TUI invocation implemented; richer diagnostics/configuration remain |
| ANA-001 | Character heatmaps/interactions, subplot gaps, budget previews and uncertainty-aware chronology reports are read-only projections. | Existing + strengthened | Analysis/TUI | Partial: read-only DTOs, full terminal heatmap rows, named co-appearance pairs, boundary-labelled subplot gaps, uncertainty-aware chronology pairs and deterministic per-scene budget previews implemented; graphical heatmaps, richer policy controls and other visualization modes remain |

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

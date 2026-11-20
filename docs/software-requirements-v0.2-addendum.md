# Writer's IDE — SRS v0.2 Addendum

**Status:** Approved scope for GitHub issue #1 and its authorized implementation follow-on  
**Baseline:** SRS v0.1 (provided in the project brief; source file was not present in the repository)

This addendum extends SRS v0.1 without replacing its scene-centred, local-first scope or the eight approved Phase 1 invariants. It records the issue's new requirements and the user's subsequent authorization to implement the feature slices. The v0.1 SRS should be checked in when its source document is available; this addendum does not claim to reproduce it.

## Requirement register and traceability

| ID | Requirement | Relationship to SRS v0.1 | Allocation | Current state |
|---|---|---|---|---|
| HYP-001 | Typed links use stable entity IDs across scenes, characters, plotlines and research; backlinks resolve after title/placement changes. | New | Domain/storage/API | Implemented in migration 0002 and services |
| HYP-002 | Passage anchors retain stable IDs, selected text, byte range, surrounding context and source hash. | Strengthened | Domain/storage; re-anchoring follows editor integration | Initial anchor creation implemented; edit re-anchoring deferred |
| HYP-003 | Broken endpoints and anchors are reportable; invalid endpoints are rejected at write time. | New | Storage/diagnostics | Implemented for persisted links |
| HYP-004 | Include/transclude links are navigable references, do not copy canonical prose, and cannot form cycles. | New | Domain/storage/reader/compiler | Cycle guard implemented; rendering/compiler semantics deferred |
| QRY-001 | Named typed scene queries combine status, character, plotline, chronology and unresolved-issue filters. | Strengthened | Domain/storage/CLI/TUI | Persistence/evaluation implemented; CLI/TUI exposure deferred |
| QRY-002 | Query results refresh from current story data and never change manuscript order. | New | Application/analytics | Implemented and tested at storage query boundary |
| IDX-001 | Full-text and relationship indexes support search while preserving stable passage references. | Strengthened | Storage/search | Initial FTS5 table; indexing/search API deferred |
| TUI-001 | Full-screen keyboard-first Ratatui/Crossterm workspaces include all SRS workspaces, help, bindings, palette, history, split panes and conflict state. | Existing + strengthened | Terminal application | Deferred; no TUI dependencies currently available locally |
| READ-001 | Read text/Markdown/EPUB references with persistent highlights, annotations, checkpoints and indexes without modifying source files. | New | Reading workspace | Deferred |
| LLM-001 | Optional local Ollama extraction is provisional, passage-linked, provenance-bearing and never silently promotes facts or edits prose. | Existing + strengthened | Optional integration | Deferred |
| REV-001 | Snapshots, diffs, selective restore and history preserve durable journal behavior and Git compatibility. | Strengthened | Storage/revision UI | Deferred |
| PIPE-001 | Dependency-aware processing runs compilation, analyses, diagnostics, export and reports with visible failures and practical invalidation. | New | Processing service | Deferred |
| ANA-001 | Character heatmaps/interactions, subplot gaps, budget previews and uncertainty-aware chronology reports are read-only projections. | Existing + strengthened | Analysis/TUI | Deferred |

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

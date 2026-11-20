# Writer's IDE — Engineering Design v0.3

**Status:** Active implementation design  
**Supersedes:** v0.2 only for the extensions below; all eight approved Phase 1 invariants remain in force.  
**Requirements:** [SRS v0.2 addendum](software-requirements-v0.2-addendum.md), plus the SRS v0.1 baseline.

## 1. Issue #1 traceability

| Topic | Existing SRS | Issue #1 effect | Implementation allocation |
|---|---|---|---|
| GrandView hoist/fold/clone, cards, budgets, Moorcock, Scrivener/Ranperen, plots, chronology, continuity, search, optional Ollama | Existing | Avoid duplicate requirements | Retain v0.1 allocation; UI/import slices remain outstanding |
| Typed links/backlinks/passage anchors | New | Strengthened with endpoint checks and stable anchors | Migration 0002 + application services; anchor revalidation/rendering remains |
| Transclusion/include cycle checks | New | Explicit display/export distinction | Cycle prevention in database; expansion semantics remain |
| Saved structured queries | New | Dynamic virtual collections | Typed persisted filters and read-only query evaluation |
| Reading, annotation, revision pipeline | New | Phased optional capabilities | Separate later slices; no source mutation |
| TUI interaction contract | Existing workspace list | Keyboard, palette, history, panes and save/conflict state made testable | Full-screen app slice |

## 2. Link and anchor model

`EntityRef` is a tagged value for scene, character, plotline or research UUID. `StoryLink` contains a directed source, target, optional source/target `AnchorId`, a `LinkKind`, and an author label. The SQL table uses stable IDs, has source/target indexes, and triggers validate endpoint existence and that an anchor belongs to the referenced scene. Endpoint deletion is intentionally not hidden by cascading link deletion: the integrity report detects dangling edges. Renaming/moving an entity only updates its row/placement and cannot invalidate a link.

`PassageAnchor` stores a UUID, scene ID, UTF-8 byte offsets, exact quote, prefix/suffix context, and source SHA-256. Creation validates UTF-8 boundaries and exact text against the current file. On manuscript edit, the required future policy is: if the quoted text occurs exactly once, propose a new location; if absent or ambiguous, retain the anchor and mark it `needs_review`/`broken`. Automatic retargeting is prohibited until the user confirms. The v0.3 API currently creates anchors but does not yet run re-anchoring after a save.

Transclusion/includes are edges over entity IDs and are cycle-checked by a recursive SQLite trigger before insertion. The first implementation rejects an edge that makes its source reachable from its target. Expansion is a reader/compiler projection, never a copy or mutation of canonical text. A compiler must offer an explicit expand-or-reference policy and identify included material in diagnostics.

The schema uses guarded polymorphic `(kind,id)` endpoint pairs because SQLite cannot express one foreign key to several target tables. Insert/update triggers check all supported tables; a diagnostic detects missing records following deletion or legacy corruption. This exception is limited to typed document edges; ordinary story relationships retain relational foreign keys.

## 3. Saved structured queries

`SavedSceneQuery` is a named, stable-ID view with a normalized ordered list of typed filters. Current filters are status, character participation, plotline membership, nominal story-time before/after, and unresolved linked issue. All filters combine with AND. Query evaluation joins current tables at read time, omits archived scenes, and orders by chapter/position/scene UUID. It returns IDs only; it does not write placement or cache stale membership. New filters require an explicit domain variant and a fixed SQL evaluator branch. Query definitions are project data and migrate with schema changes.

Chronology query filters use nominal ticks only; uncertainty-aware overlap/range queries belong to the analysis slice and must not silently interpret a missing nominal tick as zero.

## 4. Migration 0002

Migration 0002 adds `research_item`, `passage_anchor`, `story_link`, `saved_query`, `saved_query_filter`, `story_issue`, `scene_issue`, and rebuildable `scene_fts`. Existing version-1 projects migrate transactionally on open. A database newer than supported remains rejected. Foreign keys remain enabled for every connection. FTS5 is built into the selected bundled SQLite; FTS content is derived data and can be rebuilt. Future changes use numbered, forward-only migration files.

The migration's triggers enforce supported endpoint kind/existence, scene ownership for anchors, and acyclic include/transclude edges. Query result order is derived at evaluation time and cannot mutate ordering constraints. Issue status values are constrained in SQL.

## 5. Application service contracts

The application façade exposes anchor creation, typed link creation, outgoing links, backlinks, broken-link reports, saved query persistence and query execution. The terminal and CLI must call this façade; neither writes domain SQL. Storage validates passage byte ranges and link endpoints in addition to database constraints. A future `ExpandDocument` use case must carry a visited set and return a typed cycle error even though storage prevents newly created cycles, so imported/corrupt data is safe to inspect.

## 6. Failure and integrity handling

- Invalid entity IDs, mismatched anchor ownership, malformed passage ranges, unsupported query filters and duplicate names fail before or within a transaction without partial writes.
- Failed saved-query replacement leaves the prior definition intact.
- Query evaluation reports unavailable/corrupt query definitions as errors rather than returning a misleading empty result.
- Missing link endpoints are diagnostics and are never silently deleted or retargeted.
- A changed reference-file fingerprint makes annotations require review; source files remain read-only.
- FTS indexes can be rebuilt from canonical text and metadata; FTS failure must not corrupt manuscripts or database entities.
- Include cycles are rejected atomically. Read-time expansion still guards cycles as defense in depth.

## 7. Acceptance tests for this slice

1. Create a passage anchor over UTF-8 content; reject invalid offsets, split code points and quote mismatch.
2. Link scene to scene using IDs; move and rename the scene and verify link/backlink identity is unchanged.
3. Reject unknown endpoint and wrong-scene anchor both through service calls and direct SQL.
4. Create an include/transclude chain, reject a cycle, and verify the earlier edges remain intact.
5. Persist a saved character/plot/status/time/issue query, reopen project, and verify current results refresh as relationships/status change.
6. Verify query results use manuscript order and leave placement rows byte-for-byte unchanged.
7. Open a version-1 fixture and verify migration 0002 applies once; open version 2 and verify idempotent open; reject version >2.
8. Verify broken-link diagnostics report deleted/missing endpoints if encountered in an imported or repaired project.

## 8. Open design decisions

- Anchor reattachment UX and fuzzy matching threshold; no automatic fuzzy move is allowed before explicit confirmation.
- Whether passage offsets become a richer block/character position model for Markdown formatting.
- Exact transclusion rendering and export marker syntax.
- Query OR/group nesting and custom metadata filters; initial model intentionally supports AND-only filters.
- FTS fallback if a user-provided SQLite build lacks FTS5 (the current bundled build includes it).
- TUI editor component and key-binding configuration format.
- EPUB parser/security profile, annotation anchoring across normalizations, and Ollama API/model compatibility.
- Snapshot storage and selective restore semantics over journal-managed Markdown files.
- Pipeline cache key format and invalidation granularity.

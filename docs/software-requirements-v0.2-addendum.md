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
| HYP-004 | Include/transclude links are navigable references, do not copy canonical prose, and cannot form cycles. | New | Domain/storage/reader/compiler | Stable-ID inclusion of scene Markdown, research-document bodies, and imported Reading-document text; source hashes are checked before expansion, cross-document cycles are rejected, and derived compilation preserves canonical text |
| QRY-001 | Named typed scene queries combine status, character, plotline, chronology and unresolved-issue filters. | Strengthened | Domain/storage/CLI/TUI | Save, list, and dynamic run exposed in CLI and terminal as a live virtual collection; current metadata changes refresh results without changing manuscript order |
| QRY-002 | Query results refresh from current story data and never change manuscript order. | New | Application/analytics | Implemented and tested at storage and terminal view boundaries; terminal preserves the full scene cache and maps navigation by stable scene ID |
| IDX-001 | Full-text and relationship indexes support search while preserving stable passage references. | Strengthened | Storage/search/CLI/TUI | Implemented project-wide rebuildable FTS5 index across scenes, custom scene fields, characters, locations, plotlines, events, research notes, issues and reading documents; typed hits navigate from terminal; richer query syntax remains |
| TUI-001 | Full-screen keyboard-first Ratatui/Crossterm workspaces include all SRS workspaces, help, bindings, palette, history, split panes and conflict state. | Existing + strengthened | Terminal application | Partial: configurable F-keys; scene/section/chapter and basic entity creation; typed character/event/issue/plotline linking, issue status transitions and core scene purpose fields; validated basis-point allocation; multiline editing, selection, clipboard and conflict indicators; live plot and character matrices; entity-aware project search routing and record inspection are implemented for characters, locations, plotlines, events, issues, and research; character profile read/edit with aliases is implemented; location, plotline and worldbuilding forms and OS clipboard integration remain |
| TUI-002 | Central typed command registry and dispatcher provide stable command IDs and shared keyboard/palette/menu actions. | New (Issue #8) | Terminal command architecture | Registry owns metadata, aliases, argument contracts and dispatch for workspace, save, help, Compile, and parameterized scene/entity/reading/search/link/revision/extraction commands; hierarchical menu selection routes through the same dispatcher; general per-action availability, configurable keybinding conflicts and CLI integration remain |
| TUI-003 | Ctrl+/ main-menu shortcut with F10 fallback; ordinary slash remains manuscript input. | New (Issue #8) | Terminal key handling/menu | Ctrl+/ and Ctrl+_ terminal encoding plus F10 open the menu; printable slash remains editor input; hierarchical menu navigation is implemented. Configurable chords, full binding conflict reports and terminal enhanced-key protocol negotiation remain |
| READ-001 | Read text/Markdown/EPUB references with persistent highlights, annotations, checkpoints and indexes without modifying source files. | New | Reading workspace | Text/Markdown/EPUB import, persistent highlights/notes/bookmarks/checkpoints/terms, linked entities, global search and direct annotation/bookmark jumps and in-place current highlight/note styling are implemented; synthetic EPUB is tested, real-source compatibility and richer reading interactions remain |
| LLM-001 | Optional local Ollama extraction is provisional, passage-linked, provenance-bearing and never silently promotes facts or edits prose. | Existing + strengthened | Optional integration | CLI/TUI localhost extraction accepts scene or reading sources and stores provisional hashed quote-anchored proposals; proposal review shows status and evidence, and explicit per-claim approval or dismissal is persisted with optimistic file updates; approval revalidates source evidence before creating searchable canonical knowledge; offline mock verifies the HTTP contract; live Ollama compatibility, claim editing/retraction, and entity-specific promotion remain unverified |
| REV-001 | Snapshots, diffs, selective restore and history preserve durable journal behavior and Git compatibility. | Strengthened | Storage/revision UI | Verified snapshots, line diffs, CLI history/restore, and interactive terminal diff with hunk selection/restoration through the durable save journal; graphical hunk selection remains |
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

### Imported Reading-document inclusion milestone

Typed story links now accept Reading document IDs as document endpoints. Migration 0009 preserves existing links while adding Reading IDs to the endpoint constraints and source-existence checks. Compilation resolves extracted text only after verifying the imported source fingerprint, then includes it in the derived view without editing source bytes or annotation records. Storage and app tests cover stable backlinks, scene-to-reference inclusion, cycle checks across document types, and source-safe compiled output.


### Issue #8 command registry and shortcut foundation

A typed terminal registry now provides stable command IDs and metadata for workspace navigation, save, help, Compile and parameterized project actions. Keyboard workspace dispatch, palette entries and hierarchical menu choices use the shared typed dispatcher. Ctrl+/ (plus Ctrl+_ where emitted by terminal protocols) and F10 route to the main menu. F10 has been removed from the default Research workspace binding; Research remains accessible through Tab/BackTab. Ordinary `/` inserts into the manuscript editor. The popup preserves editor state and surfaces disabled-action reasons.

The menu milestone now renders and navigates a category popup from the central command registry, preserves editor state on open/close, routes arguments into the command prompt and reports disabled reasons for context-sensitive actions. The menu has File, Edit, Insert, Scene, Plot, View, Compile, Tools and Help categories. Remaining Issue #8 work includes arbitrary command keybindings, a binding editor and conflict report, enhanced keyboard protocol negotiation, and expanded contextual availability policy. Ctrl+/ remains terminal-dependent; F10 is the fallback.

The terminal registry also covers parameterized palette commands with stable IDs, categories, descriptions, aliases, and argument contracts. All registered palette invocations pass through the central dispatcher. Availability predicates, user-configurable shortcut conflict checks, and CLI registry reuse remain follow-up work.


### Evidence-checked extracted-knowledge approval milestone

Approved knowledge claims now persist in schema migration 0007 with candidate kind/name/statement, optional story-time text, stable source identity, source fingerprint/length, exact quoted passage and byte offsets, model, and approval time. Promotion rejects stale sources and invalid evidence offsets, is idempotent for a claim already promoted from the same evidence, and updates the rebuildable FTS index. The CLI `approve-extraction <project> <proposal-json-file> <claim-index>` and terminal `approve <proposal-json-path> <claim-index>` promote one claim after explicit author action. The `knowledge` CLI command, Story Bible rows, and project search expose approved records and their evidence. Tests cover scene/reading references, source immutability, staleness, persistence, idempotency, indexing, and terminal navigation. Proposal claim editing/retraction and promotion into entity-specific character/event records remain deferred.


### Selective dependency-aware processing milestone

The pipeline now exposes `run_selected`, adding transitive dependencies for requested stages and reporting unrequested stages as `NotSelected`. Failed dependencies still mark dependent stages `Skipped`; failures remain visible. Stable stage names support CLI `process --only` and terminal `process-only`/`process-expand-only`. The full `process` command remains unchanged. Tests cover Export's Compile dependency, cache reuse, excluded stages, dependency failure, stable stage names, CLI validation and terminal invocation. Declarative profiles and richer build configurations remain deferred.


### Provisional extraction review milestone

Authors can list and inspect proposal files and their candidate claims, including evidence excerpts and provisional/approved/dismissed status. A dismissed claim remains in its provenance record, cannot be promoted, and does not create a canonical story fact. Approval still requires explicit action and current source/evidence validation. CLI and terminal updates are project-local, hash-checked, atomically replaced, and reject stale proposal versions. Previous proposal JSON without a dismissed field loads as provisional. Live Ollama compatibility and entity-specific candidate promotion remain unverified.


### Character profile persistence and editing milestone

Migration 0010 persists the SRS character profile fields: full and preferred names, aliases, description, background, goals, motivations, secrets, internal/external conflicts, and status. Aliases are case-insensitively unique per character. Profile content and aliases are searchable. CLI profile inspection and field/alias updates are available; the terminal command registry exposes profile inspection and editing. Migration, persistence, validation, search, application service, and terminal workflow tests pass. Character-to-character relationship editing and first/last appearance summaries remain incomplete.


## Issue #21 — WYSIWYM semantic document foundation

The canonical prose choice remains portable UTF-8 Markdown, with story structure and semantic metadata external to prose. The new `story-document` prototype parses Markdown structure and inline semantics into a versioned source-preserving tree with UTF-8 byte ranges and content fingerprints. The dependency-aware compiler now constructs a renderer-neutral ordered `CompileIr` with stable content-unit and source-segment identities and output-to-source byte mappings; Markdown compilation passes through this IR. The prototype covers headings, paragraphs, emphasis, quotations, lists, code blocks, tables, footnotes, links, images, and task markers through pulldown-cmark 0.9.6. Synthetic UTF-8/round-trip/source-map tests pass. There is no author-provided formatted chapter fixture in the repository, so source syntax coverage and encoding decisions are not frozen pending real-chapter fidelity review. Full semantic inset persistence and print/digital renderers are deferred.

## Issue #20 — Fiction and nonfiction content model foundation

Projects now persist a book type separately from an optional template identifier. The built-in template catalog identifiers are Blank Fiction, Novel, Short Story, Blank Nonfiction, and Non-fiction Book. Migration 0011 adds typed direct-prose content units for front matter, section bodies, chapter bodies, and back matter; migration 0012 records the selected template identifier. Chapter body text has its own stable UUID, Markdown file, hash, word count, search projection and durable write/recovery journal. It can be saved, reopened, relocated, conflict-inspected, and compiled without creating a scene. The initial composition rule emits a structural node's direct body before its child content and scenes; front matter precedes the outline and back matter follows it. Recovery failpoint tests cover pre-intent cleanup, intent, pre/post replacement and post-metadata commit; conflict tests preserve both versions and require an explicit prepared/external choice. Declarative author-template editing/import/export, detailed template prompts and mixed body/scene ordering remain follow-up work and are not claimed as implemented.

## Issue #11 — Local Git snapshot foundation

A new `story-git` crate supports project-local repository initialization, named snapshots, history, and diffs through the application API, CLI, and terminal command registry. A SQLite `BEGIN IMMEDIATE` writer barrier reconciles pending scene/content-unit journals, rejects unresolved write conflicts, generates deterministic `snapshots/database.sql`, validates it in a fresh SQLite database, and holds the barrier through Git staging/commit. SQL includes canonical metadata, relationships, migration history, and import provenance; excludes transient journals, local scene snapshots, rebuildable search projections and FTS index rows. Git stages only app-owned project paths and refuses to consume unrelated staged files. Live databases, WAL/SHM, runtime state, exports, temporary files and common secrets are ignored. Tests cover repeatable no-op snapshots, SQL byte determinism/replay/integrity, consistent prose/metadata commits, path safety, live DB exclusion, and preservation of unrelated staged work. Historical deleted-scene reconstruction, selected/full restore, automatic interval snapshots, remote operations and incompatible-schema recovery remain incomplete; Issue #11 is not yet fully satisfied.

## Issue #2 — Writing workflow enhancements

| Requirement | Contract | Implementation state |
|---|---|---|
| WRK-001 Outline synchronization | Planning-only scene placeholders remain real scenes with stable IDs; structural reordering changes canonical output order; promotion does not duplicate content. | Implemented through existing scene creation, synopsis metadata, chapter placement, and Compile IR ordering. |
| WRK-002 Entity mention index | Detect names and aliases in scene prose as candidate references; retain byte offsets and source hash; distinguish detected, ambiguous, confirmed, dismissed and needs-review states; never infer narrative participation. | Implemented for character names/aliases, locations and story objects. Reindex occurs on scene save/open. Candidate confirmation does not write `scene_character`. |
| WRK-003 Quick capture | Persist scratchpad notes, support pinning and promotion to stable scene/story-bible/research destinations, preserve source text. | Implemented in schema 0013 and terminal/CLI commands. Promoted notes retain the source body and destination reference. |
| WRK-004 Session restoration | Restore workspace, selected/editing scene, cursor, scroll offset and reading document/checkpoint; tolerate missing references. | Implemented in project-local `.writers-ide/session.json`, written atomically on key/paste events. Layout/split-pane and outline expansion state are not yet saved. |
| WRK-005 Personal passage comments | Attach private notes to anchored manuscript ranges, resolve/reopen, and report broken or uncertain anchors. | Implemented in schema 0014, using the existing passage-anchor revalidation system. No collaborative review or DOCX redlines are included. |
| WRK-006 Manuscript assemblies | Keep per-edition ordering and inclusion separate from canonical scene placement; compile selected content by stable identity. | Implemented in schema 0015 and app/CLI/TUI APIs for scene membership, order, inclusion and Markdown Compile IR output. Custom export settings and assembly editing UI remain limited. |
| WRK-007 Portable archive backup | Create a single-file archive with a manifest and hashes, validate before publication, and restore safely to a new directory after relocation. | Implemented through `backup-archive`/`restore-archive`; archive traversal, file type, identity, content hash and project integrity checks are enforced. |

Issue #2's further scope remains: a plotted two-dimensional spatial canvas, board card resizing, and board export. Editor split ratios are configurable independently for horizontal and vertical layouts with Ctrl+arrow and persist across sessions. Optional privacy-preserving statistics are implemented with writing-stats on|off|report|reset: only aggregate Unicode character counts for typed versus pasted input and successful saves are stored in the local session file; no manuscript text or key sequence is recorded. Collection is off by default. Assembly drafts persist edits across multiple scene files as one resumable work unit, with each changed file committed through its normal optimistic journal. A restart can resume scenes already at either the old or proposed hash; an external divergent edit leaves the draft and current prose intact. In the terminal, :assembly-edit <assembly-uuid> opens one continuous editable buffer with stable scene boundary markers; Ctrl-S persists the complete proposal before journaled commit, and :assembly-drafts / :assembly-resume <draft-uuid> recover interrupted work. Marker damage blocks saving before any manuscript file changes. The CLI exposes draft creation, per-scene text replacement and commit/resume. The terminal also provides a focus mode for the scene editor (Ctrl-G) and persists that preference in session restoration. Spatial planning boards store independent entity cards, finite canvas coordinates, notes and cross-board-validated connectors; storage tests cover scene-order independence and invalid SQL-level connections. The Planning Boards workspace lists boards and positioned cards; Enter opens a board or follows a scene card, Ctrl+arrows move the selected card, and Esc returns to the board list. CLI and palette commands support board creation, typed entity placement, coordinate moves, connectors and item inspection. The terminal currently renders coordinate rows rather than a plotted two-dimensional canvas. Story Bible templates are stored as portable TOML under templates/story-bible and provide typed reusable character, location, object, plotline, and event profiles; profile values are stored locally as validated JSON with required-field, enum, and entity-reference checks. Existing canonical scene files remain the prose source of truth.

### Issue #4 — Multilingual editing foundation

Terminal manuscript cursor columns now use extended grapheme clusters for navigation, insertion, deletion, and selection, so combining marks and emoji sequences are not intentionally split by editor actions. Scene and content-unit word counts use Unicode word-boundary segmentation rather than whitespace tokenization, including in processing reports. Project language tags are persisted in schema migration 0017 and default to `en`; language tags can be read and set through CLI `language` and terminal `project-language`; Unicode word-boundary counts are deterministic and locale-independent. Tests cover multilingual Latin/CJK samples, persisted language settings, grapheme cursor boundaries, and actual insertion/deletion of combining and emoji clusters. Script-specific counting conventions remain open.

### Issue #12 — Scoped find and replace foundation

Core search helpers now provide literal Normal search, Extended escapes (`\\n`, `\\r`, `\\t`, `\\\\`), and Rust-regex mode with byte ranges. CLI and terminal support scopes for the current scene, an explicit scene UUID, a chapter UUID, or all scenes. `find` selects the first result; F3/Shift-F3 navigate through all matches. `replace` saves changed scenes through optimistic durable writes. Regex replacement uses Rust regex capture syntax. Search is case-sensitive; interactive replacement preview and transactional multi-scene undo remain open.

### Issue #13 — Split reference panes and outline narrowing foundation

The terminal editor supports a read-only scene reference pane with a switchable, session-restored buffer list. `reference-scene <scene-uuid>` opens/selects a stable-ID buffer; Ctrl-Tab changes focus, arrow/Page keys scroll, `reference-next`/`reference-prev` cycle buffers and `reference-close` closes the active one. Layout adapts to terminal width. `narrow <chapter-uuid>` filters the scene list without changing manuscript placement; `widen` restores it. Simultaneously visible reference panes, non-scene references, and persisted narrowing remain open.

### Issue #18 — Professional editing and mechanical proofing foundation

The editor has session-restored reference buffers, idle autosave through the optimistic journal, visible save conflicts, personal anchored comments, manuscript snapshots, selective hunk restoration, and bounded in-memory undo/redo. A deterministic read-only proofreader reports repeated adjacent words, double spaces and spaces before common punctuation with UTF-8 byte ranges; it is available through CLI `proofread <project> [all|scene-uuid]` and terminal `proofread [all]`. It never changes prose. Proofreading dictionaries and configurable rule sets, side-by-side conflict resolution, and long-duration interactive editing tests remain outstanding.

The terminal editor now performs a two-second idle autosave through the existing optimistic, journaled save path. On a detected conflict it keeps the unsaved buffer and stops automatic retry until further user activity, avoiding repeated blind writes.

A sustained persistence test now performs 24 successive writes of a large Unicode scene, reopens the project every six revisions, and verifies final text, stable scene identity, content hash, and Unicode word count.

The manuscript editor now supports a bounded 100-step in-memory undo/redo history per open scene, including cursor and selection restoration. Ctrl-Z undoes; Ctrl-Y and Ctrl-Shift-Z redo. Undo history is session-local, while durable snapshots remain the persistent revision mechanism.

### Issue #7 — Manuscript compilation formats

Compile IR renders structured section/chapter hierarchy, manuscript prose, and UTF-8 source ranges. `writers-ide compile <project> <markdown|html|epub|docx|pdf> [output-path] [--expand-transclusions]` renders Markdown in-process and uses a local Pandoc installation for HTML, EPUB, and DOCX. PDF additionally requires local LuaLaTeX. The terminal command palette exposes `compile-format` with the same output choices. Each export writes an atomic JSON source-map sidecar; offsets identify entities and byte ranges in the canonical UTF-8 Compile IR Markdown stream and do not claim final layout positions or PDF page coordinates. Export generation is offline and does not change manuscript sources. Renderer templates, named book designs, PDF typography/publishing diagnostics, and visual source mapping remain Issue #21 work.

### Issue #21 — LuaLaTeX and book-design foundation

The PDF renderer now uses a project-local temporary TeX cache, supports typed `trade-paperback`, `manuscript`, and `nonfiction` presets, and passes validated fixed Pandoc layout variables for paper size, margins, document class, font size, and line spacing. CLI and terminal compile commands accept `--design=<preset>`. The new deterministic publication diagnostics identify missing project title, empty manuscripts, and empty content units without mutating story data (`compile-check` in the CLI). Custom LuaLaTeX templates, richer typographic controls, page/chapter design, publishing diagnostics, and final production review remain incomplete.

### Issue #6 — Manuscript import foundation

The import crate now performs read-only DOCX, RTF, ODT, and EPUB inventory into a staging model with stable source-item identifiers, source fingerprint, extracted text, and explicit formatting/media loss warnings. `inspect-manuscript` emits the inventory without touching the source. `import-manuscript` creates a new project directory only, stores imported units as chapter/scene manuscript files, records import runs/source items/field provenance, and emits `exports/import-report.json`. Failed application removes only the newly created destination project; source files remain untouched. Synthetic DOCX/ODT/RTF fixtures cover extraction and source immutability. Review UI, import of comments/track changes, rich formatting fidelity, multi-document staging, and field mapping decisions remain incomplete. No real editorial documents were available for compatibility claims.

### Issue #19 — Editorial merge foundation

The review layer now inventories DOCX insertion/deletion markup and comments with author/date/comment IDs (`review-inspect`), and performs a line-oriented three-way merge from a base manuscript, current local scene, and reviewed external text. Non-overlapping edits merge automatically; overlapping edits return explicit base/local/external conflict regions and leave canonical text unchanged. CLI `review-merge <project> <scene-uuid> <base-file> <review-file>` and terminal `review-merge` use this merge through the journaled scene save. Mapping DOCX markup to anchored scene edits, side-by-side terminal conflict resolution, and one-hunk acceptance are not yet implemented.

### Issue #17 — Local browser planning and review foundation

An optional standard-library HTTP server (`writers-ide browse <project> [port]`) binds only to IPv4 loopback. The browser view reads the shared project hierarchy, scene metadata, character and plotline participation, and scene prose by stable ID; browser prose review is read-only and is served from the project application API. Browser planning edits use explicit local application-service calls. The server makes no external network calls.

The browser now supports chapter/scene creation, scene synopsis/status changes, and scene moves through the same WriterApp services used by the terminal. Its JSON projection includes stable-ID planning nodes and graph edges for placements, character participation, plotlines, events, locations, and story links; prose remains served through GET only. Host/origin validation complements the loopback bind. Query editing, all Story Bible and issue workflows, full workspace parity, and browser visualization of the graph remain open.

### Issue #10 — Configurable deterministic diagnostics foundation

Mechanical proofing now loads an optional project-local `.writers-ide/diagnostics.toml` rule set. It supports the existing repeated-word, double-space, and space-before-punctuation checks plus bounded Rust-regex rules; each rule has a stable ID, enabled flag, severity and message. Invalid config and regex patterns fail visibly. CLI `proofread-config` writes a default file without overwriting an existing one; CLI and terminal proofread commands use the project config and return UTF-8 byte ranges. Findings remain advisory and do not modify prose. Broader publishing/continuity rule families, dictionary-backed spelling, rule scoping and severity filtering remain incomplete.

### Issue #1 — Hypertext and knowledge workflows status

Existing stable-ID links, passage anchors, backlinks, broken-link checks, typed document inclusion/transclusion, cycle detection, Reading materials, annotations/bookmarks, term indexes, and explicit evidence-checked knowledge approval are available in the application and terminal workflows. Imported Reading files remain unchanged and link targets are navigable through entity IDs. Cross-document transclusion and provisional extraction provenance are covered by storage/app tests. Rich passage-level annotations across all imported formats and a graphical knowledge map remain future work.

### Issue #9 — Chronology and structured relationships status

Scenes and events use optional signed-integer `StoryTime` values with unknown/approximate/bounded precision, source labels, start/end values, and checked duration arithmetic. Character participation, scene-event links, locations, plotlines, and story links are separate relationships from manuscript placement. Timeline views and deterministic uncertainty-aware diagnostics distinguish definite overlaps, possible overlaps, unknown time, and adjacency. Full fictional-calendar display/entry tools and interactive multi-entity timeline editing remain open.

## Phase 5 issue implementation notes

Issue #14 now has UTF-8 safe snippet expansion, saved macros, persistent snippets/registers, and terminal multi-cursor placement, movement, insertion and deletion. `macro-record NAME` / `macro-stop` record typed characters, Enter, deletion and horizontal/Home/End cursor motion into reusable, persistent macros. `register-rotate A B [C...]` rotates existing values to the right and saves the change. Selection and vertical-navigation recording, paste recording, and arbitrary command recording remain outstanding.

Issue #5 adds persisted challenge start/deadline/target/mode/results, terminal countdown and word-delta recording, CLI controls, and bounded progress bars for project, chapter and challenge targets in Analytics. The view also aggregates completed/abandoned challenge words into daily bars and a cumulative total. General-session and per-scene time-series graphs remain open.

Issue #15 adds read-only character/plotline scene coverage checks exposed through `coverage`, plus persisted saved-query count assertions exposed through `query-assert-save`/`query-assert-check` in CLI and terminal. Coverage authoring UI remains outstanding.

Issue #16 adds project-relative edition recipes with `compile-edition`/`watch-edition`, source-map output, symlink/traversal protection, and local Aspell-based spellchecking with optional project word lists. The original `compile-watch` command remains. Assemblies support alternate ordering. Multi-stage pipeline automation and broader dictionary formats remain open; spelling requires an Aspell executable installed locally.

Issue #3 adds an in-editor timer and opt-in Kamikaze inactivity behavior. Kamikaze removes the previous word after ten seconds without typing; Undo restores it. Configuration and richer challenge modes remain open. Phase 5 is a partial implementation milestone, not full closure of the listed issues.

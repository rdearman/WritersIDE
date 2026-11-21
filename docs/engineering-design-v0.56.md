# Writer's IDE — Engineering Design v0.56

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.55

## Browser graph visualization (Issue #17)

The browser planning page renders the existing shared `/api/plan` projection into an inline SVG graph. Stable entity UUIDs identify structural groups, scenes, characters, plotlines, locations and events; persisted relationship edges are drawn between available nodes. Scene nodes navigate to the read-only scene review pane. The browser still mutates only the planning operations exposed by its API, and no browser route writes manuscript prose. This is a visualization of the shared graph data rather than a second graph store. Full terminal workspace parity, query editing and broader Story Bible/issue editing remain open.

## Phase 3 production refinements

`story_document::CompileFormat` now includes `Latex` (`latex`/`tex`) alongside Markdown, HTML, EPUB, DOCX and PDF. PDF compilation keeps renderer output in a project-local temporary working directory and returns `compiled.tex` plus `lualatex.log` as auxiliary artifacts. CLI compile, edition and watch commands, and the terminal compile command publish these artifacts next to the requested PDF using atomic writes. The generated TeX is produced from the same `CompileIr` and selected `BookDesign`; source maps continue to describe renderer-neutral Compile IR offsets, not visual page positions. Built-in fixed profiles now cover trade paperback, hardback, submission manuscript, nonfiction, and technical/academic layouts. Tests exercise direct TeX output and PDF sidecar generation when local Pandoc/LuaLaTeX are installed.

DOCX editorial review can now derive an accepted or rejected text view of `word/document.xml` tracked-change insertion/deletion elements. `review-inspect` exposes insertion, deletion and comment metadata; `review-docx` submits the selected accepted/rejected view with the author-supplied base through the existing three-way merge and optimistic journal save. It never modifies the DOCX source. Comment range anchoring and interactive selection/resolution of individual conflicts are still open.

## Terminal outline operations

The command palette now offers structural scene-planning operations: `section <title>`, `chapter <title>`, `subchapter <section-id> <title>`, `scene <chapter-id> <title>`, `move <scene-id> <chapter-id> <position>`, `rename <scene-id> <title>`, and `scene-status <scene-id> <status>`. Creation chooses the next sibling position; movement delegates to the application service's transactional position algorithm. Scene renaming updates typed metadata without changing stable IDs or file paths. The Outline workspace displays chapter titles alongside their scenes.

The terminal regression test creates nested structure, drafts text, links a POV character, moves and renames the scene, then verifies placement, scene identity, relationship, and manuscript content. This makes the TUI useful for actual outlining and rearrangement, while full column/hoist/clone views and metadata forms remain future work.

## Practical manuscript editing and entity creation

The Editor workspace now supports character-indexed text selection, highlighted selection rendering, multiline copy/cut/paste in the application's internal clipboard, and Shift+Arrow selection. Uppercase/shifted text is accepted, and horizontal movement crosses line boundaries. Selection and clipboard state are transient UI state; only resulting manuscript text is persisted, through the existing optimistic durable save path. Conflicts continue to retain the unsaved editor buffer.

The terminal command palette can create a character (`character <name>`), plotline (`plotline <name>`), issue (`issue <title>`), and research note (`research <title> | <body>`), then navigate to the corresponding workspace. These operations use `story-app` services and do not issue domain SQL from the UI. They create initial records only; detailed entity editing forms and OS clipboard integration remain unresolved.

Regression tests cover multiline selection extraction/deletion, clipboard paste, selection extension/collapse, shifted uppercase entry, line-boundary movement, and command-palette creation for character, plotline, issue, and research entities. Offline workspace tests and Clippy pass for this revision.

## Plot Grid matrix

The Plot Grid workspace now derives a read-only matrix from `analytics_dataset`: scenes are rows in manuscript order, plotlines are columns, and a filled marker indicates scene membership. Chapter titles are resolved from structural nodes; unplaced scenes are labelled explicitly. The matrix reads canonical relationships and cannot change them. The app-level regression fixture verifies that an assigned plotline appears in the matching scene row.

The Characters workspace now derives a read-only scene-by-character matrix. Cells distinguish POV (`P`), major (`●`), minor (`○`), off-camera (`o`), mentioned (`m`), and no recorded participation (`·`). It reads application analytics and remains a projection; editing relationships continues through application services. A regression check verifies that a POV link appears with its role marker.

## Passage anchor revalidation

Scene saves revalidate passage anchors inside the same SQLite transaction that commits the manuscript hash and search index, including recovery after an atomic file replacement. A unique exact quote match is reattached automatically. Repeated matches are scored against saved prefix/suffix context and are reattached only if one candidate has a unique non-zero best score. Missing passages become `broken`; ambiguous repeated passages become `needs_review`. The stable anchor UUID is retained in all cases. `anchors` lists a scene's anchor states and `anchor-move <anchor-uuid> <start-byte> <end-byte>` explicitly reattaches an anchor after the author selects the correct passage. The operation validates UTF-8 boundaries and exact quote identity. The CLI provides `anchors` and `anchor-move` equivalents.

Tests cover unique relocation, context ambiguity, removed passages, explicit reattachment, invalid offsets and recovery after rename-before-database-commit. Anchor updates participate in the same transaction as recovered manuscript metadata so the durable journal cannot commit text while leaving anchor state silently current at stale offsets.

## Project-wide full-text index

Migration 0005 adds a rebuildable FTS5 index over a typed `project_search_content` projection. It indexes scene titles, synopsis, purpose, goal, outcome, status and manuscript text; character and location names; plotline names; event titles/descriptions/source time; research titles/bodies; issue titles/status; and imported reading titles/text. SQLite triggers keep the projection and FTS table current. Search returns typed entity kind/ID, title and excerpt, with deterministic ranking ties. `rebuild_project_search_index` recreates the virtual index from its ordinary projection table.

`search-all` is available in the CLI and terminal command palette. Terminal results navigate to their underlying story entity or reading document. Existing scene-only and reference-only search APIs remain available. The storage tests cover initial migration from schema v4, incremental updates for prose and metadata, typed results, empty queries, result limits, and index rebuild.

Migration 0006 adds custom scene-field values to that projection. Text, long text, integer, decimal, boolean, enum, and entity-reference scalar values are searchable. Triggers recompute a scene's indexed projection when a custom value is inserted, updated, or deleted, and when scene prose or core metadata is subsequently updated. Existing custom-field values are included when migrating. Tests verify custom value updates, deletion, and that later manuscript saves preserve the custom metadata in the index.

## Scene planning commands

The terminal command palette now edits core scene purpose fields through `scene-field <scene-uuid> <synopsis|purpose|goal|outcome> | <text>`. `character-link <scene-uuid> <character-uuid> <pov|major|minor|off_camera|mentioned>` creates typed participation. `event <title> | <source-time>` creates story events and `event-link <scene-uuid> <event-uuid> [relationship]` associates them with scenes. Issues can be linked through `issue-link <scene-uuid> <issue-uuid>`.

Plotline assignment uses `plotline-link <scene-uuid> <plotline-uuid> <overlap|equal> [role]`, preserving existing memberships while adding a distinct plotline. `plotline-shares <scene-uuid> <plotline-uuid>=<basis-points>...` submits a complete author-defined allocation set; the existing domain validation requires exactly 10,000 basis points. The terminal doesn't normalize incomplete or excessive allocations. Tests exercise the commands and verify role/relationship/metadata persistence through the application API.

The issue tracker now supports status transitions through `issue-status <issue-uuid> <open|in_progress|resolved|deferred|closed>` in the terminal command palette and CLI. Invalid statuses and missing issue IDs are rejected by the service/storage boundary; updates refresh the indexed issue status. The terminal integration test creates, links, and transitions an issue, then checks its persisted status.

## Reading annotation and bookmark navigation

The Reading command palette lists annotations and bookmarks with stable zero-based indices. `annotation-jump <index>` and `bookmark-jump <index>` navigate to their stored byte offsets without changing the imported source. Navigation validates that offsets are within extracted UTF-8 text and land on a character boundary, then updates the reading pane's scroll position and focus. Annotation/bookmark list output uses the same ordering and indices as jump commands. Integration tests verify listed indices and jumps into a later passage.

## Ollama HTTP contract test

The optional Ollama client now has an offline loopback mock test that binds an ephemeral `127.0.0.1` port and serves one `/api/generate` response. It verifies the real HTTP method/path, model name, `stream=false`, JSON response format, source text in the prompt, and extraction JSON decoding. The test passes under the elevated local-socket permission; ordinary sandbox execution denies TCP bind. No external network request or installed Ollama daemon is needed. Live compatibility with Ollama versions/models remains unverified.


## In-place reading annotation rendering

The Reading source pane renders current annotations over the extracted UTF-8 text without changing that text or the imported source file. Highlight annotations use yellow background with black foreground; note annotations use cyan underlining. Stale, broken, or otherwise non-current annotation records are not applied to the passage. Invalid, out-of-range, or non-character-boundary offsets are ignored defensively. Rendering uses stored byte anchors and keeps annotation records separate from source content. The terminal integration test verifies that a saved highlight decorates the expected passage while annotation/bookmark navigation continues to work.


## Analytics workspace report

The terminal Analytics workspace now displays all scene rows in the character appearance heatmap, with participation roles distinguished as POV, major, minor, off-camera, and mentioned. It also displays named character interaction pairs and shared-scene counts, subplot gaps with boundary scene titles, uncertain chronology pairs, and per-scene word-budget suggestions. Budget suggestions are explicitly labelled as previews and are generated from a read-only analysis projection; they do not modify scene targets. The report is scrollable as a workspace list. A synthetic regression fixture verifies each report section, includes rows beyond the former 20-row cutoff, and confirms that source scene targets remain unset.


## Chronology-aware terminal scene timeline

The Timeline workspace orders scenes by canonical `StoryTime` start values rather than manuscript placement. Exact, approximate, bounded, and unknown values are shown as relative day/time labels using the project's seconds-per-day tick model; bounds remain visible. Unknown values sort after known values. Equal or absent times have deterministic title and UUID tie-breaks. Enter maps the selected chronological scene back to its stable scene ID before opening the editor, so an alternate view cannot accidentally open a different manuscript row. Keyboard list navigation uses the current workspace's visible item count, allowing every expanded Analytics report row to be reached. Tests cover negative ticks, bounded and approximate interval labels, chronological order, unknown-last ordering, and long-list navigation. Story-event timelines and explicit ambiguity grouping remain open extensions.


## Event and scene timeline

Timeline records now combine scenes and story events by their canonical nominal start tick, falling back to interval bounds when no nominal value exists and placing unknown intervals last. Scene and event rows display start/end values with explicit approximate or bounded ranges; event rows also retain the original source-time representation for readers. Equal/unknown entries use title and stable ID tie-breaks. The `event-time TITLE | START | END | SOURCE-TIME` command creates a typed event through the application service. Each time accepts `?` for unknown, an integer tick for exact time, `LOWER..UPPER` for bounded time, or `~NOMINAL,LOWER,UPPER` for approximate time. Selection navigates scenes by stable scene ID and events to their Story Bible row. Stored event intervals are loaded as typed `StoryTime` values. The timeline's display order is a navigation aid; uncertain intervals are not asserted as conclusively ordered. Tests verify event interval persistence, mixed scene/event order, relative source labels and parser validation.


## Live saved-query virtual collection

The terminal's saved-query view no longer replaces the application-wide scene cache. `query <UUID>` validates the saved query, restores the full manuscript-order scene cache, and displays matching scene references as a virtual Search collection. Each view refresh evaluates the query against current structured data; scene ordering and placements remain unchanged. Enter resolves the selected query result's stable scene ID into the full scene cache before opening the editor. `query-clear` leaves the collection and returns Search to the full scene listing. Full-text search and saved-query views clear one another explicitly. The terminal integration test applies a status change while the query view is open, verifies membership refreshes immediately, confirms the full ordered scene cache remains intact, and checks that Enter opens the matching scene identity.


## Story entity selection and search navigation

Project search now routes characters, locations, plotlines, events, and issues into the workspace and row representing that entity. Character, location, plotline, and event indices account for the Story Bible's grouped ordering (characters, locations, plotlines, events). Issue hits select the matching issue row. Reading-document entity links use the same destinations. Enter in Story Bible, Issues, and Research inspects the selected record instead of opening a manuscript scene based on the unrelated shared row index. Creation commands select the newly created character/location/plotline/event/issue/research record. The storage/application layer now exposes location creation and ordered listing, and the terminal `location NAME` command adds locations to the Story Bible. Tests cover all supported project search entity routes, entity inspection without scene misnavigation, and location creation/listing.


## Optional transclusion-aware manuscript compilation

The processing service exposes an opt-in compile mode that expands scene-to-scene `Transcludes` and `Includes` links into a derived compilation view. The default `process` behavior remains unchanged. Expansion uses the storage layer's stable-ID graph traversal, which detects cycles and rejects invalid/broken targets; it does not rewrite a scene's canonical Markdown file or its indexed manuscript text. Each included passage is separated by a Markdown horizontal rule. A target scene remains present at its own manuscript placement, so inclusion can intentionally repeat its text in the compiled output.

The expanded view is explicitly selected with `writers-ide process <project> --expand-transclusions` or terminal command `:process-expand`. The expansion mode is included in the compilation fingerprint, preventing reuse of artifacts generated under a different mode. Tests verify normal and expanded output, separator placement, cache behavior, and unchanged canonical scene text. Transclusion expansion currently targets scene passages only; general document inclusion and author-configurable compilation policies remain open.


## Hierarchical menu and command registry foundation (Issue #8)

The terminal command registry is the shared source for stable IDs, labels, descriptions, categories, mnemonics, and typed actions for workspace navigation, save, help, and compile commands. Registered actions are dispatched through one application-state dispatcher from keyboard and exact command-palette routes. Existing parameterized command handlers remain in the established command parser and are an incremental migration target; new menu actions must be registered rather than adding a menu-only handler. The registry category uses **Compile**.

Ctrl+/ is reserved as the preferred main-menu invocation when the terminal reports it as `Char('/')` with Control; Ctrl+_ is also accepted for terminal protocols that encode the chord that way. F10 is reserved as the fallback and is no longer assigned to the Research workspace. Research remains reachable by workspace cycling. These chords dispatch to the same registered main-menu action. Until popup rendering is implemented, invoking that action reports that the menu UI is pending and leaves editor text, cursor, selection, dirty state, and undo-relevant content untouched. Plain `/` remains available in manuscript text; outside the editor it continues to invoke search.

The project keymap continues to support function-key workspace overrides, but rejects F10 as a workspace key because it is reserved. Terminal-specific Ctrl+/ reporting cannot be assumed across SSH, tmux, legacy protocols, or emulator configurations; F10 is the reliable fallback. Keymap-configurable command shortcuts, conflict diagnostics, Kitty enhanced keyboard negotiation, hierarchical popup navigation, disabled menu entries, focus restoration, and full registry migration of parameterized commands are deferred to the Issue #8 visual-menu milestone. Tests cover both encoded menu chords, the F10 reservation, registry uniqueness and Compile naming, plain slash insertion, and editor-state preservation.


## Evidence-checked approval of extracted knowledge

Migration 0007 adds `approved_knowledge_claim`, a searchable project record for a claim deliberately promoted from a provisional Ollama extraction. The record preserves candidate kind/name/statement, approximate story-time text, source type and stable source ID, SHA-256 fingerprint of the exact extracted text, source byte length, exact evidence quote and byte offsets, model name, and approval timestamp. SQLite source triggers require the scene or reading record to exist; uniqueness prevents repeated promotion of the same claim from the same source revision and evidence span. Search-index triggers add approved claims to project-wide FTS.

`WriterApp::approve_extraction_claim` validates the source kind and UUID, reloads current source text, verifies fingerprint and byte length, and requires the stored evidence range to resolve to the exact nonempty quote before insertion. A changed scene or reference is rejected for review/regeneration. Promotion is idempotent for a matching previously approved record. This operation is the only application path that creates an authoritative knowledge record from a provisional candidate. It does not silently create or mutate characters, events, manuscript text, or other domain entities.

The CLI accepts `approve-extraction <project> <proposal-json-file> <zero-based-claim-index>` and `knowledge <project>`. The terminal palette accepts `approve <proposal-json-path> <claim-index>` and `knowledge`; approved records appear in the Story Bible and are reachable from project search. The explicit approval operation commits the database record first, then atomically marks that one proposal claim approved. If writing the proposal file fails after database commit, rerunning the command resolves to the existing record, avoiding duplicate facts.

Tests cover scene and reading sources, source immutability, stale source rejection, evidence verification, idempotency, persistence after reopen, source existence constraints, FTS indexing and terminal search navigation. Rich proposal review/edit/reject workflows, entity-specific promotion into character/event tables, and fact retraction are not implemented yet.


## Interactive selective revision restoration

The Revisions workspace now lists snapshots for all scenes, rather than deriving the scene being inspected from the snapshot's list index. Enter opens the selected snapshot's complete line diff in the split pane. Diff hunks are delimited by equal lines and map explicitly to current-text and snapshot-text ranges. `[` and `]` select changed hunks; `r` restores the selected hunk through the existing optimistic save and durable manuscript journal. The operation snapshots the pre-restore text, refuses to run while the editor has unsaved text, and refreshes the manuscript word counts. `Esc` closes the diff. The selected hunk is highlighted in the diff pane; green additions and red removals remain distinguishable outside it.

The restore service continues to validate the expected content hash, snapshot ownership, and both line ranges before writing. Insert-only and delete-only hunks are represented by empty ranges on the appropriate side. Tests verify hunk mapping, selective restore, pre-restore snapshot creation, and content persistence. This provides an interactive restore path; arbitrary line selection and graphical side-by-side diffs remain out of scope for the current terminal view.


## Selective processing with dependency closure

`PipelineRunner::run_selected(input, requested)` is the selective processing contract. `Stage::ALL` defines deterministic report/order output; each stage's declared dependencies are recursively included in a `BTreeSet` closure. The runner executes that closure in topological order, reuses only fingerprint- and artifact-hash-verified cache entries, marks requested dependents `Skipped` when a dependency fails, and marks excluded stages `NotSelected` with an explicit reason. An empty API selection performs no stages and reports every stage as `NotSelected`. The existing `run` API remains the full-pipeline convenience wrapper.

Stable CLI names are `compile`, `word-count`, `plotline-analysis`, `character-analysis`, `continuity`, `export`, and `revision-report`. Use `writers-ide process <project> --only export` to run Compile and Export, or combine selections such as `--only character-analysis,continuity`. `--expand-transclusions` composes with `--only`. The CLI treats `NotSelected` as an intentional partial run, while `Failed` and dependency `Skipped` indicate incomplete processing. The terminal command palette accepts `process-only <stage,...>` and `process-expand-only <stage,...>`.

Regression tests verify transitive dependency selection, excluded-stage reports, cache reuse, failure propagation, stable stage parsing, CLI argument rejection, and terminal invocation. This adds stage selection without changing stage fingerprints or full-run behavior. Declarative processing profiles, configurable artifact formats, logs, and selective invalidation driven by source entity deltas remain future pipeline work.

## Review and dismissal of provisional extraction claims

Extraction proposals remain project-local JSON records under .writers-ide/proposals/. Each claim now has a backward-compatible dismissed flag (missing in earlier proposal files means false), in addition to its existing approved flag, evidence quote, byte range, source fingerprint, and model provenance. Dismissal changes only the proposal's review status; it retains the claim and its source evidence and never creates an authoritative story fact. Approved claims cannot be dismissed, and dismissed claims cannot be promoted.

The CLI command proposals <project> lists each proposal and claim with its status and evidence. The command dismiss-extraction <project> <proposal-json-file> <claim-index> records a dismissal; approval remains explicit through approve-extraction. The terminal proposals command opens a scrollable Research-workspace review list, while dismiss and approve apply a decision to the selected proposal path and claim index. The CLI and terminal read/write proposal files only through application/storage APIs.

Proposal updates are optimistic and confined to a regular JSON file directly under the project's proposal directory. The store checks the existing file's SHA-256 against the fingerprint observed at load, writes and synchronizes a uniquely named temporary file in that directory, atomically renames it over the proposal, and synchronizes the directory. Stale updates fail without replacing the newer proposal. Tests cover project-local path validation, list/load/update, stale fingerprints, older JSON compatibility, retained evidence after dismissal, refusal to dismiss approved claims, and CLI/TUI review-row rendering.

## Uncertainty-aware character chronology diagnostics

Continuity processing now delegates same-character interval comparison to `story_analysis::character_chronology_findings`. Pipeline character inputs retain both stable character IDs and display names, so characters with identical names do not create false co-location conflicts. Reports use `definite-overlap`, `possible-overlap`, and `unverified-overlap` classifications with scene IDs and shared character IDs. A definite overlap requires both intervals to be exactly/definitely ordered as intersecting; bounded uncertainty that prevents a conclusion is reported as possible, and any unknown endpoint that prevents a conclusion is unverified. Endpoints that are definitely before or exactly the same as the other scene's start are treated as non-overlapping adjacency. These are candidate chronology findings, not asserted errors.

Analysis tests cover definite overlap, bounded possible overlap, unknown/unverified time, touching boundaries, and different stable IDs for homonymous characters. A pipeline test verifies the emitted classifications and stable character references. Wider location/custody constraints and diagnostic-to-issue workflows remain future work.

## Registry-owned command palette aliases

Each registered terminal command now declares its own palette aliases and whether it accepts an argument string. Palette lookup iterates the registry, rejects ambiguous aliases in tests, and returns a `(CommandSpec, arguments)` invocation. Exact and parameterized Compile commands therefore share the existing central dispatcher; the main palette route no longer carries a separate Compile alias/prefix switch or bypasses the dispatcher for processing. Argument parsing for selected stage names remains in the typed action dispatcher.

This increment does not claim the whole parameterized palette has migrated: structural, entity, reading, link, and extraction commands still use the established parser. They remain an incremental migration task. The registry is the required source for any future menu entry; the visual menu remains deferred. Tests verify alias uniqueness, selected-stage argument routing, and rejection of arguments on no-argument registered commands.

## Stable-ID inclusion of story and reference documents

Migration 0009 extends the typed story-link endpoint check to imported Reading documents while preserving existing link rows. Migration 0008 and 0009 constrain `Includes` and `Transcludes` endpoints to document entities. The application service enforces the same rule, while SQLite triggers protect direct writes. `expand_document_inclusions(EntityRef)` traverses scene Markdown, research-item bodies, and validated Reading extracted text by stable IDs, preserving source fingerprints, annotations, and cycle checks across document kinds. A Reading source is loaded through `get_reading_document`, which verifies its managed source file hash before returning extracted text. `expand_scene_transclusions(SceneId)` remains as a compatibility wrapper. Compilation builds a derived expanded view and does not modify canonical scene prose or imported source files. Tests cover scene-to-research and scene-to-Reading expansion, cross-document cycles, direct-SQL rejection of non-document targets, and compiled output with source text preserved.

Reading annotations remain separate from inclusion links and reference the original document fingerprint. If the managed source is externally changed, inclusion fails with the existing source-conflict diagnostic instead of compiling stale or unverified extracted text.

## Registry coverage for parameterized terminal commands

The terminal command registry now includes the stable IDs, labels, descriptions, categories, aliases, and argument contracts for the existing structural, scene, character, plotline, event, issue, reading, search, link, snapshot, and extraction palette commands. Alias uniqueness is validated. The central command entry point resolves registered invocations and passes a parameterized command to one shared legacy action parser; registered non-parameterized actions still use typed `CommandAction` handlers. This allows a future menu entry to invoke the same registered action path without a second feature-specific implementation. Existing command syntax and handlers remain intact.

Tests cover alias uniqueness, metadata dispatch for a parameterized scene move, Compile argument routing, and rejection of unexpected arguments on no-argument registered actions. The registry still lacks per-action availability predicates, configurable keybinding conflict resolution, and CLI dispatch integration. Hierarchical popup rendering, menu navigation, and capability negotiation remain deferred to the Issue #8 UI milestone.


## Structured character profiles

Migration 0010 extends the existing character record with preferred name, description, background, goals, motivations, secrets, internal and external conflicts, and status. Aliases use a normalized child table with per-character case-insensitive uniqueness. The existing `name` remains the canonical full name for compatibility; preferred name is used for character lists. Profile fields and aliases participate in project-wide FTS search, and search content is updated transactionally with profile edits.

`story-app` exposes profile read/update, validated field updates, and alias creation. The CLI provides `character-profile`, `character-set`, and `character-alias`/`character-un-alias`; the terminal palette registers and routes the same actions through the shared command registry. Tests cover migration from pre-profile schemas, validation, alias uniqueness, search indexing, persistence after reopen, and terminal editing. Character relationships, first/last-appearance reports, deletion/merge workflows, and full-form editing remain open.


## WYSIWYM semantic-document and Compile IR prototype (Issue #21)

The draft, testable contract is recorded in [wysiwym-contract-v0.1.md](wysiwym-contract-v0.1.md).

The canonical prose contract remains UTF-8, Git-friendly Markdown files; prose is not stored as database paragraph rows and is never replaced by LaTeX or `.lyx`. Project hierarchy and typed story metadata remain structured data outside the prose. The new `story-document` crate is a prototype: it parses Markdown into a versioned, source-preserving semantic projection with nested node kinds and UTF-8 byte ranges, while retaining the exact original source string and SHA-256. The initial parser enables CommonMark plus tables, footnotes, strikethrough, and task-list markers. Unsupported constructs remain in original source bytes; the projection is rebuildable and not authoritative.

`CompileIr` is renderer-neutral and ordered. Its units have stable IDs, optional parent IDs, semantic kinds (front matter, section body, chapter body, scene, back matter), titles, and one or more semantic source segments. Rendering a Markdown target creates source-map entries from output byte ranges to stable source entity IDs, source fingerprints, and source byte ranges. The existing processing Compile stage now flows through this IR; its Markdown output remains compatible. Scene units preserve existing UUIDs and chapter placement. Inclusion segments retain the IDs of the source documents so included text is not falsely attributed to the including scene.

This is a prototype contract, not a frozen canonical encoding or complete renderer architecture. The repository contains no author-provided formatted chapter, so parser coverage has only been checked against synthetic Markdown fixtures. Before freezing semantic encodings, schema migrations, or the complete Compile IR, compare round-trip fidelity against a real formatted chapter and record unsupported constructs. Typed inset representation is planned for footnotes/endnotes, citations, figures/captions, tables, cross-references, index terms, and equations; no raw LaTeX passthrough is introduced. LuaLaTeX, DOCX, EPUB3, HTML, PDF diagnostics, book-design profiles, and document templates remain separate later work.

## Fiction and nonfiction content units (Issue #20)

Projects carry an immutable `BookType` and optional built-in template identifier. The project directory remains one portable project with Markdown prose; a nonfiction chapter may own a direct `ChapterBody` content unit without manufacturing a scene. Front matter, section body, chapter body and back matter are independently identified and stored, hashed and saved through the durable content-unit journal. Chapter body is emitted before scene children in the current Compile IR ordering; more flexible mixed ordering is still an open design item.

Built-in starting templates are Blank Fiction, Novel, Short Story, Blank Nonfiction and Nonfiction Book. The initial templates select project type and leave optional structure rather than imposing a writing method. Project type and template ID are validated against `project.toml`; schema migrations 0011 and 0012 establish the content-unit and template fields.

Core scene properties remain typed: identity/title/synopsis/status, placement/order, word counts, purpose/goal/outcome and chronology. Characters, POV, locations, plotlines, events, issues and research are relational references. Remaining author-specific fields use typed custom-field definitions and values. Existing custom field kinds cover text, long text, integer, decimal, boolean, datetime, enum and entity reference. Multi-entity references, author-defined declarative TOML template CRUD and within-chapter mixed ordering remain open work. Unknown imported source fields must remain visible through provenance/mapping records and cannot be silently discarded.

Acceptance coverage creates a nonfiction project, writes and reopens chapter prose without a scene, validates its hash/word count, and compiles it. Content-unit recovery tests exercise crash windows and divergent external edits.

## Local Git history and deterministic metadata snapshots (Issue #11)

`story-git` uses an installed Git executable with no remote or network requirement. The project is a local repository rooted exactly at the project directory; nested unrelated repositories are rejected. IDE commits stage only app-owned project paths and refuse unrelated staged changes. Live SQLite/WAL files, runtime state, exports, temporary files and common secrets are ignored. Repeating an unchanged snapshot creates no commit.

The storage service establishes a SQLite `BEGIN IMMEDIATE` barrier, recovers pending scene and content-unit journals, rejects unresolved conflicts, serializes deterministic metadata SQL, validates it in a fresh database, atomically writes `snapshots/database.sql`, and holds the writer barrier through Git staging/commit. The SQL snapshot includes schema, stable metadata, relationships, import provenance and migration history, while omitting transient journals and rebuildable indexes. The live database and manuscript files cannot cross a snapshot boundary independently.

Local snapshot, history and diff operations are available through the application API, CLI and command registry. Existing scene snapshots support comparison and selective restoration; project backup writes a consistent isolated copy and validates it before publication. `git-restore-new <source-project> <commit> <new-directory>` reconstructs only app-owned versioned files, restores the deterministic SQL metadata snapshot into a fresh SQLite database, validates identity/schema/referential integrity, and publishes into a previously absent destination. It never modifies the source checkout. Restoring into an existing directory is rejected. Older commits that predate deterministic metadata snapshots cannot be restored this way and return an explicit error. Automatic snapshot scheduling and selective historical restore into the active project remain open. Git is not used as a replacement for write-ahead persistence, conflict handling or portable backup.

## Hierarchical terminal menus (Issue #8)

The central command registry defines stable command IDs, labels, categories, descriptions, argument contracts and dispatch actions. Menu presentation, command-palette aliases and dispatch share this registry. Argument-taking menu entries open the existing command prompt using their registered prefix, so menu actions do not duplicate domain logic.

`Ctrl+/` is the preferred main-menu shortcut and `F10` is the fallback. `Ctrl+_` is accepted for terminals that encode the slash chord that way. Plain `/` remains ordinary manuscript text. The popup categories are File, Edit, Insert, Scene, Plot, View, Compile, Tools and Help. Left/right switches categories, up/down selects an action, Enter dispatches it and Escape closes while preserving editor state. Contextually unavailable commands report a reason without dispatching.

The project keybinding file still maps function keys to workspaces rather than arbitrary command chords. A keybinding editor, scope-aware conflict diagnostics, multi-key sequences and terminal enhanced keyboard protocol negotiation remain open Issue #8 work. F10 remains the interoperable fallback; no unmodified printable character is reserved for menus.

## Writing workflow foundations (Issue #2)

### Outline synchronization and entity mentions

Outline promotion uses the existing scene entity and chapter placement API. An unwritten scene is represented by a normal stable scene with empty Markdown plus planning metadata; no placeholder prose is manufactured. Canonical Compile IR continues to read scene placement/order, so moving a scene changes manuscript order without changing scene UUID, links, anchors, metadata or manuscript path.

Migration 0013 adds `story_object`, `detected_mention`, and `detected_mention_candidate`. Detection scans Unicode character boundaries with case-folded names and alphanumeric boundaries. Character full/preferred names and aliases, location names, and object names produce candidate mentions. A collision produces `ambiguous`; a single entity produces `detected`. Mention confirmation updates only the mention index and never creates character participation. Confirmed mentions retain their stable entity reference; if the quoted bytes no longer occupy their recorded range after a manuscript edit, the mention becomes `needs_review`. Dismissed false positives remain suppressed while their exact passage remains. Other candidates are rebuilt on save and when a scene is opened. The current index records scene byte offsets and source hashes; paragraph-level hover previews and automatic entity appearance counts across all entity kinds remain UI/analysis work.

### Scratchpad and session restoration

Migration 0013's `scratchpad_note` stores title, body, pin state, order, status and optional typed destination reference. Destination triggers validate scene, character, location, object, plotline and research records even when SQL bypasses the application service. Promotion changes the note state and attaches a destination while retaining original text; deleting a destination reactivates the note instead of orphaning its body. The terminal and CLI expose capture/list/pin/promote operations.

The terminal stores a compact session snapshot in `.writers-ide/session.json`, atomically replacing and syncing it after input events. It remembers workspace, selected/editing scene IDs, cursor, scroll, selected index, reading document and reading offset. Restore resolves IDs against current project records and skips stale references with a visible message. The editor's full-screen focus-mode setting is also persisted; Ctrl-G toggles it while in the editor. This is UI state only and does not affect manuscript transactions. Outline collapse/hoist remains unrepresented. Horizontal and vertical editor split ratios are independently adjustable with Ctrl+arrow, clamped to 20–80 percent, and persisted in the session file alongside focus mode.

### Personal passage comments

Migration 0014 adds `personal_comment` rows that reference the existing stable `passage_anchor` table. Creation validates exact UTF-8 byte boundaries against the canonical saved manuscript and inserts anchor plus comment in one SQLite transaction. Existing anchor revalidation follows passages through ordinary edits; unresolved or orphaned anchors remain visible as their anchor state changes. Comment open/resolved state is independent of anchor integrity. These are personal author annotations, not collaborative tracked changes.

### Alternate manuscript assemblies

Migration 0015 adds `manuscript_assembly` and `assembly_item`. An item references one scene or content unit by stable ID, has an assembly-specific non-negative integer position and inclusion flag, and may not duplicate the same source twice in one assembly. A source may belong to multiple assemblies. Reordering uses a transaction and temporary integer positions to avoid immediate unique-index violations; canonical `scene_placement` is never updated. The application compiles included items in assembly order to a renderer-neutral Compile IR and Markdown export. The terminal command writes derived output under `exports/`. Assembly-level format/front-matter/export settings and a dedicated assembly workspace remain open.

Assembly editing uses project-local JSON drafts under .writers-ide/assembly-drafts/. Each draft records project/assembly identity, stable scene IDs, each scene's expected original SHA-256, and the proposed UTF-8 text. Draft updates use synced temporary files and atomic replacement. Commit preflights every source hash before saving any scene; each changed file is then written through the existing per-scene optimistic journal. If interrupted between files, retry recognizes scene files already at the proposed hash and resumes only the remaining old-hash files. Any third hash is treated as an external conflict: no divergent scene or draft is overwritten. This protocol makes multi-file persistence recoverable, though visibility across files is not atomic during a commit. In the terminal, :assembly-edit <assembly-uuid> opens one continuous buffer with stable, draft-specific scene marker lines; Ctrl-S validates order/identity, atomically persists all proposed segment text as one draft update, then commits through per-scene journals. Broken, missing, or reordered markers prevent all manuscript writes. :assembly-drafts lists pending work and :assembly-resume <draft-uuid> reopens it after interruption. CLI supports draft create/list, text replacement from UTF-8 files and commit/resume.

Optional writing statistics are disabled by default and controlled by writing-stats on|off|report|reset. When enabled, the terminal stores only aggregate Unicode scalar counts for typed versus pasted input and successful saves in .writers-ide/session.json; no text, key sequence, timing, or scene-level history is captured. Counts survive session restoration, can be reset explicitly, and stop changing when disabled.

### Story Bible templates

Reusable templates are TOML files at `templates/story-bible/<template-uuid>.toml`. A template declares its entity kind and stable field IDs, labels, value types, required flags, enum choices, and optional entity-reference target kind. Profiles are separate JSON files under `.writers-ide/story-bible-profiles/`, keyed by entity kind and stable UUID. Saving validates field coverage and value types; enum choices and referenced entity existence are checked before the atomic synced file replacement. These files relocate with the project and remain portable for Git. CLI and terminal palette commands list/import templates and show/import typed profiles. Project search indexing and graphical forms remain open.

### Spatial planning boards

Migration 0016 adds boards, positioned entity cards and directed connectors. Cards can refer to scenes, characters, plotlines, locations, events or objects; SQL triggers validate entity existence and require connector endpoints to belong to the same board. Canvas geometry is presentation metadata and never changes manuscript placement or story chronology. The Planning Boards workspace lists boards and positioned cards; Enter opens a board or follows a scene card, Ctrl+arrows move the selected card while preserving selection by stable item ID, and Esc returns to the board list. CLI and terminal commands can create boards, place typed entities, move cards by coordinates, connect cards and list board contents. The terminal renders coordinate rows rather than a plotted two-dimensional canvas; card resizing and board export remain open. Tests verify scene ordering remains unchanged and direct SQL cannot link cards across boards.

### Single-file archive and restore

`backup_archive_to` first validates the project, creates a SQLite online backup and copied project tree in a sibling staging directory, hashes every regular file into a versioned manifest, writes a ZIP archive, syncs it, then atomically renames it to a destination that must not already exist and must be outside the source project. Restore rejects existing destinations, unsafe paths and non-regular file modes, extracts to a new sibling staging directory, verifies exact manifest membership and SHA-256 hashes, opens/migrates/validates the staged project and checks project identity before publishing the directory by rename. Any failed restore removes the staging tree; the source archive is never modified. The CLI supports `backup-archive` and `restore-archive`.

### Service and validation contracts

`WriterApp` exposes the mention index/confirmation/dismissal, scratchpad, personal comment, assembly, planning board and archive operations; frontends do not issue domain SQL. Acceptance tests cover mention ambiguity and participation separation, confirmed passage drift, dismissed false-positive persistence, scratchpad pin/promotion/content retention, personal comment reanchoring/orphaning, alternate assembly ordering/inclusion without canonical mutation, archive hash validation/relocation, and spatial-board target and connector integrity. The latest storage suite covers migrations from schema versions 4 and 8 through schema version 17.

### Issue #2 work still open

A plotted two-dimensional spatial canvas, resizable board cards and board export remain unimplemented. Mention decorations/previews and broad appearance/backlink views also need a dedicated interaction design. The full-screen editor focus preference is persistent, but is not multi-pane focus/layout management. These features remain tracked against Issue #2; no claim of full issue completion is made by this milestone.

### Multilingual text foundation (Issue #4)

Migration 0017 stores a project language code, defaulting to `en`. Canonical manuscript and editor positions remain UTF-8; terminal cursor columns are now grapheme-cluster indices mapped to byte boundaries through Unicode Segmentation. Word counts use Unicode word boundaries, independent of locale and whitespace. This is a stable baseline, not a claim of fully language-specific tokenization. Language tags are validated and can be set in CLI/TUI; script-specific count policies remain open. Tests cover combining sequences, emoji ZWJ clusters, CJK cursor boundaries, and real insert/delete operations.

### Search and replace foundation (Issue #12)

Search modes share `story_core::find_matches` and `replace_matches`, returning UTF-8 byte ranges and preserving literal text semantics. Extended mode decodes only newline, carriage-return, tab and escaped backslash; regex mode uses the Rust `regex` engine (no look-around/backreferences). CLI and terminal expose current-scene, explicit scene, chapter, and all-scene scope. Every changed scene uses its expected SHA-256 through the durable save journal. Replacement is atomic per scene, not across the complete all-scenes operation; if a later scene conflicts, earlier successful scene saves remain committed and reported. F3/Shift-F3 navigate through every match; scopes include current scene, all scenes, a scene UUID, and a chapter UUID. Preview and transactional multi-scene replacement remain future work.

### Split-window and narrowing foundation (Issue #13)

The terminal editor supports a collection of read-only scene reference buffers, displaying the active buffer alongside the manuscript scene. It stores a stable scene UUID rather than a path, reloads the reference from the project on session restore, and keeps only the scroll offset in session state. Ctrl-Tab switches keyboard focus; the reference pane handles scroll keys without forwarding them into manuscript editing. Layout uses a responsive horizontal/vertical split and the saved ratio. `narrow <chapter-uuid>` filters the scene list without changing scene placements; `widen` reconstructs it from storage. Several simultaneously visible panes, non-scene reference sources and persisted narrow filters remain deferred. A terminal integration test covers adding two buffers, cycling, closing one and restoring the remaining buffer list and selection.

### Mechanical proofing and editor reliability (Issue #18)

`story_analysis::proofread` runs deterministic byte-mapped checks for adjacent repeated words (Unicode-aware case fold), doubled spaces, and spaces before punctuation. Findings are advisory and do not mutate text. CLI and terminal expose current/all-scene reports. The terminal reference pane is loaded from a stable scene ID, read-only, scrollable, and restored from session state; canonical writes still use the Phase 1 SHA-256 optimistic journal. This is a small mechanical-check baseline, not a full copy-editing engine. Dictionaries, annotation UI, and sustained editor stress scenarios remain open.

The terminal editor debounces dirty-buffer persistence by two seconds of inactivity. Autosave invokes the same scene/assembly save service as Ctrl-S, retaining expected-hash checks and journal recovery. A conflict leaves the in-memory buffer dirty and is not retried automatically; the current interface reports the conflict, while richer side-by-side conflict resolution remains open.

A sustained persistence regression test writes a large multilingual scene 24 times, reopens the project at regular intervals, and verifies exact manuscript bytes, stable UUID, SHA-256, and word-count projection. This exercises repeated journal-backed saves but is not a substitute for a long-duration interactive terminal test.

The editor keeps up to 100 recent text/cursor/selection snapshots in memory for Ctrl-Z, Ctrl-Y, and Ctrl-Shift-Z. A new scene clears this transient stack. Saved revision snapshots remain the persistent history; undo buffers are not written to disk. Undo mutations remain dirty and pass through normal autosave and conflict checks.

## Issue #3 — Active challenge feedback

This early feedback increment was superseded by the isolated-session implementation below. Penalties now act only on the challenge buffer and cannot be undone during an active challenge; the normal scene buffer remains separate and unchanged.

## Issue #3 — Isolated timed drafting challenges

Challenge start accepts a custom `challenge-start WORDS SECONDS [pressure|kamikaze] [scene=UUID]` or `challenge-preset short|standard|long [pressure|kamikaze] [scene=UUID]`. Starting requires a clean manuscript buffer and swaps in a fresh isolated draft while retaining the prior editor buffer, cursor, selection, scene identity, content hash and dirty state in memory. Optional destination scene association captures a pre-session snapshot. Challenge edits and Ctrl-S checkpoints write only to `.writers-ide/recovery/challenge-<uuid>.json` using synced temporary replacement and directory synchronisation. Each editor key and paste refreshes the checkpoint; startup lists recoverable drafts. `challenge-save-new DRAFT-UUID CHAPTER-UUID TITLE` uses the normal scene persistence API to create a new scene. `challenge-save-target DRAFT-UUID` explicitly appends to the associated scene using the normal optimistic-concurrency save path. `challenge-discard DRAFT-UUID` removes the recovery draft. Existing manuscript text is never a penalty target.

Kamikaze settings are project-local: `challenge-idle SECONDS`, `challenge-pauses COUNT` (default two), `challenge-delete-rate WORDS` (base rate per penalty step), and `challenge-taunt TEXT` (supports `{words}` and `{deleted}`). The monotonic session state consumes grace pauses first and then removes progressively more trailing words from the isolated buffer on each idle threshold. Deletion uses Unicode grapheme boundaries, clears ordinary undo history so penalties cannot be reversed in-session, and stops/reset on typing. Interface navigation, dialogs, reference focus and terminal resize pause the inactivity clock; the overall duration continues. The footer reports target progress, deadline, grace pauses and idle countdown. `q` while a challenge runs offers checkpoint-and-quit, discard-and-quit, or continue. Success, timeout and quit outcomes record elapsed/retained/deleted words and consumed pauses; Analytics includes challenge history and a Hall of Shame with failure totals and personal best.

Migration 0022 adds outcome, deletion, pause and display-mode fields. Opening a project marks interrupted challenge records as quit; synced recovery drafts remain available through `challenge-list-drafts`. Automated coverage checks two pauses, progressive and Unicode-safe deletion, no undo restoration, scene immunity, save-as-new, discard, interruption recovery, presets, success and quit behavior.

Remaining Issue #3 work: optional continuous-writing streak analytics. Target append is explicit, conflict-aware and covered by a terminal integration test; challenge content is never appended implicitly.

## Configurable diagnostic severity filtering (Issue #10)

CLI and terminal `proofread` reports accept `severity=error,warning,information` to filter the configured project-local findings after rule evaluation. The filter does not change rule configuration or manuscript data. Invalid severity names fail visibly. Rule scoping by chapter/metadata and dictionary-backed spelling remain open.

## Writing streak analytics (Issue #5)

The terminal Analytics workspace derives current and best streaks from persisted writing sessions. A UTC calendar day counts when at least one completed session has a positive net word delta; multiple sessions on the same day count once, and abandoned/zero/negative sessions do not extend a streak. The calculation is a read-only projection and does not change writing-session records.

## Recording selection changes in edit macros (Issue #14)

The data-only macro operation enum now records Shift+Arrow and Shift+Home/End as selection-extension operations. Replay maintains a grapheme-safe anchor/cursor pair, so insertion and deletion replace an active selection while preserving neighboring text. Existing macros retain their serialized operation names and behavior; arbitrary command recording remains open.

## Nested saved-query expressions (Issue #15)

Migration 0023 adds `saved_query_expression`, a project-local JSON predicate associated with a saved query. Existing flat filters remain unchanged and are ANDed with the optional expression. `QueryExpression` supports `Filter`, `All`, `Any`, and `Not`; evaluation compiles validated predicates to parameterized SQL, preserving dynamic collection semantics and manuscript-order results. Empty Boolean groups are rejected, and validation caps expressions at 32 levels and 512 nodes. CLI and terminal `query-expression-set QUERY-ID JSON|clear` operations set or remove an expression through application services. Existing saved queries need no rewrite.

## Format-specific processing recipe exports (Issue #16)

`ProcessingRecipe.exports` declares up to 16 typed `{format, output, design}` targets. Any such target adds the Compile stage to the dependency closure, while preserving the requested analyses. Each target is rendered from the canonical Compile IR and keeps its own output, source map and optional renderer sidecars. Target cache fingerprints include IR, format, design and inclusion mode; cache reuse verifies each artifact hash and rebuilds modified or missing outputs. Targets must use safe, project-relative `exports/` paths with an extension matching the format. Existing recipe files remain valid because `exports` defaults to an empty list.

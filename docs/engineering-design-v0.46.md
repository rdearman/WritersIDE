# Writer's IDE — Engineering Design v0.46

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.45

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

The source-preserving semantic projection and Compile IR are implemented prototypes, not frozen canonical encodings. The repository contains no author-provided formatted chapter, so parser coverage has only been checked against synthetic Markdown fixtures. Before freezing semantic encodings or schema migrations, compare round-trip fidelity against a real formatted chapter and record unsupported constructs. Typed inset representation is still planned for citations, figures/captions, cross-references, index terms, and equations; no raw LaTeX passthrough is introduced. PDF diagnostics and custom document templates remain separate work.

## Issue #7 — Compile format renderers and source maps

`story-document::CompileIr` is the renderer-neutral input. Structural groups are emitted in manuscript order with section/chapter headings, content units retain their declared kind, and each prose segment has a stable source entity ID, source SHA-256, and UTF-8 source range. `CompileIr::validate` rejects duplicate identities, invalid parent references, cycles, and invalid semantic source ranges.

`CompileFormat` supports Markdown, HTML5, EPUB3, DOCX and PDF. Markdown is emitted by the in-process renderer. Other formats use the local Pandoc executable; PDF uses LuaLaTeX and a temporary writable TeX cache. The renderer has an injectable executable path for tests and emits explicit missing-tool/process errors. No renderer performs network calls. Resource paths are rooted in the project directory when compiling project assets.

Application clients call `WriterApp::compile_artifact` so they consume one consistent persisted project snapshot. The CLI and terminal `compile-format` command write output beside a temporary file, synchronize bytes, rename atomically, and synchronize the containing directory. They also write `<output>.source-map.json`. Sidecar offsets are explicitly in UTF-8 Compile IR Markdown, not in renderer-specific output bytes or visual coordinates. Output and its sidecar are separate filesystem objects, so a crash between their writes can leave a completed output without a current sidecar; rerunning Compile safely regenerates both. Canonical source files are never modified.

The compile operation accepts opt-in inclusion expansion through the existing stable-ID expansion path. The default does not expand links. Custom design templates, precise DOCX/PDF location mapping, PDF publishing diagnostics, and advanced LuaLaTeX workflows are addressed by the Issue #21 foundation below and remain open for completion.

Acceptance tests cover IR hierarchy and source ranges, Markdown output without external tools, HTML/EPUB/DOCX artifacts through a locally available Pandoc, PDF through Pandoc and LuaLaTeX where available, explicit renderer failures, app-level source-map identity, and stable source text after compilation. External renderer integration tests are capability-gated; they do not claim format support in environments that lack those tools.

## Issue #21 — LuaLaTeX and publishing foundation

`BookDesign` is a typed renderer input with fixed built-in presets (`TradePaperback`, `Manuscript`, `Nonfiction`). Pandoc arguments come from compile-owned constants, not user-supplied shell fragments. The preset only applies to PDF output; Markdown/HTML/EPUB/DOCX are unaffected. LuaLaTeX receives a temporary writable `TEXMFVAR`, `TEXMFCACHE`, and `TEXMFCONFIG` to support restricted/local environments without writing into the user profile. Missing engines, unavailable fonts/packages, or renderer failures produce errors with Pandoc's stderr.

`publication_diagnostics` is a deterministic read-only check over Compile IR. Current codes: `PUB001` missing project title (warning), `PUB002` no content units/scenes (warning), and `PUB003` an empty content unit (information). The CLI `compile-check <project>` emits these records as JSON. A diagnostic is a prompt for author review; it cannot alter metadata or manuscript files.

The presets and checks establish the renderer contract but are not complete publishing design support. Custom project templates, configurable font and typographic profiles, running heads, page numbering, front/back matter treatments, print proofing and comprehensive publishing diagnostics are still open Issue #21 requirements. PDF page-coordinate source mapping is also not implemented.

## Issue #6 — Manuscript import staging and application

`story-import::inspect_manuscript` fingerprints the original file before parsing. DOCX and ODT archives are read from their relevant XML document members; RTF control words and basic Unicode escapes are decoded; EPUB spine resources are extracted in reading order. The inventory is represented as `ManuscriptStaging` with a deterministic source fingerprint, source path, source-unit IDs, titles, extracted Markdown text, warnings, and unsupported-format notes. Inspection is read-only. It is a text recovery layer; it does not claim complete rich-text fidelity.

The CLI separates `inspect-manuscript` from `import-manuscript`. The latter requires a destination that does not already exist, creates a project, imports each staging unit to a chapter and scene through the application façade, and writes an import report. Provenance rows retain each source unit's raw fields and map source title/text fields to the new scene. Any failure after target creation removes that newly created destination only; it never alters the source file or an existing project. A rerun creates a fresh project and does not merge/duplicate into a prior target. The database still protects repeated source fingerprints from silent import reuse.

Current limitations: imported document structure is reduced to inferred chapter units, paragraph prose is retained as text, and unsupported tables/media/tracked changes/styles are reported rather than recovered. DOCX, ODT, RTF, and EPUB have synthetic fixtures only. Scrivener comment markup and interactive staging/mapping remain separate work.

## Issue #19 — Editorial three-way merge foundation

The pure `story_import::three_way_merge(base, local, external)` API uses line-preserving LCS edits to combine non-overlapping changes. It returns `None` for the proposed merged text when overlapping edits exist and reports base line ranges plus each version's conflict text. No conflict markers are inserted into manuscript prose. Large LCS inputs are bounded; the algorithm uses a whole-document conservative edit if the DP matrix would exceed four million cells. `WriterApp::merge_review_text` loads the latest scene, merges, and saves a conflict-free result through the existing optimistic journal; conflicts do not write. CLI and terminal routes accept explicit base and reviewed-text files. The TUI refreshes the open buffer after a successful save.

`inspect_docx_review` separately inventories DOCX insertion/deletion text plus comment text, author, date and comment ID. CLI `review-inspect` emits that staging data. The regular manuscript import still reports these features because it does not merge them into canonical scene text. This is not yet a full review UI: markup is not yet anchored to source passages or accepted as per-change edits. Conflict resolution is all-or-nothing per scene; interactive hunk acceptance and three-way conflict editing remain open. Tests exercise synthetic Word XML review records, independent line edits, overlapping edits, unchanged source after conflicts, and journaled app saves.

## Issue #17 — Local browser planning and read-only review foundation

`story-web::BrowserServer` binds a `TcpListener` only to `127.0.0.1`; a user can SSH-forward its port without exposing a listening socket on external interfaces. It serves an embedded static page and a JSON planning projection derived from `WriterApp`/Compile input. GET scene endpoints provide read-only prose review; POST routes create chapters/scenes, update scene synopsis/status, and move scene placement through the same application services as terminal actions. The planning projection includes placements, characters, plotlines, events, locations, and typed story-link edges by stable ID. Requests are size-limited, GET/POST-only, host/origin-checked, and responses include nosniff/CSP headers. Route tests cover planning mutation, unchanged prose, invalid IDs, and loopback address checks.

This is not full browser planning parity. Browser-side editing of characters, plots, events, locations, links, issues, research and queries; all terminal workspaces; authentication for non-loopback deployments; and interactive graph layout remain open. The server must remain loopback-only until an authenticated access model is designed.

## Issue #10 — Configurable deterministic diagnostics

`ProofreadingConfig` is a versioned project-local TOML file at `.writers-ide/diagnostics.toml`. It defines stable rule IDs, enablement, severity, messages, and a matcher. Built-in matcher kinds preserve repeated-word, double-space and space-before-punctuation behavior; `regex` uses Rust's linear-time regex engine and reports UTF-8 byte ranges. Configuration validation rejects duplicate/empty IDs, unknown matchers, invalid severity and invalid expressions. Missing files select the built-in default; malformed files surface a visible error rather than falling back silently. `proofread-config` creates the initial configuration with create-new semantics. Findings are read-only. Additional consistency/publishing rule families, dictionary-based spelling, and scope filters remain open.

## Issue #1 and Issue #9 status

Stable-ID typed links, anchors, backlinks, broken-link scans, cycle-safe inclusions, Reading annotations/indexes, approved evidence-linked knowledge, signed integer chronology, uncertain event and scene ranges, and structured character/plot/location/event relationships are implemented in the preceding versioned design sections and existing migrations. Current acceptance is primarily exercised in `story-storage`, `story-app`, and terminal regression tests. This design revision does not replace those storage contracts.

## Phase 5 — Optional enhancements implementation status

### Issue #14 — Editing power tools foundation

`story-core` now provides UTF-8-safe snippet expansion with named value substitution and a cursor marker, multi-cursor insertion at stable original byte offsets, and data-only edit macro operations for insertion, replacement and case conversion. The terminal command registry exposes `snippet`, `snippet-insert`, `register-save`, `register-insert`, `cursor-add`, `cursor-clear`, `multi-insert`, `macro`, `macro-define`, and `macro-run`; macro changes are undoable. Named snippets, macros and registers persist in `.writers-ide/editor-tools.json`; native extra cursors can be positioned by zero-based grapheme row/column, move with editor navigation, and edit/delete text simultaneously. Recording macros from arbitrary key events and register rotation remain open.

### Issue #5 — Writing challenges foundation

Migration 0018 adds durable writing challenge records with target words, start/deadline, `steady` or explicitly requested `kamikaze` mode, completion status, and words written. App services validate and record start/completion; CLI commands are `challenge-start`, `challenge-finish`, and `challenge-list`. The terminal command palette provides an integrated countdown, records the open editor's word delta on finish or timeout, and can abandon a session. Explicit Kamikaze mode deletes the previous word after ten seconds without typing; the deletion is undoable and the normal journaled save remains in effect. Challenge history acts as a basic event ledger. Analytics renders bounded progress bars for the project target, chapter targets, and each recorded challenge. Historical daily aggregation and per-scene time-series graphs remain open.

### Issue #15 — Advanced inspection foundation

Migration 0019 persists typed count assertions on saved queries. CLI `query-assert-save`/`query-assert-check` and terminal `query-assert-save`/`query-assert-check` save and evaluate assertion sets against refreshed results. The analysis crate also evaluates read-only character/plotline scene-coverage requirements; CLI `coverage` checks a minimum appearance count. Checks report actual counts and return a failing status when unmet. Coverage configuration UI, arbitrary query expression evaluation, and broader assertion operators remain open.

### Issue #16 — Compile automation foundation

CLI `compile-watch <project> <format> <output> [poll-ms]` fingerprints project metadata, SQLite database and manuscript files, compiling on initial launch and after changes. Versioned `EditionRecipe` TOML adds `compile-edition` and `watch-edition` with format, project-relative output, design, transclusion policy, language and optional personal dictionary. Output paths reject traversal and symlink escapes; compile failures preserve the previous output. Recipes can invoke local Aspell spellchecking, filter candidates through a project-relative personal word list, and emit a separate JSON report without changing source prose. `spellcheck <project> <language> [wordlist]` exposes the same check directly. Aspell is an optional local executable, and no network is used. Existing manuscript assemblies provide alternate edition membership/order. Broader processing-stage automation recipes, watch debounce tuning and integrated terminal controls remain incomplete.

### Issue #3 — Timed drafting status

The terminal supports timed word-target sessions with steady mode and an explicit Kamikaze option. The countdown is visible in the footer; expiry records the word delta. Kamikaze deletes the previous word after ten seconds without typed input, supports Undo, and never runs unless selected when starting the session. The mode is stored durably. Configuration of inactivity threshold and richer challenge modes remains open.

Phase 5 has not completed every feature in the issue roadmap. Recording macros from key events, register rotation, project-wide cumulative writing graphs, coverage configuration UI, broader dictionary formats/management, and multi-stage pipeline automation remain deferred. The shipped APIs and schema are foundations, not substitutes for those remaining acceptance criteria.

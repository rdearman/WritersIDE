# Writer's IDE — Engineering Design v0.33

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.32

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

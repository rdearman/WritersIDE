# Writer's IDE

Writer's IDE is an early Rust implementation of the scene-centred novel planning and writing environment described in the supplied SRS v0.1 and [active engineering design](docs/engineering-design-v1.85.md).

The current workspace contains a reusable story core, SQLite and manuscript storage, application services, import staging, a CLI and a full-screen terminal prototype. Experimental Scrivener inventory, explicit mapping preview, staged editable/Both import, and separate series databases with CLI book creation are available. Import has been exercised against a real author-provided Scrivener for Windows 3.1.1.0 project (excluded from Git), as well as a legally licensed small Scrivener for Mac 3.1.4 fixture; a Scrivener 2.5 fixture is metadata-only. This limited evidence does not establish broad Scrivener compatibility or a general supported-version claim. See the [Issue #35 fidelity report](docs/scrivener-import-fidelity-issue-35.md) for verified behavior and known loss. Ranperen compatibility remains unverified because the reference spreadsheets are not in the repository.

The WYSIWYM parser and renderer-neutral Compile IR preserve canonical Markdown and now pass a real imported-manuscript semantic/source-map check for the supported subset. See [the Issue #21 contract](docs/wysiwym-contract-v0.3.md); specialized inset syntax and complete publication-renderer acceptance remain open.

The current implementation adds stable typed story links, passage anchors, backlink and integrity checks, cycle-checked inclusion of scenes, research documents and imported Reading references, live saved scene queries, project-wide FTS5 search, a full-screen terminal shell, reference reading and annotations, snapshot revision history with interactive hunk restoration, read-only narrative analytics, optional local Ollama assistance with evidence-linked proposal review and dismissal, and an incremental processing pipeline. Per-book chronology defaults to disabled; authors can enable story-time views and diagnostics, choose inclusive or exclusive interval boundaries, and opt into non-blocking missing-date reminders. Independent events can record character roles and locations separately from their scene references. The terminal command registry drives the command palette and hierarchical menu through a shared dispatcher, including guided series setup, Scrivener preview/import, new-book creation and read-only series references. User and project command keymaps load with project override precedence, nonfatal malformed-file warnings, and registry-backed effective shortcut labels. Project book types distinguish fiction from nonfiction, which can contain direct chapter prose as portable content units without synthetic scenes; the terminal edits these units in the shared manuscript editor and the processing Compile IR includes them. Named project Compile Configurations provide submission, e-book, print, proof, plain text and Markdown profiles with manifest-tracked outputs, collision protection, publication metadata overrides, project-local EPUB covers and confirmed managed-only cleanup. Compile diagnostics support named severity profiles, per-rule/group overrides, warning promotion, and persistent rule/scene/finding suppressions with expiry, provenance, stale revalidation and explicit revocation. Versioned declarative TOML templates can be exported, validated and instantiated locally. Local Git snapshots pair a journal-barrier-consistent deterministic SQL metadata dump with canonical project files; CLI and terminal workflows now include previewed in-place restore with safety snapshots, per-file durable recovery, history, diffs, interval snapshots and optional on-exit snapshots, with optional explicit Git remotes; local operation never requires one. The writing workflow includes keyboard mention previews, a resumable cross-scene assembly editor, spatial planning boards and persistent quick capture. Language-aware typography previews offer optional, non-destructive French and Italian suggestions, with the full multilingual acceptance scope still in progress. Split panes adapt on narrow or short terminals by temporarily showing the focused pane or only the active reference, without discarding the saved layout. See [SRS v0.2 addendum](docs/software-requirements-v0.2-addendum.md) and [active Engineering Design](docs/engineering-design-v1.85.md).

## Build and verify

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The workspace builds offline after dependencies have been cached. Runtime operation does not require network access.

## CLI

```text
writers-ide create <path> [title]
writers-ide inspect <path>
writers-ide chapter <path> <title>
writers-ide scene <path> <chapter-uuid> <title>
writers-ide show <path> <scene-uuid>
writers-ide write <path> <scene-uuid> <text>
writers-ide resolve <path> <transaction-uuid> external|prepared
writers-ide backup <path> <destination>
writers-ide search <path> <terms>
writers-ide query-save <path> <name> [status=...|character=UUID|plotline=UUID|before=TICK|after=TICK|issue=unresolved]...
writers-ide query-run <path> <query-uuid>
writers-ide link <path> <source-kind> <source-id> <target-kind> <target-id> <link-kind> [label]
writers-ide backlinks <path> <entity-kind> <entity-id>
writers-ide broken-links <path>
```

The current terminal prototype is launched with `cargo run -p writers-ide-terminal -- <project-path>`. It uses a full-screen Ratatui/Crossterm alternate screen; F1–F9 and F11–F14 switch workspaces, Ctrl+/ or F10 opens the hierarchical menu; Tools → Keybinding Manager provides searchable bindings, context/scope switching, sequence capture, conflict confirmation, unbinding, and scope reset, `/` searches, `:` opens the command palette, Enter opens the selected scene or reference, and Ctrl-S saves manuscript text. The Reading workspace accepts `:read-import <path>`, passage selection with `v` then `a` (highlight) or `n` (annotate), `b` for a bookmark, `c` for a checkpoint, and `l` to focus the reading pane. Revisions provides `:snapshot <label>`, `:history`, `:diff <snapshot-uuid>`, and `:restore <snapshot-uuid>`. Read-only analysis includes character co-appearance counts, subplot gaps, uncertainty-aware chronology flags and word-budget redistribution previews in the CLI (`analytics <project>`) and Analytics workspace. `process <project>` in the CLI or `:process` in the terminal runs the incremental local compile, analysis, diagnostic, export, and revision-summary pipeline; `process <project> --expand-transclusions` or `:process-expand` opts into derived scene inclusion during compilation without changing canonical scene files. CLI `--only export` and terminal `:process-only export` run Compile plus Export and clearly report other stages as not selected; selected stages can be comma-separated, and `--expand-transclusions` composes with selection. Optional local Ollama commands are available in the CLI: `extract <project> <scene-uuid> <model>` writes provisional evidence-linked proposals; `study <project> scene|reading <source-uuid> <model> [--questions]` returns a summary and explanation with optional comprehension questions. They contact only a localhost endpoint when explicitly invoked. `approve-extraction <project> <proposal.json> <claim-index>` explicitly promotes one current evidence-checked claim into searchable approved knowledge; `knowledge <project>` lists approved claims and provenance. Use `proposals <project>` to review proposal claims and their source evidence, and `dismiss-extraction <project> <proposal.json> <claim-index>` to dismiss one without promoting it. In the terminal, `:proposals` opens a scrollable review list; `:dismiss <proposal-path> <claim-index>` and `:approve <proposal-path> <claim-index>` record explicit decisions. Approved claims also appear in the Story Bible. Promotion rejects changed source text and never edits manuscript prose.

Each project is a directory containing one `story.db`, `project.toml`, portable Markdown files under `manuscript/`, and application recovery data under `.writers-ide/recovery/`. Close the project before copying it manually; use `backup` for a consistent application-managed copy.

Terminal function-key targets can be overridden in `.writers-ide/keybindings.toml`, for example `[bindings]` with `F1 = "Analytics"` and `F4 = "Reading"`; unspecified keys retain their defaults.

The terminal command palette can run `extract scene|reading <UUID> <model> [endpoint]` to save an unapproved evidence-linked proposal. The optional Ollama client only accepts localhost HTTP endpoints; the model request runs only after the author invokes it.

Saved scene queries can be created and listed in the terminal command palette with `query-save <name> status=idea` and `query-list`; `query <uuid>` opens current results in the Search workspace. The CLI provides `query-save`, `query-list`, and `query-run`.

Project-wide full-text search is available as `:search-all <terms>` in the terminal or `writers-ide search-all <project> <terms>` in the CLI. Results include scene prose and metadata, character/location/plotline names, event text, research notes, issues, and imported reading materials. `writers-ide rebuild-search <project>` reconstructs the FTS5 index from its canonical local search projection.

The terminal outline supports `section <title>`, `chapter <title>`, `subchapter <section-uuid> <title>`, `scene <chapter-uuid> <title>`, `move <scene-uuid> <chapter-uuid> <position>`, `rename <scene-uuid> <title>`, and `scene-status <scene-uuid> <status>`. The command palette also creates basic entity records with `character <name>`, `plotline <name>`, `issue <title>`, and `research <title> | <body>`.

Scene planning uses `scene-field <scene-uuid> <synopsis|purpose|goal|outcome> | <text>`, `character-link <scene-uuid> <character-uuid> <pov|major|minor|off_camera|mentioned>`, `event <title> | <story-time>`, `event-link <scene-uuid> <event-uuid> [relationship]`, and `issue-link <scene-uuid> <issue-uuid>`. Plotline membership uses `plotline-link <scene-uuid> <plotline-uuid> <overlap|equal> [role]`; author-defined allocations use `plotline-shares <scene-uuid> <plotline-uuid>=<basis-points>...` and must total 10,000.

Issue statuses can be changed with `issue-status <issue-uuid> <open|in_progress|resolved|deferred|closed>` in the terminal or `writers-ide issue-status <project> <issue-uuid> <status>` in the CLI.

The manuscript editor accepts shifted/uppercase input, Shift+Arrow selection, and internal clipboard operations with Ctrl-C/Ctrl-X/Ctrl-V. Horizontal arrows cross line boundaries. Clipboard contents are local to the running application.

The terminal command palette also accepts `link <source-kind> <source-id> <target-kind> <target-id> <related|cites|transcludes|includes|navigates> [label]`, `outgoing <kind> <uuid>`, `backlinks <kind> <uuid>`, `broken-links`, and `transclude <scene-uuid>`. CLI `outgoing` lists links from an entity.

After scene text changes, passage anchors are automatically relocated only when the quote and context identify one unambiguous location. Use `:anchors` to inspect anchor status and `:anchor-move <anchor-uuid> <start-byte> <end-byte>` to resolve an ambiguous or broken anchor manually. The CLI supports `anchors <project> <scene-uuid>` and `anchor-move <project> <anchor-uuid> <start-byte> <end-byte>`.

In the Reading workspace, `:annotations` and `:bookmarks` show zero-based indices. Use `:annotation-jump <index>` or `:bookmark-jump <index>` to return to a stored passage without changing the source document.

Reading references expose their story links with `reading-links`; `reading-follow <index>` navigates to the linked scene or switches to the character, plotline, or research workspace.

The SRS and engineering design are the product baselines. The SRS was supplied in the project brief and is not yet present as a repository file. No Ranperen spreadsheets or substantial multi-chapter Scrivener fixture are present, so broad importer compatibility has not been claimed.


`writers-ide browse <project> [port]` starts the local loopback browser for planning and read-only review. Its header links to a standalone HTML Compile preview (`/preview`), generated on request through local Pandoc without changing manuscript files. The preview blocks scripts and remote resources.

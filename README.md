# Writer's IDE

Writer's IDE is an early Rust implementation of the scene-centred novel planning and writing environment described in the supplied SRS v0.1 and [active engineering design](docs/engineering-design-v0.16.md).

The current workspace contains a reusable story core, SQLite and manuscript storage, application services, a synthetic import inventory/mapping interface, a CLI and a full-screen terminal prototype. Real Scrivener/Ranperen adapters are not present.

The current implementation adds stable typed story links, passage anchors, backlink and integrity checks, cycle-checked scene inclusion, live saved scene queries, project-wide FTS5 search, a full-screen terminal shell, reference reading and annotations, snapshot revision history, read-only narrative analytics, optional local Ollama assistance, and an incremental processing pipeline. See [SRS v0.2 addendum](docs/software-requirements-v0.2-addendum.md) and [Engineering Design v0.24](docs/engineering-design-v0.24.md).

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

The current terminal prototype is launched with `cargo run -p writers-ide-terminal -- <project-path>`. It uses a full-screen Ratatui/Crossterm alternate screen; F1–F14 switch workspaces, `/` searches, `:` opens the command palette, Enter opens the selected scene or reference, and Ctrl-S saves manuscript text. The Reading workspace accepts `:read-import <path>`, passage selection with `v` then `a` (highlight) or `n` (annotate), `b` for a bookmark, `c` for a checkpoint, and `l` to focus the reading pane. Revisions provides `:snapshot <label>`, `:history`, `:diff <snapshot-uuid>`, and `:restore <snapshot-uuid>`. Read-only analysis includes character co-appearance counts, subplot gaps, uncertainty-aware chronology flags and word-budget redistribution previews in the CLI (`analytics <project>`) and Analytics workspace. `process <project>` in the CLI or `:process` in the terminal runs the incremental local compile, analysis, diagnostic, export, and revision-summary pipeline. Optional local Ollama commands are available in the CLI: `extract <project> <scene-uuid> <model>` writes provisional evidence-linked proposals; `study <project> scene|reading <source-uuid> <model> [--questions]` returns a summary and explanation with optional comprehension questions. They contact only a localhost endpoint when explicitly invoked. `approve-extraction <proposal.json>` records review but does not promote claims into canonical facts.

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

Reading references expose their story links with `reading-links`; `reading-follow <index>` navigates to the linked scene or switches to the character, plotline, or research workspace.

The SRS and engineering design are the product baselines. The SRS was supplied in the project brief and is not yet present as a repository file. No Ranperen spreadsheets or Scrivener fixtures are present, so importer compatibility has not been claimed.

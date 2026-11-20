# Writer's IDE

Writer's IDE is an early Rust implementation of the scene-centred novel planning and writing environment described in the supplied SRS v0.1 and [engineering design](docs/engineering-design-v0.2.md).

The current workspace contains a reusable story core, SQLite and manuscript storage, application services, a synthetic import inventory/mapping interface, a CLI and a full-screen terminal prototype. Real Scrivener/Ranperen adapters are not present.

The current link/query slice adds stable typed story links, passage anchors, backlink and integrity checks, cycle-checked scene inclusion, live saved scene queries, and FTS5 scene search. The terminal prototype includes scene editing, reference reading for text/Markdown/EPUB, independent annotations and indexes, plus scene snapshot history and restore operations. See [SRS v0.2 addendum](docs/software-requirements-v0.2-addendum.md) and [Engineering Design v0.5](docs/engineering-design-v0.5.md).

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

The current terminal prototype is launched with `cargo run -p writers-ide-terminal -- <project-path>`. It uses a full-screen Ratatui/Crossterm alternate screen; F1–F14 switch workspaces, `/` searches, `:` opens the command palette, Enter opens the selected scene or reference, and Ctrl-S saves manuscript text. The Reading workspace accepts `:read-import <path>`, passage selection with `v` then `a` (highlight) or `n` (annotate), `b` for a bookmark, `c` for a checkpoint, and `l` to focus the reading pane. Revisions provides `:snapshot <label>`, `:history`, `:diff <snapshot-uuid>`, and `:restore <snapshot-uuid>`. Read-only analysis includes character co-appearance counts, subplot gaps, uncertainty-aware chronology flags and word-budget redistribution previews in the CLI (`analytics <project>`) and Analytics workspace.

Each project is a directory containing one `story.db`, `project.toml`, portable Markdown files under `manuscript/`, and application recovery data under `.writers-ide/recovery/`. Close the project before copying it manually; use `backup` for a consistent application-managed copy.

The SRS and engineering design are the product baselines. The SRS was supplied in the project brief and is not yet present as a repository file. No Ranperen spreadsheets or Scrivener fixtures are present, so importer compatibility has not been claimed.

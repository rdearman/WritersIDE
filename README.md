# Writer's IDE

Writer's IDE is an early Rust implementation of the scene-centred novel planning and writing environment described in the supplied SRS v0.1 and [engineering design](docs/engineering-design-v0.2.md).

The current workspace contains a reusable story core, SQLite and manuscript storage, application services, a synthetic import inventory/mapping interface, and a small CLI. It does not yet include the terminal editor or real Scrivener/Ranperen adapters.

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
```

Each project is a directory containing one `story.db`, `project.toml`, portable Markdown files under `manuscript/`, and application recovery data under `.writers-ide/recovery/`. Close the project before copying it manually; use `backup` for a consistent application-managed copy.

The SRS and engineering design are the product baselines. The SRS was supplied in the project brief and is not yet present as a repository file. No Ranperen spreadsheets or Scrivener fixtures are present, so importer compatibility has not been claimed.

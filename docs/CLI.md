# WIDE command-line reference

The `writers-ide` command is a development and automation interface. WIDE’s
terminal menus are intended for ordinary authoring; CLI operations that accept
UUIDs are not required for normal writing.

Run the current binary’s complete usage output with:

```sh
cargo run -p writers-ide -- --help
```

## Create and inspect a project

```sh
cargo run -p writers-ide -- create-template ./MyNovel novel "The Lantern House"
cargo run -p writers-ide -- inspect ./MyNovel
```

Built-in templates include `blank-fiction`, `novel`, `short-story`,
`blank-nonfiction` and `nonfiction-book`. The `content` and `write-content`
commands support chapter-level prose and sections in non-fiction projects:

```sh
cargo run -p writers-ide -- content ./MyNovel chapter - "A First Chapter"
cargo run -p writers-ide -- read-content ./MyNovel CONTENT-UUID
cargo run -p writers-ide -- write-content ./MyNovel CONTENT-UUID "Draft prose"
```

The CLI prints identifiers for newly created records. Use the terminal
application to create and organise content without entering identifiers.

## Inspect and import manuscripts

```sh
cargo run -p writers-ide -- inspect-scrivener /path/to/Book.scriv
cargo run -p writers-ide -- import-scrivener /path/to/Book.scriv ./ImportedBook \
  --book-type=fiction
cargo run -p writers-ide -- inspect-manuscript /path/to/manuscript.docx
```

Scrivener inventory and import are experimental. Import behaviour, supported
source features and known limitations are described in the [Scrivener fidelity
report](scrivener-import-fidelity-issue-35.md). Inspect a source and review
warnings before relying on an import.

## Back up and compile

```sh
cargo run -p writers-ide -- backup-archive ./MyNovel ./MyNovel-backup.zip
cargo run -p writers-ide -- restore-archive ./MyNovel-backup.zip ./RestoredNovel
cargo run -p writers-ide -- compile ./MyNovel epub ./exports/MyNovel.epub
cargo run -p writers-ide -- compile ./MyNovel docx ./exports/MyNovel.docx
```

The available compile formats are shown by `writers-ide --help` in the current
build. Inspect generated books before distribution; publication fidelity is
still being validated.

## Regression tools

See the [development guide](DEVELOPMENT.md) for build and workspace checks and
the [real-manuscript regression guide](REAL_MANUSCRIPT_REGRESSION.md) for the
private local corpus. Private fixtures and their generated publications must
remain outside version control.

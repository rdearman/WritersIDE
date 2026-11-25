# WIDE development guide

This guide is for contributors working on the Rust source tree. WIDE is
pre-release software; commands below describe the current repository and may
change as the application is integrated.

## Build and run

Install a stable Rust toolchain. From the repository root:

```sh
cargo build --workspace
cargo run -p writers-ide-terminal
```

To open an existing project directly:

```sh
cargo run -p writers-ide-terminal -- /path/to/project
```

Cargo may need network access to fetch dependencies the first time. Once the
dependencies are cached, `--offline` can be used for builds and tests. Normal
application use is local; optional Ollama features require an explicitly
configured local service.

## Verification

The routine workspace checks are:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Target a crate or integration suite with `cargo test -p <crate-name>`; the
terminal package also has author-workflow and PTY integration tests. Some tests
exercise local socket binding. If a sandbox blocks that operation, rerun the
specific test with the required local permission and report the limitation;
the test does not need an external service.

### Independent verification tools

These are development-time checks, not application runtime dependencies. Their
availability and versions were checked on the development host on 25 November
2026. Installed language dictionaries are shown separately from the spell
checker version.

| Tool | Verified version | Use |
| --- | --- | --- |
| EPUBCheck | 4.2.6 | Validate EPUB packages independently of WIDE |
| Aspell | 0.60.8.1 | Spelling checks; English, French and Italian dictionaries are installed |
| Hunspell | 1.7.2 | Spelling checks; `en_GB`, `en_US`, French, Italian, German, Spanish, Portuguese and Russian dictionaries are installed |
| qpdf | 11.9.0 | Inspect PDF structure and integrity |
| Poppler (`pdftotext`, `pdfimages`, `pdfinfo`) | 24.02.0 | Inspect PDF text, images and metadata |
| SQLite CLI | 3.45.1 | Run database integrity and schema checks |
| xmllint | libxml2 2.9.14 | Inspect and validate XML documents |
| unzip | 6.00 | Inspect archives and EPUB package contents |
| cargo-audit | 0.23.0 | Check Rust dependencies against the advisory database |
| cargo-deny | 0.22.1 | Review advisories, licences, sources and dependency policy |
| cargo-machete | 0.10.1 | Find unused Cargo dependencies |
| cargo-llvm-cov | 0.9.2 | Measure Rust test coverage |

On this host `/usr/bin/epubcheck` is a launcher that cannot find its interpreter
inside the restricted development shell. The installed checker itself works
through `java -jar /usr/share/java/epubcheck.jar`; the private regression
harness detects and uses that JAR when needed. The interactive dictionary
selection should still be checked against each passage’s language metadata;
the presence of dictionary files alone does not verify WIDE’s language choice.

Availability is not the same as a successful audit. Run a tool at an appropriate
checkpoint and record its result. Do not automatically upgrade dependencies or
make any of these tools a mandatory runtime requirement.

Latest local audit checkpoint (25 November 2026): `cargo audit` loaded 1,568
RustSec advisories and reported 26 advisories plus 10 allowed warnings across
the lockfile, including transitive SQLite, URL, ZIP and zstd dependencies. No
dependency was automatically upgraded. The affected dependency paths and
exposure need deliberate compatibility and security review before release.
`cargo deny check` could not finish because locked platform-specific crate data
was unavailable from the network-restricted Cargo registry. `cargo-machete`
found two unused direct dependencies, which were removed; a repeat check passed.
`cargo llvm-cov` could not run because the `llvm-tools-preview` Rust component
was unavailable and could not be installed in this environment. These blocked
checks are not passes.

Examples:

```sh
cargo audit
cargo deny check
cargo machete
cargo llvm-cov --workspace --all-features --summary-only
sqlite3 path/to/project/story.db 'PRAGMA integrity_check;'
java -jar /usr/share/java/epubcheck.jar path/to/book.epub
aspell --lang=en_GB list < manuscript.txt
hunspell -d en_GB manuscript.txt
qpdf --check book.pdf
pdfinfo book.pdf
pdftotext book.pdf -
xmllint --noout document.xhtml
unzip -t book.epub
```

Run the real private-manuscript suite only when the local fixtures are present.
Its procedures, privacy rules and current findings are documented in the
[regression guide](REAL_MANUSCRIPT_REGRESSION.md). That suite is not a public CI
dependency; public tests use synthetic fixtures.

## Command-line tools

The workspace provides the `writers-ide` CLI for project inspection, content
operations, imports, backups and compilation. See the [CLI reference](CLI.md)
for common developer workflows. For the complete command list from the current
build, run:

```sh
cargo run -p writers-ide -- --help
```

The CLI is an engineering and automation interface. Authors should use the
terminal application’s menus and workspace controls for ordinary writing.

## Architecture and specifications

- [Software requirements addendum](software-requirements-v0.2-addendum.md)
- [Engineering Design v0.2 baseline](engineering-design-v0.2.md)
- [Latest recorded engineering addendum](engineering-design-v2.43-addendum.md)
- [WYSIWYM semantic document contract](wysiwym-contract-v0.3.md)
- [Current development handover](DEVELOPMENT_STATUS.md)

The addendum series records later decisions on top of earlier design documents;
it has not yet been consolidated into one current engineering specification.

## Workspace map

| Area | Responsibility |
| --- | --- |
| `crates/story-core` | Domain types and validation |
| `crates/story-storage` | SQLite, project files, persistence and recovery |
| `crates/story-app` | Application services and author workflows |
| `crates/story-import` | Import inventory, mapping and extraction |
| `crates/story-document` | Semantic document model and compilation |
| `apps/cli` | Command-line engineering interface |
| `apps/terminal` | Ratatui/Crossterm application |
| `tools` | Regression and development harnesses |

Keep domain behaviour independent of the terminal interface. Do not commit
private manuscript fixtures, generated publications or content-bearing reports.

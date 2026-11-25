# Writer's IDE Development Status

Updated: 2026-11-25

This is the handover note for ongoing implementation. GitHub issues remain the
authoritative source for approved requirements and human UAT findings.

## Priorities

1. Protect manuscript data and fix regressions that block ordinary writing.
2. Consolidate Outline and Scene Cards into the one hierarchical outliner
   approved in #55, coordinated with #20 and #68.
3. Continue the terminal word processor work in #61 and the remaining UAT
   workflow corrections under #44.
4. Complete import, publishing, recovery and other approved work in dependency
   order. The feature freeze in #44 remains in force.

## Recent verified commits

- `1d2e165` — structural removal Undo/Redo outside the manuscript editor.
- `4519221` — Tools > Writing Statistics opens the author-facing dashboard.
- `97b8878` — safe sentence deletion and visible editor caret after selection.

All three commits are pushed to `origin/master`.

## Current editor regression work

Commit `97b8878` addresses UAT-19 and UAT-20 in #61:

- Plain arrow navigation clears Select All while preserving editor focus and
  displays the insertion point without changing manuscript content.
- Ambiguous unpunctuated multiline Delete Sentence falls back to the current
  line; a punctuated sentence on its own line removes only that sentence line.
- Both deletion paths use the existing undoable source-range replacement.

The full terminal unit suite (329 tests), author workflows (3 tests), and a
standalone serialized Linux PTY run (19 tests) passed. Formatting and workspace
Clippy passed. A full workspace run passed unit and crate tests but had
intermittent PTY startup/menu timing failures; the PTY suite passed when run
independently. Run the full workspace suite again after the next implementation
batch, and investigate any repeated PTY failure before claiming the gate.

## Current architectural finding

The terminal still exposes `Workspace::Outline` and `Workspace::Cards` as
separate workspaces. GitHub #55 was amended on 25 November to supersede the
separate-outliner requirement: one hierarchical tree must serve fiction and
non-fiction, with optional card/column presentations sharing the same canonical
selection and ordering. Existing data/services already represent sections,
chapters, scenes and direct prose content units. The next implementation task
is to unify the author-facing workspace/projection without duplicating those
records or changing storage unless an audit proves it necessary. Preserve
Included/Excluded and Unplaced distinctions, and retain existing recovery and
compile behavior.

## Verification commands

```sh
cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline -p writers-ide-terminal --bin writers-ide-terminal
cargo test --offline -p writers-ide-terminal --test author_workflows
cargo test --offline -p writers-ide-terminal --test pty_smoke -- --test-threads=1
cargo test --offline --workspace -- --test-threads=1
```

The full workspace includes an Ollama test that binds a loopback-only local
socket. Run that test with the user-authorized socket permission if the sandbox
denies binding; it does not require an external service or network access.

## Working tree notes

At the last check, `.gitignore` had an unrelated user change and `UAT/` was
untracked/user-owned. Do not stage, modify or remove either as part of unrelated
implementation work.

## Human acceptance

Automated coverage is not human usability acceptance. Keep author-facing issues
open when UAT is required, including #44, #55, #61 and #63. Do not treat this
status note as an issue closure or as evidence of human UAT.

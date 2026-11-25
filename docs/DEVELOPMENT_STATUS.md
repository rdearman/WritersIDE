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

The full workspace suite passed for the latest batch: terminal unit tests
333/333, author workflows 3/3 and serialized Linux PTY tests 20/20, along with
the workspace crate and doc tests. Workspace Clippy with `-D warnings`,
formatting and `git diff --check` passed.

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

## Latest outliner and real-manuscript work

The terminal outline now treats Enter on a section/chapter as “open its first
manuscript document”: direct prose is preferred by outline order, otherwise the
first descendant document is opened. Empty headings open an empty manuscript
context with a useful creation hint. Returning from the editor restores the
selected scene or prose document. Space now expands/collapses headings and
toggles a scene between Included and Excluded; Ctrl+Space remains available for
multi-selection. These changes are incremental; Outline and Scene Cards are
still separate workspaces, so #55 is not complete and still requires human UAT.

Verified for this batch: full workspace tests, workspace Clippy, formatting,
Python harness syntax and `git diff --check` all passed.

Private real Scrivener regression fixtures were checked read-only on 2026-11-25.
`(cd fixtures && sha256sum --quiet -c SHA256SUMS)` passed before and after each
run. Git tracks no files under `fixtures/`, `generated/`, `reports/` or
`baselines/`; the four root-relative exclusions are present. The three projects
use Scrivener 3.1.5.1 Windows manifests (manifest revision 2.0): book 01 has 70
binder items and RTF/PDF assets; book 02 has 64 items, nested structure and 44
embedded RTF picture groups; book 03 has 118 items and RTF/plain-text assets.
`epubcheck` is unavailable; Java, Calibre and Pandoc are present.

The private production-pipeline harness is `tools/real_manuscript_regression.py`.
It defaults to repository-relative fixtures and writes all project copies,
publications and content-bearing reports under ignored `generated/` and
`reports/`. Commands:

```sh
python3 tools/real_manuscript_regression.py --phase inspect
python3 tools/real_manuscript_regression.py --book book-02 --phase import
python3 tools/real_manuscript_regression.py --phase compile \
  --project-root generated/IMPORT_RUN_ID
python3 tools/real_manuscript_regression.py --phase full
```

Latest complete three-book run: `reports/clean-commit-d3381a4/`, with generated
projects and EPUBs under `generated/clean-commit-d3381a4/`. It used the
production importer/storage/compiler and reopened projects before compilation.
The tested source revision is `d3381a462e9a103f86701c69333838e00e524ab5`; the
harness marked the tree dirty because user-owned `.gitignore`, `README_TMP.md`
and `UAT/` files were present, while application sources matched the commit.
A local provisional result is preserved at
`baselines/provisional-d3381a4/`; the earlier provisional run remains in
`baselines/provisional-9c5c887/`. Neither is an accepted publishing baseline.

For all three books, extracted manuscript bodies survived persistence/reopen
(44/44, 41/41 and 76/76), research bodies survived (1/1, 1/1 and 4/4), Trash
bodies were excluded, and fixture fingerprints matched before/after. All
imports remain incomplete due to unresolved warnings/items. These checks
establish extraction-to-storage consistency, not complete fidelity.

The RTF image import repair now preserves all 44 reference illustration payloads
byte-for-byte for the illustrated non-fiction book; its generated EPUB has 44
images and zero broken references. A separate non-fiction source contains two
images absent from its historical reference publication; this is unresolved and
may reflect a source/revision/compile difference. EPUBCheck remains unavailable.
Reference/generated paragraph counts are 1298/358, 648/408 and 1813/1430; link
counts are 45/0, 149/0 and 26/0. These require tracing through source
extraction, persisted documents, compile input and historical settings. Do not
claim publication acceptance. The current fixtures have zero reference table
rows, so they do not validate table preservation. Next: investigate unexpected
source images and remaining text, paragraph, link and navigation differences;
then run a standards validator and continue the other open v1.0 requirements.

The user also provided `README_TMP.md` for replacing the product README. This is
queued in documentation/release preparation after current high-priority import
and publication regressions; it must be capability-audited, preserve accurate
engineering guidance in developer docs, verify the licence and use only genuine
screenshots made with synthetic content.

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

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

Latest measured run: `reports/verification-after-fixes-20261125/`, with
generated projects and EPUBs under
`generated/verification-after-fixes-20261125/`. It used the production
CLI/importer/storage/compiler and reopened projects before compilation. The
report identifies the pre-commit source revision and a dirty working tree, so
it is diagnostic, not a release baseline. The measured results remain
provisional.

For the three books, bodies extracted by the importer were preserved in
persisted Markdown (44/44, 41/41 and 76/76); research bodies were also preserved
(1/1, 1/1 and 4/4), and Trash bodies were excluded. Fixture fingerprints matched
before and after. These checks establish extraction-to-storage consistency,
not complete fidelity to every source asset or historical publication. All
imports remain `incomplete` because importer warnings and unresolved source
items remain.

Publication acceptance has not passed. EPUBCheck is unavailable in this
environment. For the illustrated non-fiction fixture, the reference contains
44 image resources and the generated EPUB contains none; RTF picture groups are
not currently preserved through import. Reference/generated paragraph counts
are 1298/358, 648/407 and 1813/1430; link counts are 45/0, 149/0 and 26/0.
These measurements require investigation but do not alone prove source-revision
or compiler causes. Historical publication revisions/settings have not been
reconciled. Do not promote a baseline or claim import/publishing acceptance.
Next: repair image/asset import at the source layer, improve sanitised
diagnostics, and trace text, paragraph, link and navigation differences
through extraction, persisted documents and compile input. Keep all
manuscript-bearing reports local and never commit fixture-derived files.

After committing the current importer and content-unit persistence fixes, rerun
the three-book suite against a clean commit. Keep its result provisional and do
not replace a better validated baseline automatically.

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

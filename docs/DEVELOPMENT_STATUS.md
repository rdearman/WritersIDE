# Writer's IDE Development Status

Updated: 2026-11-25

This is the handover note for ongoing implementation. GitHub issues remain the
authoritative source for approved requirements and human UAT findings.

## Current checkpoint — 25 November 2026

The first-run Guided Tour navigation defect has a verified implementation
regression. While the tutorial is on its Save step, editor input remains
available after the first verified character; Help-menu arrow navigation can
reach **Resume Tutorial**; and the visible instruction and paused footer now
give the exact `Help > Resume Tutorial` route. A Linux PTY workflow exercised
first launch, lesson navigation, typing and saving in the disposable sample
project, pausing, resuming through Help, advancing to lesson 2, pausing and
resuming there, then exiting. It verified the saved sentence after reopening
the isolated sample project and verified the author's original project stayed
scene-free. This automated evidence does not replace human UAT for #41/#59.

Verification for this checkpoint: `cargo fmt --all -- --check`,
`cargo clippy --offline --workspace --all-targets -- -D warnings`, and
`cargo test --offline --workspace -- --test-threads=1` all passed. The full
workspace run included 336 terminal unit tests, 3 author-workflow integration
tests, 22 Linux PTY tests, and all other workspace unit, integration and
documentation tests. The first-run tutorial PTY journey passed. Keep #41 open
for author verification; continue with the next highest-priority open v1.0
repair after this batch is pushed.

A second outliner repair adds Edit > Rename Selected Section or Chapter. The
field starts with the current heading, Ctrl+A replaces it, Enter persists the
new name, and Esc cancels. Storage tests confirm stable node identity, parent,
position, and dependent scene placement; a Linux PTY journey renamed a chapter,
exited, reopened the project, and verified the heading and unchanged prose.
`cargo fmt --all -- --check` and workspace Clippy passed. The complete
`cargo test --workspace --offline` initially hit the sandbox restriction on the
Ollama crate's localhost-only test socket; rerunning with the previously
authorised socket permission passed, including 337 terminal unit tests, 3 author
workflow tests, and 24 Linux PTY tests.

The latest human publication UAT reports material EPUB fidelity failure despite
EPUBCheck passing. The new priority is to trace RTF decoding corruption from
Scrivener extraction through generated XHTML and fix private-planning-material
leaks in the shared Compile IR. Treat the current Scrivener source as authoritative;
historical EPUBs are comparison evidence, not exact text baselines. Before making
changes, refresh issues #7, #35, #44, #53, and #73 and verify fixture checksums.
Keep all manuscript-bearing output and detailed reports local.

The 25 November follow-up audit found that binder `IncludeInCompile` was being
read from recursively flattened metadata. A folder could therefore inherit a
child's flag. Import inventory now reads each item's own metadata and records
its `SectionType` separately. It also inventories project publication metadata,
content scope, format-level section layout mappings and default front/back
matter switches from `Settings/compile.xml`. Synthetic regression coverage
verifies parent and child flags remain independent. The private inspect-only
run `reports/20261125T224346Z/` read all 70/64/118 binder items, recovered the
available project metadata and two compile format records per project, and left
fixture checksums unchanged. This is an inventory correction only: these
source compile rules are not yet applied to the shared Compile IR, WIDE
publication profiles, or semantic document presentation. Publication privacy
and fidelity therefore remain failed acceptance gates.

The follow-up outliner repair separates scene-title editing from save-conflict
review, which previously shared the same key-input mode. Scene > Rename now
opens a focused field prefilled with the current title; Ctrl+A clears it,
Enter saves, and Esc cancels. A Linux PTY test navigates from the editor back
to Outline, opens the Scene menu, renames the selected scene, exits, and
reopens the project to verify the title persisted while manuscript prose stayed
unchanged. Formatting, workspace Clippy, the full workspace tests, and all 23
Linux PTY tests passed for this batch. This fixes rename input only; the
author-approved single unified outliner and metadata inspector remain
outstanding in #55/#68 and still require human UAT.

The root README is now an author-facing alpha introduction. Supporting
documents are [Getting Started](GETTING_STARTED.md), the [developer guide](DEVELOPMENT.md)
and the [CLI reference](CLI.md). The README screenshots were captured from the
running terminal application in a PTY using only the synthetic “The Lantern
House” demonstration project. `README_TMP.md` remains the supplied, untracked
draft and must not be staged. The author has also requested a later docs-folder
consolidation; schedule it after higher-priority reliability, editing, import
and publishing work, and audit references before removing any documents.

Independent verification tools checked on this host:

- EPUBCheck 4.2.6; Aspell 0.60.8.1; Hunspell 1.7.2.
- Hunspell dictionaries: British and American English, French, Italian,
  German, Spanish, Portuguese and Russian. Aspell dictionaries include
  English, French and Italian.
- qpdf 11.9.0; Poppler utilities 24.02.0; SQLite CLI 3.45.1; libxml2/xmllint
  2.9.14; unzip 6.00.
- cargo-audit 0.23.0; cargo-deny 0.22.1; cargo-machete 0.10.1;
  cargo-llvm-cov 0.9.2.

Availability does not mean every audit has passed. These are optional
development checks, not runtime dependencies. The EPUBCheck wrapper at
`/usr/bin/epubcheck` fails under the restricted shell, but the installed JAR at
`/usr/share/java/epubcheck.jar` runs successfully through Java.

The latest completed real-manuscript run is
`reports/list-marker-all-books-20261125/` (projects and publications under the
matching `generated/` directory). It used the production importer, persistence,
reopen and compiler based on `a21dc27a3b51599076ccbace486ea5946529520b` with
uncommitted importer/harness changes. Fixture checksums passed after testing.
Manuscript bodies survived persistence exactly: 44/44, 41/41 and 76/76;
research bodies survived 1/1, 1/1 and 4/4. All three Scrivener source
fingerprints were unchanged and SQLite integrity checks returned `ok`. The RTF
list-marker fix restored the affected book's measured list items from 36 to 45.
All generated EPUBs had zero EPUBCheck errors and warnings; historical
references had 2/2/0 errors. A compile-only rerun of all three persisted
projects is at `reports/list-marker-compile-correct-status-rerun-20261125/`.
The full-run JSON acceptance wording was stale, so use the compile-only report
for validator status. Neither report establishes fidelity acceptance: source
compile inclusion mappings and material text/structure differences remain
unresolved, and human publication review is outstanding. See
[the regression guide](REAL_MANUSCRIPT_REGRESSION.md) for exact details.

Targeted Rust importer tests (23), the importer integration test (1), the
synthetic Python regression tests (6), `cargo fmt --all -- --check`, workspace
Clippy with warnings denied, and the full workspace suite passed after the RTF
list repair and unused dependency cleanup. The latest full workspace run
included 334 terminal unit tests, 3 end-to-end author workflows, 21 PTY tests,
and all workspace unit/integration/doc tests. The Ollama loopback test passed
with the previously authorised local socket permission. The PTY suite also
confirmed that Previous Position with empty history leaves the app running;
#62 remains open for human UAT, not because implementation is unverified by
automation.

`cargo audit` completed after fetching a local RustSec database (1,568
advisories) and reported 26 vulnerabilities plus 10 allowed warnings in the
lockfile. Relevant dependency paths include `rusqlite`/`libsqlite3-sys`,
`url`, `zip` and `zstd`; the concrete advisory list should be reviewed before a
release. No automatic dependency upgrades were made. `cargo deny check` could
not download all locked platform-specific crate metadata in the restricted
environment. `cargo-machete` initially found two unused direct dependencies;
both were removed, and a repeat check passed. `cargo llvm-cov` could not run
because `llvm-tools-preview` is not installed and rustup could not write or
download it here. Do not claim dependency-policy or coverage checks passed.

Next: finish source compile-section mapping and accurate inclusion semantics,
compare the semantic imports/publications against the current source rather
than assuming historical EPUB equality, then continue the highest-priority
open v1.0 issue. The feature freeze remains active. Documentation-folder
consolidation remains queued behind the reliability, editing, import and
publication priorities.

## Priorities

1. Protect manuscript data and fix regressions that block ordinary writing.
2. Consolidate Outline and Scene Cards into the one hierarchical outliner
   approved in #55, coordinated with #20 and #68.
3. Continue the terminal word processor work in #61 and the remaining UAT
   workflow corrections under #44.
4. Complete import, publishing, recovery and other approved work in dependency
   order. The feature freeze in #44 remains in force.

## Recent verified commits

- `66be93e` — preserve Scrivener RTF hyperlinks during import.
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

GitHub #55 was amended on 25 November to require one author-facing outliner,
with optional card/table presentations sharing canonical selection and order.
The View menu now exposes Scene Cards as a presentation of Outline; switching
between tree and cards preserves chapter/scene selection and does not add a
second navigation-history or Tab destination. The Scene menu, inclusion toggle,
scene creation, metadata edits and scene movement now work from the tree.
Insert > New Chapter from the tree creates an empty sibling/child at the
selected structural boundary, refreshes immediately, and selects the new node;
the fresh-project PTY workflow now verifies this. The Removed Scenes flow
returns to the original outliner presentation and selection. The underlying
`Workspace::Cards` renderer remains a separate projection, so this is an
integration step, not completion of #55. Next is consolidating the hierarchy
and metadata inspector around the unified outline while preserving
Included/Excluded and Unplaced distinctions, stable records, recovery and
compile behaviour.

## Latest outliner and real-manuscript work

The terminal outline now treats Enter on a section/chapter as “open its first
manuscript document”: direct prose is preferred by outline order, otherwise the
first descendant document is opened. Empty headings open an empty manuscript
context with a useful creation hint. Returning from the editor restores the
selected scene or prose document. Space expands/collapses headings and toggles
a scene between Included and Excluded; Ctrl+Space remains available for
multi-selection. The card renderer is reached from View as another Outline
presentation, with shared selection, actions and return context. The metadata
inspector and full structural integration remain outstanding; #55 is open for
human UAT.

Verified for this batch: formatting, workspace Clippy with warnings denied,
the complete workspace tests (335 terminal unit tests, 3 author workflows,
21 PTY tests, and all other workspace unit/integration/doc tests), and real PTY
menu, empty chapter creation, scene creation, removal, structural undo and
outliner presentation checks passed. The Ollama loopback unit test required the
previously authorised local socket permission.

The #56 rendering regression now exercises an actual PTY transition from
manuscript prose to an imported synthetic reference and back, checking that
neither view leaks text into the other and that both remain contiguous through
80×24 and 201×60 resizes. It also verifies the reference source remains
unchanged and the manuscript bytes survive reopen. The historical scattered-
glyph screenshot still has not been reproduced; this is added regression
coverage, not a proven root-cause fix. #56 remains open for human verification.

The #58 shortcut-precedence fix now also has a real-PTY regression in the same
smoke workflow. It sends the visible File-menu `n` mnemonic and the terminal
Ctrl+N byte, confirms each opens New Project, cancels both dialogues, and
checks that the manuscript screen remains active and unchanged. The focused
Linux PTY test passed; human verification of terminal/key-delivery variants
remains outstanding.

Private real Scrivener regression fixtures were checked read-only on 2026-11-25.
`(cd fixtures && sha256sum --quiet -c SHA256SUMS)` passed before and after each
run. Git tracks no files under `fixtures/`, `generated/`, `reports/` or
`baselines/`; the four root-relative exclusions are present. The three projects
use Scrivener 3.1.5.1 Windows manifests (manifest revision 2.0): book 01 has 70
binder items and RTF/PDF assets; book 02 has 64 items, nested structure and 44
embedded RTF picture groups; book 03 has 118 items and RTF/plain-text assets.
EPUBCheck is installed as `/usr/share/java/epubcheck.jar` (v4.2.6) and is invoked
with Java because the `/usr/bin/epubcheck` wrapper cannot find its interpreter
inside the restricted shell. Calibre and Pandoc are also present.

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
may reflect a source/revision/compile difference. EPUBCheck was unavailable for
this historical run; later runs validated generated EPUBs with the installed
EPUBCheck JAR, as recorded above.

The next importer correction preserves safe Scrivener RTF hyperlink fields as
Markdown links, retains visible text while omitting unsafe destinations, and
reports unsupported targets. Synthetic importer tests cover a safe web link and
a rejected `javascript:` URI. The three-book production run at
`reports/hyperlink-before-commit-20261125/` confirmed the source bodies remain
exact after persistence/reopen (44/44, 41/41, 76/76), all source fingerprints
remain unchanged, and generated publications contain 63, 170 and 63 links
respectively, with zero broken internal links. Historical references contain
45, 149 and 26 links. Those count differences remain unresolved: the generated
links correspond to non-Trash source RTF fields, but the reference EPUBs are
historical outputs and source/compile settings may differ. Text, paragraph and
navigation differences also remain substantial; no publication acceptance is
claimed. Reference/generated paragraph counts are 1298/358, 648/408 and
1813/1430. The current fixtures have zero reference table rows, so they do not
validate table preservation. Next: investigate remaining text, paragraph,
navigation, image-scope and historical-link differences, then run a standards
validator and continue the other open v1.0 requirements.

The user supplied `README_TMP.md` as the draft for an author-facing product
landing page. It remains queued in documentation/release preparation after the
current high-priority import/publication regression; audit its capability
claims, licence, links, developer documentation and genuine synthetic-content
screenshots before replacing the root README.

The post-commit three-book regression for `66be93e307489f53f1d5368290c1c040c0b962d8`
is stored locally in `reports/clean-commit-66be93e/`; a provisional local copy is
in `baselines/provisional-66be93e/`. The run completed and fixture checksums
passed. Source-body persistence was exact for manuscript bodies (44/44, 41/41,
76/76) and research bodies (1/1, 1/1, 4/4), and all three source fingerprints
were unchanged. Generated EPUBs contain 63, 170 and 63 links, with zero broken
internal links; references contain 45, 149 and 26. Generated/reference image
counts are 2/0, 44/44 and 0/0; all generated references resolve. Text sequence
similarities remain 0.951104, 0.872523 and 0.909124; paragraph counts remain
1298/358, 648/408 and 1813/1430. Imports are incomplete. EPUBCheck was not run
for this historical baseline; later validation results are recorded above.
These results are provisional, not fidelity acceptance. The earlier note about
pending GitHub comments is superseded by subsequent REST updates.

The user also provided `README_TMP.md` for replacing the product README. This is
queued in documentation/release preparation after current high-priority import
and publication regressions; it must be capability-audited, preserve accurate
engineering guidance in developer docs, verify the licence and use only genuine
screenshots made with synthetic content.

## Latest publication work checkpoint — 25 November 2026

Human Calibre UAT rejected the recent EPUBs for fidelity despite successful
EPUBCheck validation. Commit `5de1f5e` fixed RTF `\\uN` fallback parsing and
made content outside recognised Draft/Research/Trash containers private by
default. Commit `c2adcbb1490b956f313c2ae88bb34501227316b2` then fixed
per-binder-item metadata scoping and added inventory of project metadata,
`SectionType`, compile scope, format layout mappings and front/back matter
switches. These source compile settings are not yet applied to WIDE's shared
Compile IR or named publication profiles. Do not claim publication acceptance:
chapter numbering, hidden structural headings, scene separators, variables and
alternative publication matter remain incomplete.

The latest exact-commit local EPUB set, for provisional Calibre review, is:

- `generated/20261125T230034Z/book-01.epub`
- `generated/20261125T230034Z/book-02.epub`
- `generated/20261125T230034Z/book-03.epub`

The exact-commit machine and human reports are in
`reports/20261125T230034Z/`. Fixture checksum and source fingerprint checks
passed. Source body persistence was exact (manuscript 39/39, 36/36, 68/68;
research 6/6, 6/6, 12/12). All generated EPUBs passed EPUBCheck 4.2.6 with
zero errors/warnings. Historical references separately had 2/2/0 errors and
zero warnings. Historical token comparison has missing/additional counts
918/1084, 1081/205, and 2643/2494. These are unresolved edition comparisons,
not proof of current-source omissions; semantic fidelity and privacy acceptance
remain open. The import report still marks each import incomplete. See
[the regression log](REAL_MANUSCRIPT_REGRESSION.md#25-november-2026--scrivener-compile-metadata-inventory-correction)
for the current limits and exact classification.

Verification for `c2adcbb` passed: `cargo fmt --all -- --check`, workspace
Clippy with warnings denied, the full offline workspace suite (including 337
terminal unit tests, 3 author journeys and 24 Linux PTY tests), and 7 Python
regression-harness tests. The full test suite required the previously
authorised localhost socket permission for one Ollama test. Fixture SHA-256
verification passed after the exact-commit private run.

Next: apply source compile inclusion/layout and metadata to the shared Compile
IR and named publication profiles, then repeat the full real-manuscript run.
Keep the feature freeze and continue the open v1.0 backlog afterward. `.gitignore`,
`README_TMP.md` and `UAT/` remain user-owned/unrelated and must not be staged.

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

## Current working checkpoint — RTF paragraph indentation and EPUB differential tooling

The current working batch fixes an importer defect confirmed by Calibre UAT:
some Scrivener RTF paragraph indentation was emitted as leading Markdown
whitespace (including mixed space-plus-tab runs), so prose became `<pre>` code
in the exported EPUB. The production importer now serializes leading RTF
indentation without Markdown code-block semantics. Added synthetic regression
coverage; original prose and internal tabs are preserved.

The earlier paragraph-indentation check outputs are retained locally:

- `generated/rtf-indent-full-20261125/book-01.epub`
- `generated/rtf-indent-full-20261125/book-02.epub`
- `generated/rtf-indent-full-20261125/book-03.epub`

Full per-book discrepancy report: `reports/rtf-indent-full-20261125/` (private,
content-bearing, do not commit or share publicly). Fixture SHA-256 validation
passed before and after. Persisted source bodies are exact (39/39, 36/36,
68/68); generated preformatted-block counts are 0/0/0; book 02 retains all 44
illustrations; generated EPUBCheck 4.2.6 results are zero errors/warnings.
The `cargo test --workspace --offline` suite passed after the production fix,
including all unit, author workflow, PTY and documentation tests. Python
regression harness tests, formatting and Clippy also passed.

The differential harness now inspects per-document paragraph/character
differences and reports CSS declarations, typography, headings, pre/code,
lists, tables, assets, links, navigation and metadata. It reports likely
pipeline context and keeps all content-bearing details local. Historical EPUBs
are not exact source baselines. Overall fidelity remains failed human UAT; this
repair only addresses the verified unintended code-block defect.

The differential tooling, punctuation repair and inventory are separate
checkpoints; latest outputs and reports are recorded below. The next work is
Scrivener compile selection/layout integration in the shared Compile IR and
named publication profiles. Keep all private manifests, generated output and
detailed reports out of Git. The root `.gitignore`, `README_TMP.md` and `UAT/`
are user-owned changes; do not stage or alter them.

## Expanded private regression corpus — 26 November 2026

Commit `3db2360` added the read-only expanded-corpus inventory tooling. Its latest
local report is `reports/back-catalogue-20261126-release/manifest.json`;
temporary converted MOBI text is removed after analysis.
The inventory inspected 26 Scrivener manifests and classified 47 publication
and resource files. It records Creator/version hints, manifest revisions,
binder structure counts, candidate language, SHA-256 hashes, publication
metadata, uncertain project/publication matches and possible related editions.
All three controlled projects have an exact normalized-text match in this
back-catalogue; the possible match and revision candidates are not confirmed.
Project-tree and publication-file hashes remained identical between runs,
the controlled fixture checksum manifest passed pre/post, no symlinks were
found, and Git tracks no private files. Five focused inventory tests and all
14 Python regression harness tests passed.

## RTF punctuation repair — 26 November 2026

The expanded corpus uncovered a confirmed defect: the production Scrivener RTF
decoder dropped named punctuation controls including `\endash`. The importer
now preserves en/em dashes, typographic quotation controls, bullets, NBSP,
non-breaking hyphen and discretionary hyphen. Synthetic unit coverage passes.

The latest three-book results are in
`reports/rtf-punctuation-all-20261126/`, with exact local EPUB paths:

- `generated/rtf-punctuation-all-20261126/book-01.epub`
- `generated/rtf-punctuation-all-20261126/book-02.epub`
- `generated/rtf-punctuation-all-20261126/book-03.epub`

The harness report records base commit `3db2360` and a dirty working tree
because the punctuation change was being exercised. Commit `895d71f` contains
the tested importer patch and is the resulting source revision.

Source body persistence matched exactly (39/39, 36/36 and 68/68; research
6/6, 6/6 and 12/12). Book 02 retains the reported date-range en dash, all 44
illustration placements and 43 distinct embedded payloads. All three generated
EPUBs passed EPUBCheck 4.2.6 (zero errors/warnings) and contain no unintended
preformatted blocks. Fixture checksum validation passed before and after.
`cargo fmt --all -- --check`, workspace Clippy with warnings denied, full
`cargo test --workspace --offline` (337 terminal tests, 3 author workflows,
24 PTY tests and all other workspace/doc tests), and all 14 Python regression
tests passed. The Ollama test used the previously authorized loopback-only
socket access; no external network service was used.

Next: integrate Scrivener's imported compile inclusion/layout and metadata into
the shared Compile IR and existing named publication profiles, with synthetic
tests and the controlled three-book regression. Issues #53, #7 and #73 now have
sanitized status comments; all remain open for human acceptance. Publication
fidelity remains failed human UAT. Never commit the private reports or generated
EPUBs. Do not stage the unrelated root `.gitignore`, `README_TMP.md` or `UAT/`.

## RTF emphasis whitespace repair — 26 November 2026

Fresh Calibre inspection of book 01 exposed literal Markdown bold markers around
Scrivener bold runs whose content began or ended with whitespace. The production
RTF importer now keeps leading and trailing whitespace outside the emphasis
delimiters, preserving semantic emphasis without visible `**` artefacts. The
focused regression and all 31 `story-import` tests pass.

Full fresh import, persistence/reopen and compile results are in
`reports/rtf-emphasis-all-20261126/`; private EPUBs for further local inspection
are:

- `generated/rtf-emphasis-all-20261126/book-01.epub`
- `generated/rtf-emphasis-all-20261126/book-02.epub`
- `generated/rtf-emphasis-all-20261126/book-03.epub`

All 143 checked manuscript bodies survived persistence exactly (39/39, 36/36,
68/68); all 24 checked research bodies did also (6/6, 6/6, 12/12). Generated
EPUBCheck 4.2.6 results are zero errors and warnings, and no generated book
contains `<pre>` blocks. Book 01's visible prose contains no literal asterisk
formatting delimiters. Book 03 contains one `***` paragraph as literal source
content in the imported Markdown; it is not introduced by the emphasis fix and
requires a separate semantic scene-break/layout decision. Fixture checksums
passed before and after the run. The EPUBs remain provisional; overall
publication fidelity has not passed human UAT.

The regression JSON identifies base commit `a55a0f2` and a dirty tree because
the tested whitespace correction was not yet committed when the run started.
Next: run formatting, Clippy and the full workspace suite, then commit/push the
small importer correction and update #53/#7/#73 with sanitised test evidence.
Continue with shared Compile IR inclusion/layout/profile integration; do not
stage private reports/EPUBs or the user-owned root `.gitignore`,
`README_TMP.md` and `UAT/`.

## Scrivener compile-excluded document retention — in progress

The importer previously mapped any binder item with `IncludeInCompile=No` to
`Skip`, which discarded it from the editable WIDE project. Import planning now
keeps the document and its provenance. For mapped scenes, the application also
sets the existing persistent scene exclusion so the prose remains editable but
is absent from default compilation. Synthetic application and importer tests
cover retention and compile omission. This does not yet apply compile flags to
section/chapter headings or chapter-level prose, nor does it recover per-format
overrides; those still require shared Compile IR and publication-profile work.
The targeted importer tests pass. A fresh three-book run and full workspace
verification now pass. The synthetic application test confirms a source scene
marked `No` remains editable, carries an excluded compile status and is absent
from the default Markdown artifact. The source fixture is unchanged.

The fresh controlled run is `reports/scrivener-inclusion-retain-20261126/`
with outputs under `generated/scrivener-inclusion-retain-20261126/`; all source
checksums passed before and after. All source/reopened manuscript and research
bodies matched exactly and all generated EPUBs passed EPUBCheck 4.2.6 with zero
errors/warnings. The current controlled books mark all binder items included,
so the `No`-flag path is verified synthetically rather than by these three
sources.

Verification passed: `cargo fmt --all -- --check`, workspace Clippy with
warnings denied, and the full offline workspace test suite (337 terminal unit
tests, 3 author-workflow tests, 24 real-PTY tests and all other workspace and
documentation tests). The locally authorised Ollama test bound only its
loopback fixture socket; no external network was used. The 14 Python regression
tests passed in the preceding batch.

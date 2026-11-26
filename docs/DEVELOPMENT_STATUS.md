# Writer's IDE Development Status

Updated: 2026-11-26

This is the handover note for ongoing implementation. GitHub issues remain the
authoritative source for approved requirements and human UAT findings.

## Current checkpoint — expanded-corpus RTF and EPUB resource regressions, 26 November 2026

The expanded local corpus was used to target `fixtures/back-catalogue/scrivener/BowString.scriv`, which contains a real 8-row, 2-column RTF table and embedded illustrations. Its best automated publication match is `fixtures/back-catalogue/epub/How_To_Make_a_Bowstring.epub`; the relationship remains an **unconfirmed candidate**, not an asserted edition match.

This run exposed and repaired three general defects:

- Relative project resource roots were passed to Pandoc before the process changed into its temporary render directory. Images were left as unresolved EPUB references. The renderer now canonicalises the root before launching subprocesses; a synthetic relative-root EPUB test verifies image packaging.
- Scrivener RTF table row and cell controls were treated as ordinary formatting. Simple rectangular tables are now represented as Markdown tables at extraction time. A synthetic 2×2 RTF table test verifies row and cell preservation.
- Scrivener list indentation after list markers could be interpreted as a nested Markdown code block. RTF alignment whitespace following the list marker is now removed while preserving the list body; a synthetic regression test covers the nested-code failure.

`tools/expanded_scrivener_regression.py` now provides a repeatable one-project
extension of the controlled regression harness. It requires a reviewed mapping,
uses only the production import and compile CLI, fingerprints the fixture tree,
and writes a machine-readable source-fidelity and full EPUB differential report
to ignored local directories. Usage is documented in
`docs/REAL_MANUSCRIPT_REGRESSION.md`.

A fresh production import, save/reopen and EPUB compile for BowString is at:

- Project: `generated/back-catalogue-targeted-20261126/BowString-WIDE-repaired-import/`
- EPUB: `generated/back-catalogue-targeted-20261126/BowString-WIDE-repaired.epub`
- Latest harness EPUB: `generated/back-catalogue-targeted/bowstring-harness-20261126b/publication.epub`
- Local comparison report (contains manuscript-derived material): `reports/back-catalogue-targeted-20261126/bowstring-structural-comparison.json`
- Detailed differential report: `reports/back-catalogue-targeted/bowstring-harness-20261126b/summary.json`
- Import inventory and mapping: `reports/back-catalogue-targeted-20261126/`

The repaired EPUB preserves 11 image placements backed by 9 unique embedded assets, 8 table rows/16 cells, and has zero `pre`/`code` blocks. EPUBCheck 4.2.6 reports zero errors and warnings. The candidate historical EPUB has 8 rows/16 cells and 11 image placements, but its pairing with the Scrivener project is unconfirmed; the detailed local comparison records 23 changed paragraph regions, character evidence, CSS, metadata and navigation differences. The current source bodies are exact in the imported project (20/20), ruling out extraction or persistence for those mapped bodies; compile selection/layout and historical revision remain unresolved. These differences are investigation leads, not automatic WIDE defect classifications. Publication acceptance remains provisional.

The original three controlled books were then run through the production full regression harness at `reports/expanded-corpus-bowstring-fix-20261126/`; their EPUBs are in `generated/expanded-corpus-bowstring-fix-20261126/`. Source checksums and fingerprints remained unchanged; imported manuscript-body verification was exact for 39/39, 36/36 and 68/68 bodies after persistence/reopen. Each generated EPUB passed EPUBCheck 4.2.6 with zero errors and warnings. All three import reports remain incomplete, and the run remains provisional pending discrepancy triage and human publication review.

Targeted verification completed: RTF table extraction, RTF list-indentation regression, RTF Unicode fallback regression, relative-root EPUB image packaging, and the existing renderer artifact test passed. All 21 Python tooling tests passed. Workspace `cargo fmt --all -- --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings`, and the full offline workspace suite passed after retrying the localhost-only Ollama test with the previously authorised socket permission: 338 terminal tests, 3 author-workflow tests, 24 Linux PTY tests, and all other workspace unit, integration and doc tests. The controlled regression run preceded only the additional warning for unsupported merged/nested RTF tables; table conversion and list whitespace code were included in that run. Fixture checksums passed before and after the regression runs. No candidate reference match has been promoted to confirmed.

## Current checkpoint — Scrivener compile variables, 26 November 2026

Scrivener project metadata is now carried into new standalone imports when it
is explicitly present in the source inventory. Project title metadata is used
as the suggested import title unless the author supplies a title; available
author, language, genre and synopsis fields are also retained. Missing author
data remains empty, and an invalid or absent source language falls back to the
configured writing language. Importing into an existing project does not
replace its metadata.

The shared Compile IR renderer now resolves the supported Scrivener variables
`<$PROJECTTITLE>`, `<$TITLE>`, `<$FULLNAME>`, `<$AUTHOR>` and sequential
`<$n>`/`<$N>` without changing canonical manuscript text. Unresolved supported
variables and unsupported variables produce source-mapped diagnostics. The
current implementation does not yet recover every Scrivener format-specific
variable or numbering rule. In particular, the current Les Retraités-Pirates
output still exposes structural binder headings (`Manuscript`, sections,
ellipsis chapter placeholders and event headings); resolving variables does
not complete the Scrivener compile-layout mapping in #7/#53/#73.

Fresh production import/reopen/compile run on the working tree:

- Reports: `reports/compile-vars-full-20261126/`
- Projects and EPUBs: `generated/compile-vars-full-20261126/`
- EPUBs: `generated/compile-vars-full-20261126/book-01.epub`,
  `generated/compile-vars-full-20261126/book-02.epub`,
  `generated/compile-vars-full-20261126/book-03.epub`
- All three generated EPUBs pass EPUBCheck 4.2.6 with zero errors and warnings.
- Source body text is exact after persistence/reopen: book-01 39/39 manuscript
  and 6/6 research items; book-02 36/36 and 6/6; book-03 68/68 and 12/12.
- All three source fingerprints are unchanged. Import remains marked
  incomplete for each project; no publication fidelity acceptance is claimed.
- Historical EPUB comparison remains materially different (719/79,
  1040/156 and 1657/1568 missing/additional token instances respectively).
  These are investigation leads, not a classification or acceptance metric.
- The generated book-02 retains all 44 inventoried images; the run also reports
  the existing content/navigation differences for continued investigation.
- Fixture checksums passed before and after the complete run.

Verification for the compile-variable batch: `cargo fmt --all -- --check`,
`cargo clippy --offline --workspace --all-targets -- -D warnings`,
`cargo test --offline --workspace -- --test-threads=1`, and
`python3 -m unittest discover -s tools -v` all passed. The workspace suite
included 337 terminal unit tests, 3 author-workflow integration tests and 24
Linux PTY tests. The Ollama localhost-only test required the previously
authorised loopback socket permission. This automated evidence does not
replace human UAT; #7/#53/#73/#44 remain open for acceptance.

The differential harness now includes source-body evidence in each detailed
paragraph discrepancy. For an imported project, compile-only comparisons also
recheck extracted Scrivener body hashes against the persisted project. Reports
state which pipeline stages are supported or ruled out by those checks, instead
of labeling every difference with the same generic cause. They also enumerate
added/removed CSS declarations and font settings and field-level metadata
differences. This narrows investigation; it does not infer that a historical
edition difference is a WIDE defect or fully render the browser CSS cascade.
All 21 Python tooling tests pass.

An exact-commit compile-only rerun of the three imported projects is at
`reports/diff-stage-20261126/`; its latest EPUBs for local visual review are:

- `generated/diff-stage-20261126/book-01.epub`
- `generated/diff-stage-20261126/book-02.epub`
- `generated/diff-stage-20261126/book-03.epub`

The rerun verified 39/39, 36/36 and 68/68 manuscript bodies against their
persisted source bodies, with zero missing or changed destinations. Each
generated EPUB passed EPUBCheck with zero errors and warnings. Material
paragraph, typography, metadata, heading, navigation and inclusion differences
remain in the local reports. Fixture checksums passed. This comparator
instrumentation batch does not change generated publication content and does
not constitute human acceptance.

## Current checkpoint — 26 November 2026

The author expanded the local private corpus under `fixtures/back-catalogue/`.
The latest inventory is
`reports/back-catalogue-20261126-expanded-v4/manifest.json`, generated by
commit `7720ed9`. It re-inspected all 26 Scrivener manifests (including one
nested project), 45 publication candidates and two supporting image assets;
there were no inspection failures. The matcher returned candidate rankings,
not confirmed edition links. The three controlled projects have exact
normalized extracted-text counterparts in the expanded corpus; their source
tree fingerprints are separately recorded. Similarity leads among related
archery manuscripts are not proof of revision lineage. The protected fixture
checksum manifest passed before and after inventory.

The regression reader previously lost paragraph-level evidence when a
historical EPUB chapter was not well-formed XML. Commit `e4ac09e` adds a
tolerant HTML recovery pass for analysis while retaining each XML parse error
as a reported defect. This does not repair or rewrite the reference EPUB. The
same report now records per-paragraph character edits, UTF-8 source ranges,
and missing/additional Unicode codepoints with names. All 19 Python tooling
tests pass. Exact-commit compile-only comparison from the persisted imports is
`reports/e4ac09e-exact-diff/`; generated EPUBs are under
`generated/e4ac09e-exact-diff/`. All three generated EPUBs pass EPUBCheck
4.2.6 with zero errors and warnings. The historical references remain
separately reported with error counts 2/2/0. The reader recovered paragraph
structure from 16, 13 and 26 malformed reference XHTML documents, exposing
paragraph totals 1298/979, 648/279 and 1813/1284 (reference/generated). These
differences are not yet classified as source revision, compile settings or
WIDE defects; use current Scrivener source, import reports and compile settings
to resolve them. The comparison remains provisional and requires human
publication acceptance.

The fresh import/full run at `reports/7720ed9-full-diff/` used commit
`7720ed9` before the tolerant comparison-reader change. It preserved every
checked manuscript body (39/39, 36/36, 68/68) and research body (6/6, 6/6,
12/12) exactly across import and reopen, and all Scrivener source fingerprints
remained unchanged. The exact-commit compile-only run at `e4ac09e` used those
persisted projects. It adds publication comparisons, not a fresh import.

The inventory harness now distinguishes publication candidates from image
assets and records binder depth and nested Scrivener projects. Its local
manifest at `reports/back-catalogue-20261126-expanded-v2/manifest.json`
contains 26 inspectable `.scrivx` projects, 45 candidate publication documents
and 2 supporting image assets. One manifest is nested within another project.
The 26 includes the three controlled Scrivener fixtures. Creator tags are
source metadata hints, not verified desktop-version compatibility. Match
rankings and related-project similarities are unconfirmed leads; zero exact
normalized-text duplicate groups were found. The originals passed
`fixtures/SHA256SUMS` before and after inventory. No private data is tracked.

Targeted testing with an explicit local mapping (not an author-approved
publication layout) found a compiler defect not present in the three
controlled manuscripts: repeated
structural and manuscript heading titles could be emitted with duplicate
XHTML IDs, causing EPUBCheck failures and invalid navigation. The shared
Pandoc path now adds deterministic, distinct heading IDs derived from Compile
IR source identities for HTML-family output only. Plain Markdown and TXT
remain unchanged. The repeated-title synthetic EPUB test passes. The local
renderer smoke-test output (not a fidelity-approved publication)
`generated/back-catalogue-adderand-anchor-fix-20261126/Adderand.epub` passes
EPUBCheck 4.2.6 with zero errors and warnings, and each XHTML document has
unique IDs. The explicit test mapping is retained in local reports and does
not establish the right compile layout for that project. The earlier,
intentionally retained failing artifact is
`generated/back-catalogue-adderand-20261126/Adderand.epub`; do not treat it as
the corrected output.

The controlled fresh import/reopen/compile regression at
`generated/back-catalogue-anchor-control-20261126/` preserved 39/39, 36/36 and
68/68 manuscript bodies plus 6/6, 6/6 and 12/12 research bodies. All three
Scrivener source fingerprints were unchanged and all generated EPUBs passed
EPUBCheck 4.2.6 with zero errors or warnings. Its differential reports are
under `reports/back-catalogue-anchor-control-20261126/`; these results were
produced from the working tree immediately before the renderer fix was
committed. An exact-commit compile-only rerun at
`reports/e2dd238-controlled-recompile-20261126/` used `e2dd2387bf77bd47485c0ec81dce3cce43b60176` and confirmed all three source fingerprints unchanged and all generated EPUBs EPUBCheck-clean. Publication fidelity remains provisional: each historical EPUB comparison still has paragraph-level differences, import previews still report incomplete mappings, and human UAT has not passed.

Latest controlled generated EPUBs for local inspection (not human-accepted):

- `generated/e2dd238-controlled-recompile-20261126/book-01.epub`
- `generated/e2dd238-controlled-recompile-20261126/book-02.epub`
- `generated/e2dd238-controlled-recompile-20261126/book-03.epub`

The expanded-corpus DOCX candidate also produced a detailed local report at
`reports/back-catalogue-20261126-expanded/adderand-docx-diff-detailed.json`.
RTF-extracted canonical text and persisted scene text matched exactly. After
excluding the generated metadata title and two structural headings, the 205
source paragraphs and generated DOCX body paragraphs match exactly in order
and content (NFC plus whitespace normalization; boundaries retained). The
historical DOCX has 417 paragraphs (204 non-empty); it differs from the
current source in 30 paragraph-change regions. The source revision and
historical formatting/spacing differences remain unresolved; they are not
evidence of WIDE body corruption. This candidate is not an author-approved
edition or a human publication acceptance result.

For the renderer/inventory batch, `cargo fmt --all -- --check`, workspace
Clippy with `-D warnings`, all 16 Python tooling tests, and the full offline
workspace suite passed. The workspace suite included 337 terminal unit tests,
3 author-workflow tests and 24 Linux PTY tests, plus all crate, integration
and documentation tests. The Ollama localhost-only test ran under the
previously authorised local-socket permission. Fixture checksums passed after
both the expanded inventory and the complete controlled regression.

GitHub open issues and recent comments were refreshed using the REST API at
this checkpoint. #7, #53 and #73 carry current compile/import/publishing
requirements; #15, #33, #44, #67 and #68 remain open with the approved
diagnostic, acceptance, assets and outliner updates. #75 is outside the v1.0
feature freeze. Continue the v1.0 backlog; do not begin v2.0 work.

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
The refreshed report `reports/back-catalogue-20261126-expanded-final/manifest.json`
inspected 26 Scrivener manifests and classified 47 publication/resource files
(13 EPUB, 16 MOBI, 4 ODT, 4 RTF, 2 DOCX, 2 HTML, 2 TXT, 1 DOC, 1 PDF and 2
image files). It records Creator/version hints, manifest revisions, binder
structure counts, candidate language, SHA-256 hashes, publication metadata,
uncertain project/publication matches and possible related editions. Source
Creator hints identify 24 `SCRWIN-3.1.5.1` tags, one `SCRWIN-3.1.1.0`, and one
`SCRWIN-1.0.0`; these are not independently verified compatibility claims.
The classifier suggests English for 24 projects, one unclassified Latin-script
project and one uncertain CJK-script project. The corpus has 941 Text, 284
Folder, 26 DraftFolder, 26 ResearchFolder, 26 TrashFolder, 15 PDF and 4 Image
binder items; maximum binder depth ranges from 1 to 4. Exact normalized-text
duplicate groups: 0. The matcher ranks 130 project/publication candidates and
85 possible related-project pairs; these remain unconfirmed. The controlled
fixtures have close/exact normalized-content candidates, but lineage is not
established. The controlled fixture checksum manifest passed before and
after, no symlinks were found, and Git tracks no private corpus files. The
tool does not execute corpus files. Focused tooling tests are synthetic; the
expanded private projects are targeted compatibility data, not accepted
publication references.

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

## Pandoc quote preservation — in progress

A local visual comparison of matching book-01 passages traced a second
compiler-introduced difference: Pandoc's implicit `smart` Markdown extension
converted ASCII quote pairs (`''`) to curly quotes while leaving the legacy
opening double-backticks unchanged. The production renderer now disables that
implicit conversion, and a synthetic HTML-render test verifies literal quote
pair preservation. A compile-only run of all three persisted books passed
EPUBCheck 4.2.6 with zero errors/warnings; generated book-01 XHTML retains 209
ASCII quote pairs, and the matched passage page has the source's five ASCII
closing pairs with no curly double quote substitutions.

The local visual inspection report is
`reports/no-smart-quotes-compile-20261126/visual-book01.md`; rendered PDFs and
page images remain in ignored `generated/visual-book01-20261126/`. That report
also records that WIDE's current book design uses Georgia while the reference
uses Times New Roman, and that heading/section pagination differs. These are
not random font changes; matching the author's intended compile layout remains
unresolved. The compile-only three-book regression passed EPUBCheck 4.2.6 with
zero errors/warnings for every generated EPUB. `cargo fmt --all -- --check`,
workspace Clippy with warnings denied and the full offline workspace test suite
passed, including 337 terminal unit tests, 3 author workflows, 24 Linux PTY
tests and all library/CLI/doc tests. The Ollama test used only the previously
authorised local loopback socket. This renderer correction is ready to commit;
human visual and publication acceptance remain outstanding.

## Scrivener content-unit compile inclusion — implementation in progress

The next #68/#53 fidelity gap was that `IncludeInCompile=No` was retained for
scenes but ignored for prose stored directly on section/chapter content units.
Migration 52 adds recoverable per-content-unit inclusion state (included by
default). The unified outline now displays this state and Space toggles it
without removing the prose. The shared Compile IR omits excluded content units
from ordinary compilation, while named assembly item selection remains an
independent, explicit way to include selected text in an alternate edition.
The Scrivener importer maps an explicit `No` flag on section/chapter prose to
that state. Heading visibility and structural container inclusion remain
separate and are not conflated with body inclusion.

Focused storage, application, pipeline, importer, and outline-key regressions
pass, including reopen persistence, default compile omission, explicit assembly
inclusion, and synthetic Scrivener chapter-prose exclusion. Migration downgrade
tests were updated and rerun. Workspace Clippy and the final full offline
workspace suite passed: 338 terminal unit tests, 3 author workflows, 24 Linux
PTY tests, and all library, CLI, import, storage, Ollama and documentation tests.
The suite required the previously authorised permission for the Ollama test's
loopback-only fixture socket. This is not yet a complete per-profile Scrivener
compile-settings mapping or human-accepted workflow.

The full private three-book regression then ran at the dirty working revision
based on `ed6a40d`: `reports/content-inclusion2-20261126/`, with generated
projects and EPUBs under `generated/content-inclusion2-20261126/`. Output EPUBs:

- `generated/content-inclusion2-20261126/book-01.epub`
- `generated/content-inclusion2-20261126/book-02.epub`
- `generated/content-inclusion2-20261126/book-03.epub`

All three source trees were unchanged and fixture checksums passed. Import body
fidelity was exact for the 39/39, 36/36 and 68/68 manuscript bodies and 6/6,
6/6 and 12/12 research bodies respectively. Imports remain marked incomplete
because source mapping/unsupported-content review is unresolved. Generated
EPUBs each passed EPUBCheck 4.2.6 with zero errors and warnings. The run is
provisional: it does not establish that all Scrivener compile exclusions or
historical publication differences are correctly resolved, and human visual
publication acceptance remains outstanding.

## Proofreading command discoverability — implementation complete, UAT pending

The Review menu now exposes the existing offline spellcheck, grammar check and
grammar settings commands directly. Spellcheck is categorised with Review and
Proofreading & Diagnostics rather than Tools. Regression assertions verify both
the command taxonomy and its visible inclusion in the curated Review menu. A
real Linux PTY check now opens Review at 80x24, sees all three actions, selects
Spellcheck and verifies its prompt opens. The full terminal package passed:
338 unit tests, 3 author workflow tests and 25 real PTY tests; formatting and
terminal-package Clippy with warnings denied passed. This improves access to
existing checks; offline dictionary definitions and thesaurus support remain
unimplemented, and human menu usability verification remains outstanding.

## EPUB differential emphasis detail — implementation and targeted real comparison complete

The local EPUB XHTML comparison now records source-located semantic emphasis
from actual markup, including bold, italic, underline, strikethrough, subscript
and superscript. Equivalent XHTML forms (for example `<b>` and `<strong>`) are
normalised for comparison while original element paths remain in the detailed
report. Differences identify missing or additional emphasized text; manuscript
content is never altered. The full Python tooling suite passes (24 tests) and
all tooling modules compile. Regenerate reports during the next relevant real
manuscript run so they include this new field; earlier local reports do not.
The new comparator was then applied to the existing BowString candidate pair
without re-importing or modifying fixtures. Its fresh local report is
`reports/back-catalogue-targeted/bowstring-semantic-format-20261126c/differential.json`.
The candidate reference remains unconfirmed; the report found 1 reference and
2 generated semantic emphasis runs. This is a discrepancy lead requiring
source-side investigation, not a confirmed formatting defect or acceptance
result. This refreshed comparison reused the already imported/generated private
artefact; it did not repeat import, reopen or EPUBCheck. Fixture checksums passed
before and after comparison. The comparison keys now include the reading-order
document and containing paragraph, so emphasis moved to another paragraph is
reported instead of being counted as preserved.

## Current checkpoint — RTF underline and CSS-aware EPUB comparison, 26 November 2026

RTF underline controls were missing from the Scrivener importer. The importer
now converts underline runs into semantic Pandoc underline spans and composes
them with bold/italic/strike runs. The production renderer test checks HTML,
DOCX and LaTeX representations. The actual controlled-book EPUB comparison
also identified one underline recovered from book 02's RTF source; book 01
and book 03 contain no newly detected underline runs from this import path.

The differential tool now interprets simple direct CSS class rules for
semantic formatting. This prevents a historical EPUB's class-styled emphasis
from being treated as absent merely because WIDE emits semantic elements. It
does not simulate the full CSS cascade or establish visual equivalence.

Verification and outputs for the fresh three-book production run:

- Reports: `reports/rtf-underline-all-20261126/`
- EPUBs for local inspection: `generated/rtf-underline-all-20261126/book-01.epub`,
  `generated/rtf-underline-all-20261126/book-02.epub`,
  `generated/rtf-underline-all-20261126/book-03.epub`
- Fixture checksum verification passed before and after. All three source
  fingerprints remained unchanged.
- Mapped manuscript body counts preserved exactly after persistence/reopen:
  39/39, 36/36, and 68/68; research bodies 6/6, 6/6, and 12/12.
- EPUBCheck 4.2.6: each generated EPUB passed with 0 errors and 0 warnings.
- No unintended `<pre>` or `<code>` blocks were found.
- Historical semantic formatting comparison remains materially different:
  missing/additional semantic runs 177/115, 92/7, and 86/84 respectively.
  This is not a fidelity pass; differences still require investigation against
  the authoritative Scrivener sources and compile layout.
- `python3 -B -m unittest discover -s tools -v`: 25 tests passed.
- Focused importer and production renderer tests passed. Focused Clippy passed
  for `story-document` and `story-import`.
- `cargo fmt --all -- --check`, workspace Clippy with warnings denied, and
  `cargo test --offline --workspace -- --test-threads=1` all passed. The first
  non-escalated test run hit the sandbox's loopback bind restriction in the
  Ollama-only test; the full suite passed on retry with the previously
  authorised local socket permission. This included 338 terminal unit tests,
  3 author-workflow tests, 25 Linux PTY tests, all other workspace tests and
  doc tests.
- Sanitized implementation updates were subsequently posted to #7, #53, #61
  and #73 via GitHub REST, with the checked commit recorded.

The private regression run tested the dirty tree based on `622328c`, and must
not be attributed to a later source commit. Commit/push only the importer,
renderer test, comparator, regression tests and documentation. Do not stage
user changes in `.gitignore`, `README_TMP.md`, or `UAT/` and do not commit any
private corpus, generated files or reports.

## Terminal rendering of imported underline — implementation and workspace verification, 26 November 2026

The previous importer change stored Scrivener underline as a semantic Pandoc
`{.underline}` span, but the terminal's CommonMark projection could not parse
Pandoc attributes. This made the author-facing editor show the delimiters and
did not style the text. The projection now lowers only recognized underline
spans in temporary parser input, maps all resulting grapheme ranges back to
the untouched canonical Markdown, and renders underline distinctly from
strikethrough. Inline code and escaped literal markers are excluded from this
projection. Regression tests cover nested bold/underline, combining Unicode,
source ranges, and literal spans.

Verification: `cargo fmt --all -- --check`, workspace Clippy with warnings
denied, and `cargo test --offline --workspace -- --test-threads=1` passed.
The workspace run included all unit, integration, author-workflow and 25 Linux
PTY tests. No private manuscript files were used for this projection-only
change; the preceding real three-book import/compile evidence belongs to the
prior importer commit and is not claimed as verification of this editor fix.
The new behavior is implemented and automated verification passed; visual
human UAT remains outstanding.

## Configurable compile heading presentation — in progress, 26 November 2026

The shared Compile IR and named edition recipes now support generated-output
rules for showing or hiding section, chapter, generic group, scene and matter
headings. Edition recipes can replace chapter titles with a sequential
`{number}` template and insert a configured separator between consecutive
scenes when scene headings are hidden. These settings only affect generated
output; canonical titles and prose remain unchanged. This provides a general
mechanism for internal container headings, numbered chapter placeholders and
scene-boundary presentation without matching literal binder names.

Focused document and application tests pass, including a saved named
configuration that suppresses internal section and scene titles, numbers two
chapters, inserts scene separators, and verifies the stored prose is unchanged.
The full workspace suite passed (338 terminal unit tests, 3 author-workflow
tests and 25 Linux PTY tests, plus all other workspace and doc tests). Workspace
Clippy with warnings denied and formatting passed. A focused re-run against the
final template-validation code passed. The full run was based on commit
`968316e` with the compile-presentation changes in a dirty working tree.

Fresh production regressions for all three controlled books are recorded in
`reports/compile-presentation-20261126/`; generated EPUBs are
`generated/compile-presentation-20261126/book-01.epub`,
`generated/compile-presentation-20261126/book-02.epub`, and
`generated/compile-presentation-20261126/book-03.epub`. Import bodies survived
save/reopen exactly (39, 36 and 68 manuscript bodies; 6, 6 and 12 research
bodies); all source fingerprints were unchanged. EPUBCheck 4.2.6 reported zero
errors and warnings for all three generated EPUBs. The run is provisional: all
three imports report `incomplete`, and historical publication discrepancies
remain. The generated EPUBs use default presentation; the new policy itself is
verified by synthetic output tests, not by Scrivener-specific profile mapping.
Fixture checksums passed before and after.

Scrivener's source format/layout settings are still inventoried but are not
translated automatically into these WIDE presentation rules; the user must
configure a named recipe. Per-Scrivener format profiles, front/back matter
selection and a guided terminal configuration editor remain outstanding. This
is partial #7/#53/#68/#73 work, not a claim that Scrivener publication mapping
is complete. The human publication fidelity gate remains failed/pending review.

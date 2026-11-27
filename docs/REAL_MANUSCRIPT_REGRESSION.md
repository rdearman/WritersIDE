# Private real-manuscript regression suite

This local-only suite exercises the production Scrivener importer, project
storage, reopen path and publication compiler against three private reference
projects. The source books and historical publications are copyrighted test
data. Never commit fixture files, generated manuscripts/publications or
content-bearing reports, and never include manuscript excerpts in public issue
comments. `fixtures/`, `generated/`, `reports/` and `baselines/` must remain
root-relative Git exclusions.

## Source authority and comparison policy

The current Scrivener source is authoritative for import fidelity. Recoverable
compile inclusion/exclusion settings must be preserved and respected. The
historical EPUBs predate edits to the Scrivener projects, so they are evidence
of presentation and structure rather than exact textual oracles. Calibre may
also have transformed earlier editions. Investigate discrepancies against the
source binder and compile settings. Do not edit imported prose to match an old
publication or silently dismiss mismatches as revision differences.

The importer records binder `IncludeInCompile` flags. Inspection of the three
current projects found the flag set to `Yes` on every inventoried binder item.
The `.scrformat` files also contain `Statistics/ExcludeFrontMatter` and
`Statistics/ExcludeBackMatter`. These are word-count/statistics options, not
publication membership switches; they must not be used to suppress selected
front or back matter. The importer now preserves these values as typed
statistics metadata, and a regression test verifies that setting them does not
remove explicitly selected matter from a publication profile. Scrivener's
compile overview separately selects front/back matter and assigns section
types to layouts. Those assignments are stored in `compile.xml`; the selected
formats in the current fixtures have incomplete or absent assignments, which
WIDE reports rather than guessing. Therefore the generated publications must
not be considered proof that all Scrivener layout and inclusion settings have
been preserved. Resolve the remaining source assignment gaps before claiming
source-to-publication fidelity.

User clarification: additional back matter, corrected websites/email links and
newsletter information may exist in the Scrivener projects but not in the
historical EPUBs. Some binder items may have been deliberately excluded from
compilation. Preserve the current source, report unresolved mapping decisions,
and distinguish source revisions from import or compile defects with evidence.

## Reference validation

EPUBCheck 4.2.6 reports these defects in the historical EPUB 2 references.
They are reference-only findings and must not be copied into WIDE's EPUB 3
output:

| Reference | Errors | Warnings | Known reference-only diagnostics |
| --- | ---: | ---: | --- |
| Book 01 | 2 | 0 | OPF-054 placeholder date; RSC-020 malformed mailto URI |
| Book 02 | 2 | 0 | RSC-005 invalid paragraph/list nesting; RSC-020 malformed mailto URI |
| Book 03 | 0 | 0 | None |

Generated EPUBs must have zero errors and warnings. The harness records
reference and generated validator results separately. It resolves
`epubcheck` on `PATH`; when `/usr/bin/epubcheck` resolves to the installed JAR,
the harness invokes it with Java. Current development setup: EPUBCheck 4.2.6,
JAR at `/usr/share/java/epubcheck.jar`.

## Running tests

First verify the protected fixture checksum manifest. Do this before and after
each complete private run:

```sh
(cd fixtures && sha256sum --quiet -c SHA256SUMS)
```

The test harness creates a unique directory under `generated/` and `reports/`
for each run. Never reuse a run ID. Paths default to the repository-relative
`fixtures/`, `generated/` and `reports/` directories; override them only when
needed.

```sh
# Inventory without importing
python3 tools/real_manuscript_regression.py --phase inspect

# One book or all three: inspect, import, save/reopen, compile and compare
python3 tools/real_manuscript_regression.py --book book-02 --phase full
python3 tools/real_manuscript_regression.py --phase full

# Import only
python3 tools/real_manuscript_regression.py --phase import

# Compile projects from an earlier import run. The root contains one
# book-01-project, book-02-project and book-03-project directory.
python3 tools/real_manuscript_regression.py --phase compile \
  --project-root generated/IMPORT_RUN_ID

# Synthetic public tests do not require private fixtures
python3 -m unittest discover -s tools -p 'test_real_manuscript_regression.py'
```

## Expanded private back-catalogue

The uncurated corpus under `fixtures/back-catalogue/` is additional test data,
not a replacement for the three controlled books above. Its project/reference
relationships are ranked candidates only. A close title or text match does not
prove edition lineage. Rebuild its local content-bearing manifest with:

```sh
cargo build --offline -p writers-ide
python3 tools/back_catalogue_inventory.py \
  --cli target/debug/writers-ide \
  --output reports/back-catalogue/RUN_ID/manifest.json
```

For a targeted production import/reopen/compile test, supply a reviewed mapping
JSON. This prevents uncertain binder items from being silently assigned an
authoritative manuscript role. An optional reference is explicitly reported
as an unconfirmed candidate unless edition lineage has independent evidence:

```sh
python3 tools/expanded_scrivener_regression.py \
  --source 'fixtures/back-catalogue/scrivener/PROJECT.scriv' \
  --mapping reports/back-catalogue/RUN_ID/reviewed-mapping.json \
  --book-type nonfiction \
  --reference 'fixtures/back-catalogue/epub/CANDIDATE.epub' \
  --run-id UNIQUE_RUN_ID
```

The targeted harness uses the production CLI and the same local EPUB
comparators as the controlled suite. It refuses to reuse run directories,
fingerprints the selected source project, checks `fixtures/SHA256SUMS` before
and after, stores full source-derived reports only under ignored
`reports/back-catalogue/`, and puts imported projects and publications under
ignored `generated/back-catalogue/`. A missing or incomplete mapping is a test
failure; inspect the source inventory, then review the plan explicitly.
Unmatched or unfinished projects can still be tested for import-body fidelity,
reopen integrity, source preservation and generated EPUB validity without
supplying a historical publication.

Reports contain private paths, structural measurements and detailed local
validator diagnostics. Do not copy whole reports into GitHub. Sanitised issue
updates may report code-level changes, status counts and test outcomes only.

## Current measured result — 25 November 2026

The full run `reports/list-marker-all-books-20261125/` used the production
pipeline. Fixture checksums passed. Source bodies survived save/reopen exactly:
book 01 44/44, book 02 41/41 and book 03 76/76; research bodies survived 1/1,
1/1 and 4/4. All source fingerprints were unchanged. SQLite integrity checks
returned `ok`. Generated EPUBs passed EPUBCheck 4.2.6 with zero errors and
warnings; reference errors were 2/2/0. The Scrivener RTF list-marker repair
restored the affected book's measured list items from 36 to 45, matching the
historical reference's count. This is evidence for that formatting feature,
not general publication equivalence.

That full run's JSON `acceptance` text was generated before a later harness
wording fix and still says EPUBCheck is required. The compile-only rerun
`reports/list-marker-compile-correct-status-rerun-20261125/` recompiled all
three persisted projects and verified the accurate status: all three generated
EPUBs passed EPUBCheck, with discrepancy triage and human review still
outstanding. The harness helper placement was subsequently corrected and
Python tests passed; do not use the stale acceptance field in the first report
as validation status.

Material text, paragraph, navigation, image-scope and inclusion differences
remain unresolved. These results do not constitute import/publication fidelity
acceptance or a promoted WIDE baseline. Human review of generated publications
is still required.

## Compile-variable and project-metadata verification — 26 November 2026

Standalone Scrivener imports now preserve available project author, language,
genre and synopsis metadata without inventing absent values. Project title
metadata supplies the suggested destination title, while an author-provided
title takes precedence. Existing-project imports do not overwrite project
metadata.

The shared compiler resolves the currently supported title/author variables
and sequential number token from Compile IR. Replacement is render-only:
canonical Markdown remains unchanged, source hashes remain tied to the source,
and diagnostic ranges use original UTF-8 byte offsets. Missing supported
metadata and unsupported Scrivener variables are reported as source-linked
warnings. Format-specific Scrivener variable syntax, section layout rules,
numbering schemes and compile selection are not fully mapped yet.

Run a fresh private regression with:

```sh
(cd fixtures && sha256sum --quiet -c SHA256SUMS)
python3 tools/real_manuscript_regression.py --book all --phase full \
  --run-id UNIQUE_RUN_ID
(cd fixtures && sha256sum --quiet -c SHA256SUMS)
```

The 26 November run `reports/compile-vars-full-20261126/` used the production
import, save/reopen and compile paths. Its generated EPUBs are under
`generated/compile-vars-full-20261126/`; each passed EPUBCheck 4.2.6 with zero
errors and warnings. All checked manuscript and research bodies matched their
source extraction exactly after reopen, and source fingerprints did not
change. Import status was incomplete for all three projects. Historical EPUBs
remain invalid in the first two cases (two reference errors each); those are
reference-only and must not be copied into generated EPUBs.

This run does not establish publication fidelity. Meaningful differences in
text sequence, paragraph count, headings, navigation, compile inclusion and
structure remain. The current Book 03 output still publishes internal
structural headings and literal chapter placeholders; metadata-variable
resolution alone is not the approved Scrivener compile-layout implementation.
Use the local detailed comparison in the run report to trace each discrepancy
to source extraction, canonical storage, Compile IR or renderer before
classifying it. Human publication acceptance remains outstanding.

The differential report also lists concrete added/removed CSS declarations,
font properties, publication metadata and ordered navigation entries. For each
changed paragraph it records both XHTML document/path locations, the expected
and generated paragraph text, and Unicode character edits. A compile-only run
rechecks extracted source bodies against persisted WIDE documents; if all
checked bodies are exact it narrows the candidate causes to historical edition
revision/conversion, Scrivener compile selection, Compile IR structure or the
renderer. If source bodies differ, extraction/import and persistence remain
candidates. These stage hints are evidence-based leads, not automatic defect
classifications. The report also explicitly notes that CSS declaration
comparison is not a full browser cascade or visual layout test.

The report also records semantic inline emphasis (bold, italic, underline,
strikethrough, superscript and subscript) with XHTML element paths and compares
the marked text after normalising equivalent elements such as `<b>` and
`<strong>`. This exposes missing or added emphasis that tag totals alone would
miss. Comparison includes spine order and containing block text, so identical
phrases moved to a different paragraph are not counted as preserved. It remains
structural evidence; visual layout still requires rendering.

The exact-commit compile-only differential rerun is
`reports/diff-stage-20261126/`; generated publications are
`generated/diff-stage-20261126/book-01.epub`,
`generated/diff-stage-20261126/book-02.epub` and
`generated/diff-stage-20261126/book-03.epub`. It rechecked 39/39, 36/36 and
68/68 manuscript bodies after persistence/reopen, reported zero missing or
changed destinations, and verified zero EPUBCheck errors and warnings for all
three generated EPUBs. Substantial historical-reference and compile-layout
differences remain; use the run's local reports for investigation.

## Integrity and privacy

- Never modify the original Scrivener projects, reference publications or
  checksum manifest. Treat them as read-only.
- Use only isolated project copies in `generated/`; the harness sets a private
  `XDG_CONFIG_HOME` and Cargo offline mode.
- Keep detailed reports local and ignored by Git.
- Public CI must use synthetic or redistributable fixtures. The private books
  are optional local tests, never a public-CI dependency.
- A missing fixture or unavailable validator is explicitly unverified, not a
  pass. Provisional baselines must not replace a better result automatically.

## Expanded private back-catalogue

The optional, uncurated corpus under `fixtures/back-catalogue/` supplements,
but does not replace, the three controlled books above. Inventory it with the
production `inspect-scrivener` command and cautious local matching:

```sh
(cd fixtures && sha256sum --quiet -c SHA256SUMS)
python3 tools/back_catalogue_inventory.py \
  --output reports/back-catalogue-RUN_ID/manifest.json
(cd fixtures && sha256sum --quiet -c SHA256SUMS)
python3 -m unittest discover -s tools -p 'test_back_catalogue_inventory.py'
```

The manifest records project fingerprints, source-reported Scrivener creator
tags, binder type and depth counts, source inclusion flags, text-size and
language hints, publication metadata and checksums. It separates candidate
publication documents from supporting images. Project/edition matches and
similar project pairs are ranked leads only; inspect source and revision
evidence before confirming them. A matching publication is not assumed to be
the exact edition compiled from the current Scrivener project. The harness
does not execute files found in the corpus. Any conversion is staged beneath
ignored `generated/`; reports remain local beneath ignored `reports/`.

The local EPUB comparator records paragraph-level changes, exact changed
character sequences, Unicode codepoints/names, document paths and structural
details. Historical EPUB XHTML that fails XML parsing is analysed through a
tolerant HTML tree recovery pass; the original parse error remains in the
report, and recovered values are evidence rather than a claim that the source
publication is standards-compliant. The recovery pass is covered by synthetic
malformed list nesting and mailto-attribute tests. It does not alter source or
reference files. See the exact-commit example report
`reports/e4ac09e-exact-diff/` for local content-bearing output.

For a manually mapped candidate, use the production import CLI with a reviewed
mapping JSON, then validate, reopen, compile and compare its publication. Do
not invent a generic mapping merely to make an unfinished or ambiguous project
import. Private local run `reports/back-catalogue-20261126-expanded-v2/`
inventoried 26 Scrivener manifests (one nested project), 45 publication
candidates and 2 supporting image assets; all 26 projects were inspected and
no exact normalized text duplicates were found. This count includes the
controlled projects also present in the expanded corpus.

The targeted back-catalogue import used an explicit local test mapping, not an
author-approved publication layout. It exposed a compiler defect: same-titled
structural and manuscript headings received duplicate XHTML IDs, causing
EPUBCheck errors and a navigation-order warning. The Pandoc renderer
now assigns stable distinct IDs derived from source entity identity to
generated structural and unit headings in HTML-family formats; Markdown and
plain-text exports remain unchanged. Synthetic repeated-heading coverage and
a fresh local project compile verified unique XHTML IDs and zero EPUBCheck
errors or warnings. See the current `DEVELOPMENT_STATUS.md` handover for the
run directory. This output validates the repeated-heading renderer path only;
it does not establish the correct compile mapping for that project. This
targeted case and the three controlled books remain provisional comparisons;
they do not establish human publication acceptance.

## 25 November 2026 — publication UAT failure and safe-import checkpoint

The author's Calibre review of an earlier set of generated publications failed
human fidelity acceptance. EPUBCheck passing is not publication acceptance.
The current Scrivener projects are authoritative for import; the historical
EPUBs may predate source edits and are comparison evidence only. Do not edit
imported text to force a match to those editions.

Production RTF extraction now consumes the complete `\uN` fallback according
to scoped `\ucN`, decodes consecutive hex byte escapes using the declared
`\ansicpg`/`\cpg` encoding, and retains ordinary numeric prose. Synthetic
tests cover curly quotes, French accents, copyright, literal 92/93/94 digits,
zero and multiple fallbacks, UTF-8 byte escapes, and Korean text. Full
font-table-specific code-page handling and UTF-16 surrogate-pair coverage
remain limitations to investigate.

Scrivener `DraftFolder`, `ResearchFolder`, and `TrashFolder` roles are now
classified from their structural type. Content outside a recognised area is
retained as private Research by default and flagged for author review. It is
not made publishable merely because Scrivener marks it included or because it
is in the binder. The import regression harness uses the same conservative
classification. Unknown items may be explicitly remapped during review; their
prose remains preserved in the project.

Run `uat-publication-safe-20261125` used commit
`09dceb278426571b7cc4f29b64e58f7224a961b0` plus the uncommitted changes under
test. Fixture checksums passed, all three source fingerprints were unchanged,
and the imported manuscript and research bodies matched the source exactly:
book 01 39/39 and 6/6; book 02 36/36 and 6/6; book 03 68/68 and 12/12.
Five, four, and eight long-string checks of unknown/private content respectively
found no such content in the generated EPUBs. These are targeted leak checks,
not proof of total publication equivalence. All three generated EPUB 3.2 files
passed EPUBCheck 4.2.6 with zero errors and warnings. Historical reference EPUB
validation remains separate: error counts 2/2/0 and zero warnings.

Local EPUBs for human inspection (private outputs; do not commit):

- `generated/uat-publication-safe-20261125/book-01.epub`
- `generated/uat-publication-safe-20261125/book-02.epub`
- `generated/uat-publication-safe-20261125/book-03.epub`

The machine report and per-book mapping inventories are in
`reports/uat-publication-safe-20261125/`. The run predates the final synthetic
test-fixture correction in the working tree. Token sequence similarity to the
historical references was 0.965808, 0.931482, and 0.953626; substantial
differences remain and are not accepted as fidelity. Source/reopen exactness
does not establish correct Scrivener compile selection, chapter layouts,
variables, front/back matter, or publication structure. The next publication
work must recover those settings from project/format metadata and validate the
shared Compile IR before adding exporter-specific behavior.

## 25 November 2026 — Scrivener compile metadata inventory correction

The production inventory now reads binder `IncludeInCompile` and `SectionType`
from each binder item's own metadata scope. It also inventories project
metadata, project content scope, format-level section layout mappings, and
front/back matter selection flags from `Settings/compile.xml`. This avoids the
recursive-field ambiguity where a container could inherit a nested document's
`IncludeInCompile` value. It does **not** yet apply these settings to WIDE's
shared Compile IR or a selected WIDE publication profile. Do not claim chapter
numbering, hidden organisational headings, scene separators or retailer-specific
matter are recovered until they have source-linked integration tests and pass
the fresh three-book publication regression.

The synthetic test
`scrivener_compile_settings_and_binder_flags_are_scoped_to_their_owner` covers
independent parent/child flags, project metadata, layout mappings and front/back
matter switches. The inspect-only real-fixture run is recorded under
`reports/20261125T224346Z/`; its output includes private project metadata and
must remain local and ignored. Fixture checksums passed before and after the
run.

## 25 November 2026 — paragraph indentation differential analysis

The human Calibre review of `generated/list-marker-compile-correct-status-rerun-20261125/`
found ordinary book-01 prose rendered as preformatted code. Differential
analysis now reports per-XHTML-document paragraphs and source locations,
headings, pre/code blocks, literal Markdown delimiters, lists, tables, images,
links, Unicode punctuation and CSS typography. It also compares paragraph
sequences and character-level Unicode changes between reference and generated
EPUBs. Historical-publication differences are clues for investigation; the
source RTF and imported/reopened project remain the authorities for source
fidelity.

Tracing the source showed paragraph indentation serialized into canonical
Markdown as leading mixed whitespace such as a space followed by a tab. The
Markdown renderer consequently interpreted ordinary prose as a code block.
The earlier correction handled a tab at byte zero, but a fresh targeted import
still produced 110 preformatted blocks. The corrected importer now recognizes
leading RTF paragraph indentation runs containing tabs or at least four ASCII
spaces and encodes those as non-breaking spaces. Tabs inside paragraph text
remain intact. It does not rewrite prose or add a book-specific workaround.
Synthetic coverage is in
`scrivener_paragraph_indent_does_not_become_a_markdown_code_block`.

The full fresh production import/save/reopen/compile run is:
`reports/rtf-indent-full-20261125/`, with generated EPUBs in
`generated/rtf-indent-full-20261125/`. The original three-fixture checksum
manifest passed before and after the run. Persisted source bodies matched
exactly for book 01 39/39, book 02 36/36 and book 03 68/68. Generated
preformatted block counts were 0/0/0; book 02 retained 44 images. All three
generated EPUBs passed EPUBCheck 4.2.6 with zero errors or warnings. A
full-workspace `cargo test --workspace --offline` passed after this correction,
including 337 terminal unit tests, 3 author workflow tests and 24 real-PTY
tests. `cargo fmt`, Clippy with warnings denied, and 9 Python harness tests also
passed for the batch.

This closes the confirmed unintended-code-formatting defect at the import
serialization layer. It does not resolve broader publication fidelity:
historical text, paragraph, CSS, layout, navigation, variables, Scrivener
compile semantics, publication profiles and inclusion discrepancies remain
subject to source-level investigation and human acceptance. The detailed local
report contains private text and stays under ignored `reports/`; do not attach
it to issues or commit it.

The harness writes detailed local JSON and readable discrepancy reports,
including expected/actual paragraph text and source document/path. A compact
summary only records structural counts and validator status. Visual layout
comparison remains follow-up work where equivalent reference sections and
safe local rendering tools are available.

## Expanded private back-catalogue inventory — 26 November 2026

The additional corpus is separate from the three controlled regression books
and is deliberately not considered a curated reference set. Generate a local
manifest after building the production CLI:

```sh
(cd fixtures && sha256sum --quiet -c SHA256SUMS)
python3 -B tools/back_catalogue_inventory.py \
  --output reports/back-catalogue-RUN_ID/manifest.json
(cd fixtures && sha256sum --quiet -c SHA256SUMS)
python3 -B -m unittest discover -s tools -p 'test_back_catalogue_inventory.py' -v
```

The tool recursively uses the production read-only `inspect-scrivener` command,
records per-project and per-publication SHA-256 values, counts binder structures,
extracts available metadata/text without executing corpus files, and ranks
publication associations using names, metadata and normalized three-word
content shingles. It also compares candidate projects to the original three
controlled books and lists possible related revisions. Similarity results are
not confirmed matches. The report is local and content-bearing; never commit
it. Extracted MOBI text, when Calibre is available, is staged under ignored
`generated/`.

The latest read-only inventory is
`reports/back-catalogue-20261126-release/manifest.json`. Temporary MOBI text
was removed after content matching. It found 25 top-level
`.scriv` folders containing 26 `.scrivx` manifests (one nested project), and
47 publication/resource files: 13 EPUB, 16 MOBI, 4 ODT, 4 RTF, 2 DOCX, 2 HTML,
2 TXT, 1 DOC, 1 PDF and 2 image files. The Scrivener Creator tags indicate 24
projects tagged `SCRWIN-3.1.5.1`, one `SCRWIN-3.1.1.0`, and one `SCRWIN-1.0.0`;
these are source-reported Creator/version hints, not independently verified
application/platform compatibility. Manifest revisions are 2.0 for 25 and 1.0
for one. Binder item counts range from 5 to 129 (1,322 total); extracted
content ranges from empty to approximately 70,829 words. The local detector
suggests English for 24, with one Latin-script language unclassified and one
CJK-script project whose language remains uncertain. Treat all detector output
as candidate metadata, not ground truth.

The top-ranked project/publication candidates comprise 6 strong, 6 possible
and 14 uncertain suggestions; all remain unconfirmed pending source/edition
inspection. Each project also has lower-ranked suggestions, which are not
asserted as matches. Exact normalized-text duplicate groups were not found.
Pairwise content-shingle analysis produced potential
relationships among the archery manuscripts; these are candidate shared-source
or revision relationships, not proof of duplicate editions. Each original
controlled project had an exact normalized-content match to its corresponding
back-catalogue project. The same 26 project-tree checksums and 47 publication
file checksums matched across the initial and final inventory passes. The
controlled fixture checksum manifest also passed before and after; the corpus
contains no symlinks and Git tracks no back-catalogue files. Text extraction
was available for all publications except two image-only resources; all four
ODT files were parsed for text and metadata. The inventory's synthetic suite
passed 5 tests, and the complete Python harness suite passed 14 tests.

Refreshed expanded-corpus inventory, 26 November 2026:
`reports/back-catalogue-20261126-expanded-final/manifest.json`. It reinspected
all 26 Scrivener manifests and 47 publication/resource files, with zero
inspection failures, zero exact normalized-text duplicate groups, 130 ranked
project/publication candidates and 85 possible related-project pairs. These
candidate relationships remain unconfirmed; no source files were executed or
modified, and the fixture SHA-256 manifest passed after inventory.

This inventory is useful for selecting targeted compatibility tests, not a
publication fidelity result. It does not import/compile all 26 projects, and
historical formats lacking extractable text or matching evidence remain
unmatched. Use small targeted subsets first; do a corpus-wide sweep at the next
significant import/compile milestone.

## 26 November 2026 — RTF punctuation control words

The second human UAT reported a missing en dash in book 02. A fresh production
inventory established that the source RTF uses named controls (`\endash` and
related punctuation controls), which the previous parser silently discarded.
The importer now maps RTF en/em dashes, single/double quotation controls,
bullets, non-breaking spaces, non-breaking hyphens and discretionary hyphens to
their corresponding Unicode characters. It does not rewrite manuscript text.
Synthetic coverage is
`scrivener_rtf_named_punctuation_controls_preserve_dashes_quotes_and_special_hyphens`.

The targeted fresh book-02 run is
`reports/rtf-punctuation-book02-20261126/`; the full fresh three-book run is
`reports/rtf-punctuation-all-20261126/`, with EPUBs in
`generated/rtf-punctuation-all-20261126/`. Checksums passed pre/post. All
persisted manuscript and research bodies exactly matched importer output.
The en dash and reported date range survived into generated EPUB XHTML; 44
illustration/figure placements and 43 distinct image payloads remain. All
three EPUBs passed EPUBCheck 4.2.6 with zero errors/warnings and have no
unintended `<pre>` elements. The full offline workspace suite passed,
including 337 terminal tests, 3 author journeys and 24 PTY tests; formatting,
Clippy with warnings denied and all 14 Python harness tests passed.

This repairs one source-to-import punctuation defect and confirms no detected
regression in the controlled books. Publication human acceptance remains open;
Scrivener compile inclusion/layout, variables, alternative publication matter,
formatting and remaining differential discrepancies still require work and
review.

## 26 November 2026 — RTF underline and CSS-aware emphasis comparison

The importer now preserves Scrivener RTF underline controls as a semantic
Pandoc span, composable with bold, italic and strike formatting. Synthetic
coverage verifies underline start/end controls and combined bold/underline;
the production renderer test verifies semantic underline in HTML, DOCX and
LaTeX output. The differential analyser also reads simple CSS class rules for
semantic emphasis, so historical EPUBs that encode bold/italic/underline via
classes can be compared with generated EPUBs using semantic tags. This is a
focused semantic comparison, not a complete CSS cascade/layout engine.

Fresh three-book import, persistence/reopen and compile run:

- Reports: `reports/rtf-underline-all-20261126/`
- Generated EPUBs: `generated/rtf-underline-all-20261126/book-01.epub`,
  `generated/rtf-underline-all-20261126/book-02.epub`, and
  `generated/rtf-underline-all-20261126/book-03.epub`
- EPUBCheck 4.2.6: all three generated EPUBs pass with zero errors/warnings.
- Current source fingerprints and fixture checksum manifest: unchanged.
- Mapped included source bodies persisted exactly: book 01 39/39, book 02
  36/36, book 03 68/68; research bodies: 6/6, 6/6, 12/12.
- The generated book 02 contains the semantic underline recovered from its
  RTF source; all three outputs have zero unintended preformatted/code blocks.
- Comparison against historical EPUBs still reports semantic-formatting
  differences (missing/additional runs: book 01 177/115, book 02 92/7,
  book 03 86/84). These counts are investigation evidence, not proof of a
  WIDE defect or a publication-fidelity pass; historical source revisions and
  formatting/layout mapping remain to be classified.

The real comparison ran with an uncommitted working tree based on commit
`622328c`; do not associate its generated EPUBs with a later commit without
rerunning. Detailed reports and artifacts are private and remain untracked.

## 26 November 2026 — RTF emphasis whitespace

Book 01's fresh generated EPUB exposed literal Markdown bold markers when a
Scrivener bold RTF run included leading or trailing whitespace. The importer
now leaves that whitespace outside the Markdown emphasis delimiters, so the
renderer receives valid semantic emphasis. The synthetic regression is
`scrivener_rtf_emphasis_keeps_surrounding_whitespace_outside_markdown_delimiters`.

The full production run is `reports/rtf-emphasis-all-20261126/`; the three EPUBs
are in `generated/rtf-emphasis-all-20261126/`. The checked manuscript and
research bodies matched exactly across save/reopen (39/39, 36/36, 68/68 and
6/6, 6/6, 12/12). EPUBCheck 4.2.6 reported zero errors/warnings for all three;
no generated output contained `<pre>` blocks, and book 01 no longer has visible
asterisk formatting markers. A `***` paragraph in book 03 is present as
escaped literal content in the canonical source imported from RTF, so it is
source-origin, not introduced by this change; its intended scene-break meaning
remains a separate compile-layout question. Fixture checksums passed before
and after the run. The detailed reports and EPUBs are private local data and
must remain untracked. This repair is not overall publication acceptance.

## 26 November 2026 — retain compile-excluded Scrivener scenes

Scrivener binder items marked `IncludeInCompile=No` were previously proposed as
`Skip`, which removed them from the imported project. The importer now retains
such items; when mapped as scenes, it applies WIDE's existing persistent scene
exclusion so the content remains editable but stays out of default compilation.
Synthetic importer and application tests verify this. This does not yet apply
Scrivener compile flags to structural headings or chapter-level prose, or
recover per-format inclusion overrides.

The fresh three-book run is
`reports/scrivener-inclusion-retain-20261126/`, with EPUBs in
`generated/scrivener-inclusion-retain-20261126/`. All source fixture checksums
passed before and after; all checked source bodies matched after persistence,
and generated EPUBCheck 4.2.6 reported zero errors/warnings. The controlled
projects have `IncludeInCompile=Yes` for inventoried binder items, so these
results show no regression in them; the excluded-item behaviour is covered by
synthetic fixtures. Publication fidelity and human acceptance remain open.

## 26 November 2026 — preserve source quotation characters at compile

Visual/differential analysis found Pandoc's implicit Markdown `smart` extension
changed ASCII closing quote pairs to curly quotes while the matching legacy
opening double-backticks remained literal. The RTF source and canonical
Markdown retain the author's ASCII characters; the transformation originated
in the WIDE renderer. The production Pandoc input format now disables implicit
smart typography. A synthetic HTML regression verifies the original quote
pairs and apostrophes survive unchanged.

The three persisted projects were recompiled through the production pipeline:
`reports/no-smart-quotes-compile-20261126/`, with generated EPUBs in
`generated/no-smart-quotes-compile-20261126/`. All three passed EPUBCheck 4.2.6
with zero errors/warnings; no generated output contains unexpected
preformatted blocks. The book-01 generated XHTML retains the source's ASCII
quote pairs. Local PDF rendering and visual comparison were performed with
Calibre 9.17.0 and Poppler; detailed comparisons and generated page images are
in ignored local-only paths documented in `DEVELOPMENT_STATUS.md`. Current
WIDE book design typography and heading/layout differences from the historical
reference remain unresolved; the historical edition is not an exact text
baseline. No source files were modified.

## 26 November 2026 — Compile heading presentation and terminal underline

The follow-up editor projection change is committed as `968316e`; it hides
Pandoc underline delimiters in the rendered terminal editor while mapping text
back to unchanged canonical Markdown. The shared Compile IR heading-presentation
slice is committed as `365c098`. Its fresh controlled regression is
`reports/compile-presentation-20261126/`; generated EPUBs are in the matching
`generated/compile-presentation-20261126/` directory. All three EPUBs passed
EPUBCheck 4.2.6, but their import status remains incomplete and publication
fidelity remains provisional. See `docs/DEVELOPMENT_STATUS.md` for exact body
counts, checksums, limitations and feature status. These generated artifacts
used default presentation, so they do not establish Scrivener-specific layout
equivalence or human acceptance.

## 26 November 2026 — manuscript/research destination classification

The Book 03 source-body verifier now classifies a source item with a manuscript
destination (`scene` or `content_unit`) as manuscript content even if the
importer also creates a secondary `reading` index record. This corrects the
earlier 68/68 manuscript and 10/12 research result: it was a verifier category
error, not missing imported bodies. Synthetic tests cover mixed destinations,
front matter whose source area is unknown, research and unmapped content.

The targeted compile-only rerun is
`reports/scrivener-profile-fidelity-rerun-20261126/`; its generated EPUB is
`generated/scrivener-profile-fidelity-rerun-20261126/book-03.epub`. It reports
70/70 manuscript and 10/10 research bodies exactly preserved after reopen,
with no missing or changed bodies. EPUBCheck 4.2.6 reports zero errors and
warnings. The report remains provisional because it uses a prior same-day
import, does not resolve source layout assignments, and is not human accepted.

The subsequent full Book 03 production run after profile-warning scoping is
`reports/scrivener-layout-warning-20261126/`. It freshly imported, persisted,
reopened and compiled the project. Its profile warnings now name only selected
source items with unresolved layouts; private research and unselected material
do not contribute. Source-body preservation remains 70/70 manuscript and 10/10
research, and EPUBCheck remains clean. See `DEVELOPMENT_STATUS.md` for the
verification details and the exact remaining limitation.

## Scrivener binder-level layout defaults — 27 November 2026

Scrivener `.scrivx` manifests can assign default section types by binder
category (`Folders`, `Containers`, and `Files`) under `LevelTypes`; an
individual item's `SectionType` is an override. WIDE's importer now retains
these defaults and the shared compile-profile translator resolves an explicit
item override first, then the matching binder-category default. Layout
references resolve by exact source layout ID or by a unique exact layout name.
Ambiguous names remain a warning. This retains source semantics without
inferring layout meaning from binder titles.

The targeted controlled Book 03 run is
`reports/level-types-book03-20261127-02/`; its generated EPUB is
`generated/level-types-book03-20261127-02/book-03.epub`. The production import,
save/reopen and compile path preserved 70/70 manuscript bodies and 10/10
research bodies, with zero missing or changed destinations. The source
fingerprint and fixture checksum manifest were unchanged, and EPUBCheck 4.2.6
reported zero generated EPUB errors or warnings. The run remains provisional:
the default profile still has three selected section types without assignments
and eight assigned layout references whose built-in layout definitions are
not available in the project; the alternate profile has thirteen selected
types without assignments. WIDE reports these gaps rather than guessing. The
exact-commit rerun is `reports/level-types-book03-ab42c04-20261127/`, tested at
`ab42c04760397d7be109125e5eab9481e6d02a5a`. Its harness marks the working tree
dirty because the author's untracked `README_TMP.md` and `UAT/` are present;
`git status` showed no modified tracked files.

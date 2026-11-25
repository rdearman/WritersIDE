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
current projects found the flag set to `Yes` on every inventoried binder item;
the active Scrivener compile formats also set `ExcludeFrontMatter=Yes` and
`ExcludeBackMatter=Yes`. The effect of those format-level switches and the
source section-type/layout mapping is not yet represented or verified in
WIDE's compilation decisions. Therefore the current generated publications
must not be considered proof that all Scrivener inclusion settings have been
preserved. Resolve this mapping before claiming source-to-publication fidelity.

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

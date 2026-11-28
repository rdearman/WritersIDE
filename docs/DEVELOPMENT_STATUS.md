# Writer's IDE Development Status

## Safe sentence deletion through the production terminal — 28 November 2026

Added a Linux PTY regression for the reported multi-line Delete Sentence hazard.
The test creates a four-line unpunctuated passage, selects Edit → Delete
Sentence through the visible application menu, confirms only the caret's line
is removed, saves, exits and reopens the project to verify exact persisted
text. The production PTY test passes. This verifies the safe fallback for
unpunctuated multiline text; it does not establish broader editor usability or
human acceptance of sentence-selection semantics.

The same checkpoint adds a production PTY regression for Outline → Convert
Selected Section or Chapter. It selects an actual section in the rendered
outline, chooses Chapter in the conversion dialogue, confirms the visible
change, closes the application and reopens the project. The assertion verifies
that the converted node retained its stable ID, title and sibling position.
Both new focused PTY workflows pass; full-workspace verification is pending for
this test-only increment.

The replace-preview confirmation test previously used F5 for applying the
preview, despite Enter already being the supported ordinary confirmation key.
It now explicitly previews and applies with Enter, guarding against an
accidental function-key-only workflow. The focused test passes. The complete
terminal binary unit suite also passes (386/386); the workspace integration
suite was not conclusively completed in this run.

Reference Library navigation now has a two-step Escape route: Escape closes an
open reference back to its library list, then Escape returns to the actual
originating workspace. The production PTY test navigates from the manuscript
outline into a reference, closes both levels with Escape, verifies Outline is
restored, opens the selected scene and verifies its manuscript text. This
replaces the test's previous manual Navigate → Manuscript route and prevents
assuming the editor was the origin when it was not.

## Confirm revision-hunk restoration before applying — 27 November 2026

Revision history previously restored a highlighted hunk immediately when the
author pressed `r`. It now opens a focused confirmation naming the snapshot and
hunk; Enter or `y` applies only that hunk, while Escape or `n` cancels and
leaves the current diff/text unchanged. Confirmation re-reads the selected
snapshot diff and current document hash before writing. The existing restore
service still creates a safety snapshot of the pre-restore text. Unit coverage
verifies both cancellation and confirmation, and the production Linux PTY
journey performs cancel-then-confirm, exits and reopens to verify the restored
text and safety snapshot.

Focused unit tests and the production Linux PTY test pass. Verification on
27 November 2026: `cargo fmt --all -- --check`,
`cargo clippy --offline --workspace --all-targets -- -D warnings`, and
`cargo test --offline --workspace -- --test-threads=1` all passed. The test
run included 39 PTY tests; one protected private-corpus test was intentionally
ignored because its local fixture path was not configured. An initial test run
exposed a collision with the existing chapter-title input mode; revision
confirmation now uses a distinct mode, and the full suite passes after that
correction. Both selective and whole-document restoration now have explicit
confirmation and safety snapshots. Snapshot rename/retention policy and human
UAT remain outstanding.

The Revisions diff also exposes `R` to restore the entire document from the
selected snapshot. It previews the current and snapshot character counts,
requires Enter/`y` confirmation, and supports Escape/`n` cancellation. The
existing optimistic hash check and journalled restore service preserve the
current text in a safety snapshot. A unit regression covers preview,
cancellation, full restoration and safety snapshot creation. Follow-up
verification on 27 November 2026 passed workspace formatting, warnings-denied
workspace Clippy, and the full offline workspace test suite, including 39 PTY
tests (one protected private-corpus test was intentionally ignored because its
local fixture path was not configured). A separate production PTY test now
covers whole-document preview, cancellation, confirmation, clean exit/reopen,
restored prose and the pre-restore safety snapshot. Human UAT remains open.

## Scrivener inherited section-type profile suppression — 27 November 2026

Compile profiles previously applied a Scrivener layout's `Include Text=No`
only when a binder item had an explicit `SectionType`. Scrivener also assigns
section types by binder category through `LevelTypes`; the presentation
translator already honoured those defaults, but profile assembly did not.
Profile creation now uses the same effective section-type resolver, preserving
explicit per-item overrides while applying inherited per-format body
suppression. The synthetic import-to-profile test now omits per-item section
types and verifies the inherited mapping, source exclusion, retained heading
and private canonical body.

Verification passed on the current change: `cargo fmt --all -- --check`,
warnings-denied workspace Clippy, and the complete offline workspace suite
(including 358 terminal unit tests, 3 author workflows, and 38 PTY tests; one
protected-corpus PTY intentionally ignored). The Ollama loopback mock ran with
the author's prior socket permission. The exact-commit Book 03 production run
is `reports/level-types-inherited-872013c-20261127/`, with its imported
project under `generated/level-types-inherited-872013c-20261127/book-03-project/`.
It retained all 70/70 manuscript and 10/10 research bodies after reopen, left
the protected source unchanged, and the standard generated EPUB passed
EPUBCheck 4.2.6 with zero errors or warnings. The separately compiled
Scrivener `DEFAULT` profile is
`generated/level-types-inherited-872013c-20261127/book-03-project/exports/inherited-profile.epub`;
it also passes EPUBCheck with zero errors or warnings, but reports 14 missing
or unavailable layout assignments. Its content/heading presentation therefore
remains provisional and is not evidence of source-layout fidelity. The full
report records source/reference text discrepancies and 3 unresolved variables.
Fixture SHA-256 validation passed before and after. No human acceptance is
claimed.

## Manual whole-project backup workflow — 27 November 2026

The existing consistent project archive service is now exposed as File → Back
Up Project and through Search All Commands. The shared file browser starts in
the user's home folder with a sensible `.wide.zip` filename. Creating the
archive first saves pending manuscript text, then uses the existing SQLite
online-backup, project-tree copy, SHA-256 manifest, same-filesystem staging and
atomic publication path. Existing archive files are never
overwritten; failures leave the browser open with an actionable message.

A production PTY regression exercised the visible File menu, created the
archive, restored it into a separate directory, reopened and validated the
project, checked manuscript text and attachment bytes, and confirmed the
source attachment was unchanged. This covers the manual-backup foundation in
#70, not its entire contract. Rolling schedules, preference controls,
retention, backup destination policy, close/exit prompts, failure retry and
SSH/SIGHUP recovery remain outstanding. Human UAT remains required.

## Revision snapshot author workflow — 27 November 2026

The existing snapshot service is now covered through the production terminal
interface as well as service and UI-state tests. The PTY journey creates a
labelled scene snapshot from the command palette, edits and saves the prose,
opens the Revisions workspace, reads the changed text in its diff, restores the
selected hunk, and exits. Reopening verifies the original prose and confirms
that the restored pre-change text was itself snapshotted. The revision list now
shows the scene and author-provided label without placing a UUID and raw Unix
timestamp ahead of the useful description; this keeps labels legible at 80
columns.

Focused production PTY verification passed. The full offline workspace suite
passed, including 358 terminal unit tests, all three author workflows, 37
production PTY tests (one protected-corpus test intentionally ignored), and
all workspace library, CLI, integration, and documentation tests. The
Ollama-only localhost mock test passed with the previously authorised socket
permission. `cargo fmt --all -- --check` and workspace Clippy with warnings
denied also passed. This does not complete #69: restore still executes immediately
from the inspected diff without a separate confirmation step, and whole-document
restore, rename/retention policy, and human UAT remain outstanding. No
interaction-design proposals awaiting author approval were implemented.

## #38 expected external-edit conflict through the production PTY — 27 November 2026

A new PTY regression launches the real terminal with a saved scene, atomically
replaces that scene file from an external writer while WIDE is open, types a
separate in-memory edit, and waits for autosave. WIDE reports the conflict,
keeps the in-memory edit available, offers Quit Without Saving, exits cleanly
when that explicit choice is selected, and leaves the externally written file
byte-for-byte intact. On reopen, the unchanged metadata hash still causes
conflict detection; the test intentionally verifies the file bytes rather than
using a normal load that would suppress that integrity warning.

This demonstrates the expected concurrent-edit safety path; it does **not**
reproduce the author's earlier spontaneous conflict or explain the separately
reported disappearance of 11 outline scenes. Do not disable conflict checks or
close #38 on this evidence. Existing empty-project idle-startup PTY coverage
also confirms no conflict appears without a selected document. Human
reproduction and the missing-scenes root cause remain outstanding.

## Protected ArrowMaking Scrivener import through the TUI — 27 November 2026

The previously ignored production PTY test was explicitly run with
`WIDE_PRIVATE_ARROWMAKING_PROJECT=fixtures/back-catalogue/scrivener/ArrowMaking.scriv`
and Rust's `--ignored` selector. It passed through the file browser, normal
automatic import path, outline, direct prose edit/autosave, permanent Save,
reopen, profile compile and source-fingerprint check. The imported structure
and automatically proposed scene count remained available after reopening.
Fixture SHA-256 validation passed before and after the run. The private source
was not modified and remains excluded from Git. This is automated workflow
evidence, not author acceptance; import progress feedback and publication
fidelity gaps remain open.

## Scrivener format-level scene separator — 27 November 2026

Scrivener `.scrformat` files can define the separator between consecutive text
documents at format level (`SeparatorSettings/TextSeparators/Between`). WIDE
now imports this value as typed format metadata and applies it to the compile
presentation only when selected scene layouts provide no scene separator.
An explicit layout separator retains precedence; an incomplete or unsupported
setting produces a warning. This remains subject to the source-format mapping
limits recorded in `REAL_MANUSCRIPT_REGRESSION.md` and human publication UAT.

Synthetic regression coverage verifies the XML import, the format-level
fallback, the layout-level precedence and separator rendering between two
scenes within one chapter. The full offline workspace suite passed against the
implementation committed as
`4e8f471b91f068d5976a48f2eab984a71aba0099`: 358 terminal unit tests, three
author-workflow tests, 35 PTY tests, all workspace library, CLI, integration
and doc tests; one protected private-corpus PTY was intentionally ignored.
Workspace formatting and warnings-denied Clippy passed.

The exact-commit production three-book regression is
`reports/format-separator-4e8f471-20261127/`; generated EPUBs are
`generated/format-separator-4e8f471-20261127/book-01.epub`,
`generated/format-separator-4e8f471-20261127/book-02.epub`, and
`generated/format-separator-4e8f471-20261127/book-03.epub`. Fixture checksums
passed before and after. Source fingerprints remained unchanged, all 149 included
manuscript/research bodies (41+4, 38+4, 70+10) survived persistence and
reopening exactly, and excluded Trash bodies remained excluded. EPUBCheck
4.2.6 reported zero errors and warnings for all generated EPUBs. The run remains
provisional: all three imports are classified incomplete, and the recorded
source/reference publication discrepancies remain material. This regression
does not establish human publication fidelity.

## Scrivener statistics metadata and three-book regression — 27 November 2026

The Scrivener profile investigation corrected an earlier interpretation of
`ExcludeFrontMatter` and `ExcludeBackMatter`: in `.scrformat` these values are
inside `Statistics`, where they control word/statistics totals. They are not
publication inclusion flags. The importer now retains each optional boolean on
its typed per-format settings. A regression confirms that selected front and
back matter remains publishable even when both statistics exclusions are on.
Compile inclusion continues to come from binder flags and the selected
front/back matter folders in `compile.xml`.

Authoritative Scrivener 3 documentation also confirms that section layouts are
assigned to section types in the project compile overview. A format with no
assignments can still compile, but its section-layout presentation is not
applied. WIDE preserves explicit assignments and reports missing or
unavailable ones; it does not copy assignments from another format or infer
them from binder names. In the current Book 03 source, the selected custom
format has no section-layout assignments, while the separate `DEFAULT`
settings name built-in layouts that are not present in the project's custom
layout definitions. That is a source configuration gap, not evidence that the
custom layout names should be guessed. The resulting presentation remains
provisional pending source settings clarification and human review.

The production three-book run at commit
`798abe95a9403a0805174a7fe62475872d322ec4` is
`reports/scrivener-stat-layout-798abe9-20261127/`; EPUBs for local review are:

- `generated/scrivener-stat-layout-798abe9-20261127/book-01.epub`
- `generated/scrivener-stat-layout-798abe9-20261127/book-02.epub`
- `generated/scrivener-stat-layout-798abe9-20261127/book-03.epub`

All source fingerprints were unchanged. Exact import/reopen body preservation
was 41/41 manuscript and 4/4 research for Book 01; 38/38 manuscript and 4/4
research for Book 02; and 70/70 manuscript and 10/10 research for Book 03.
Trash body records were intentionally excluded (3, 4 and 0 respectively), no
body destinations were missing or changed, and EPUBCheck 4.2.6 reported zero
errors and warnings for all three generated EPUBs. Import status remains
incomplete for all three, and these metrics do not establish publication
fidelity: semantic comparison discrepancies remain and the reports remain
provisional. Fixture SHA-256 verification passed before and after the exact-
commit run.

The first full workspace run found a flaky production PTY authoring test: its
sentence was sent as one burst, and the terminal dropped the final characters
under load. The test now emits that sentence at a human typing cadence. The
formerly failing PTY passed in isolation and in the complete workspace run.
`cargo fmt --all -- --check`, workspace Clippy with warnings denied, and the
full offline workspace test suite passed; the terminal suite passed 358 unit
tests, 3 author-workflow tests and 35 production PTY tests, with the protected
ArrowMaking PTY intentionally ignored without its local fixture variable. The
loopback-only Ollama test passed with the author's previously authorised local
socket permission. This changes test input pacing only, not terminal behaviour.

## #38 project-copy completeness regression — 27 November 2026

The reported UAT symptom that Save As omitted `attachments/`, `templates/`, or
`exports/` was not reproduced in the current implementation. Save As uses the
project archive/restore path, which includes those managed trees and a
consistent SQLite backup. A new terminal application-state regression
populates attachments, a nested template, an existing export and research
content, selects Save As through the registered File command and browser, then
checks source and destination bytes, project identity, manuscript prose, and
the reopened database. A production Linux PTY test now exercises the same copy
through the visible File menu and Save As browser, then reopens and validates
both projects. The focused state and PTY tests pass. This is verification
coverage, not a claimed root-cause fix. The reported false external-change
conflict and missing scene rows remain unreproduced; preserve conflict
detection and leave #38 open for continued reproduction and human UAT.

Follow-up P0 startup check: a production Linux PTY now opens a genuinely empty
project, dismisses first-run help, waits beyond the two-second autosave window,
and verifies that no save-conflict/save-failed state appears and no unattached
editor is opened. The test also checks that Insert Date and Time is unavailable
without a document and exits normally. It passed. This rules out the reported
spurious conflict for that idle empty-project path only; it does not reproduce
the author's dirty-buffer conflict, establish its trigger, or explain the
missing scene rows. Conflict detection remains enabled and #38 stays open.

Verification for this batch: `cargo fmt --all -- --check`, workspace Clippy
with warnings denied, and the complete offline workspace test suite passed.
The terminal suite passed 358 unit tests, all 3 author-workflow tests, and 34
production PTY tests; the protected ArrowMaking PTY is intentionally ignored
unless its private fixture path is configured. The full suite includes the
localhost-only Ollama mock-server test run with the author's authorised socket
permission. A first full-suite attempt exposed that the tutorial PTY sent its
entire sample sentence as one artificial burst and intermittently lost the
tail; the harness now sends those characters at a human typing cadence, and
the test passed both in isolation and in the complete PTY run. This test-harness
adjustment does not change application input behaviour.

## Recent project author workflow — 27 November 2026

The command audit found that `File > Recent Projects` had service/state tests
but no production terminal journey. A new PTY test seeds an isolated recent
project list, opens a different project through the visible menu, confirms its
fiction outliner is active, opens a scene, and checks its saved prose and
identity after exit/reopen. This exposed that projects without a saved session
were landing on Dashboard. The application now selects the established fiction
Scene Cards or non-fiction Outline as the default on startup/project switch,
then restores any saved per-project workspace over that fallback. The
production Recent Projects PTY passes. Editor-to-outliner navigation now tracks
the latest Scene Cards or Outline presentation, including across first Save
promotion, so Esc returns to the view the author was using. PTY coverage
verifies scene/chapter renaming, Scrivener import through edit, autosave,
promotion, reopen and compile, and the fiction create/save/reopen/continue
journey. Final verification passed: `cargo fmt --all -- --check`, workspace
Clippy with warnings denied, and `cargo test --offline --workspace
-- --test-threads=1`. The run passed 358 terminal unit tests, 35 production PTY
tests (one private fixture PTY intentionally ignored), 3 full author workflows
and all other workspace tests. The Ollama test's local loopback socket ran with
the user's previously authorised permission. Human UAT remains necessary.

## Scrivener `LevelTypes` profile mapping — 27 November 2026

The importer now retains Scrivener binder-category section defaults from
`.scrivx` `LevelTypes`. The shared compile-profile mapping gives an explicit
document `SectionType` precedence, then resolves the `Folders`, `Containers`,
or `Files` default from the binder type. Per-format layout references resolve
by exact source layout ID or one unique exact layout name. Ambiguous names are
reported; missing definitions are not guessed from document titles.

Focused importer and app tests pass. Formatting, workspace Clippy with warnings
denied, and the full offline workspace test suite pass (one protected private
ArrowMaking PTY is intentionally ignored unless its local path is configured).
The exact-commit Book 03 production regression is
`reports/level-types-book03-ab42c04-20261127/`; its EPUB is
`generated/level-types-book03-ab42c04-20261127/book-03.epub`. Commit
`ab42c04760397d7be109125e5eab9481e6d02a5a` preserved 70/70 manuscript and
10/10 research bodies through import/reopen, did not modify the source, and
passed EPUBCheck 4.2.6 with zero errors/warnings. Fixture checksums passed
before and after. The regression harness marks the worktree dirty because the
author's untracked `README_TMP.md` and `UAT/` are present; `git status` showed no
modified tracked files. The earlier working-tree run
`reports/level-types-book03-20261127-02/` remains local and provisional.
Profile fidelity remains provisional: the default profile has three selected
types with no layout assignment and eight assignments whose built-in layout
definitions are absent; the alternate profile has thirteen unassigned selected
types. Do not invent mappings. Issues #7/#53 remain open for further profile
recovery and human publication acceptance.

The latest author-proposed interaction model remains under review and is not
approved. Do not undertake interface restructuring from that proposal.

## Dependency, licence and coverage-tool audit — 27 November 2026

The independent Rust tooling is available on this machine: cargo-audit 0.23.0,
cargo-deny 0.22.1, cargo-machete 0.10.1, and cargo-llvm-cov 0.9.2. `cargo
machete --with-metadata --skip-target-dir` passed and found no unused
dependencies. The feature hardening and audit refresh are committed and pushed
as `7ad9ab81244d3a2f1272539b28f559919fce2f52`.

`cargo audit` ran against a locally fetched RustSec database (1,568 advisories)
and reported 24 advisory instances across 645 locked package records after
WIDE's direct `zip` 0.6.6 dependency was restricted to the Deflate feature. This
removes unnecessary AES, bzip2, zstd and time codec support from that path and
eliminates its zstd decoder warnings without changing package versions. Audit
findings still cover ZIP versions 0.6.6, 7.4.0 and 8.6.0, bundled SQLite
(`libsqlite3-sys` 0.35.0), and URL 2.5.8. They require reachability and
compatibility review; the advisory count is not a count of confirmed exploitable
WIDE paths. WIDE's ZIP 0.6.6 call sites read entries rather than invoking ZIP
extraction. The bundled SQLite advisory is relevant when opening an untrusted
project database and sibling WAL. The package upgrade needed to include patched
SQLite was not made because the existing project constraint forbids automatic
dependency upgrades. `bincode` and `paste` are reported as unmaintained, not as
vulnerabilities.

`cargo deny` still has no repository policy file; its default license policy is
not a meaningful compatibility verdict, and locked platform-specific package
metadata has previously been unavailable in this restricted registry. No
license allowlist was invented.

The workspace Cargo manifests declare `MIT`, but the repository has no tracked
root `LICENSE` or `COPYING` file. This declaration is an unresolved release
documentation/licensing consistency issue; do not present WIDE as MIT-licensed
or make an open-source claim until the author resolves it. No dependency was
package version was upgraded and no runtime dependency was added. `cargo
machete --with-metadata --skip-target-dir` passes. cargo-llvm-cov is installed;
coverage was not measured in this checkpoint. The first workspace test attempt
could not link because the workspace volume was full. Only the rebuildable
`target/debug/incremental` cache was removed; the retry with incremental
compilation disabled passed. `cargo fmt --all -- --check` and
`cargo clippy --offline --workspace --all-targets -- -D warnings` passed. The
full `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --offline --workspace
-- --test-threads=1` run passed all 753 tests across the workspace, including 356
terminal unit tests, 3 author workflows and 30 PTY tests; one protected private
ArrowMaking PTY remains intentionally ignored without its private fixture
environment variable. All doc tests passed. No runtime/network service was
needed; the test requiring a temporary loopback listener ran with the separately
authorised local-socket permission.

## Scrivener profile warning scope — 26 November 2026

Scrivener profile warnings about missing section-layout assignments now apply
only to documents actually selected for the publication: mapped manuscript
documents included by the source, plus front/back matter explicitly selected
by that format. Private research, compile-excluded documents, and alternate
matter not selected for the format no longer produce irrelevant layout
warnings. The rule uses source destination and compile settings, not binder
names. Selected documents without source layout assignments remain warnings;
WIDE does not infer how they should print.

Regression: `scrivener_layout_rules_translate_using_import_roles_and_report_gaps`
covers included manuscript layout gaps while also ensuring private research
and compile-excluded content do not add warnings. The existing
`scrivener_selected_matter_is_imported_as_editable_private_by_default_content`
test verifies that format-selected back matter still receives its configured
heading behavior and included body. All 64 `story-app` tests pass. The full
offline workspace suite passes with the authorised loopback socket permission:
356 terminal unit tests, 3 author-workflow tests, and 30 PTY tests passed; the
protected ArrowMaking PTY is the single intentional ignore without its local
fixture environment variable. Workspace Clippy with warnings denied and
`cargo fmt --all -- --check` pass.

A fresh Book 03 import/save/reopen/compile run is
`reports/scrivener-layout-warning-20261126/`; its EPUB is
`generated/scrivener-layout-warning-20261126/book-03.epub`. All 80 body-bearing
source records reconcile (70 manuscript, 10 research), with no missing or
changed bodies; fixture checksums pass before and after; EPUBCheck 4.2.6 reports
zero errors/warnings. The imported profile now presents three unresolved
layout warnings tied to selected source entries, down from unrelated warnings
across private/unselected entries. This remains provisional: profile fidelity
and exact intended rendering of those source section types are unresolved, and
human publication review is still required.

The exact-commit compile-only validation is
`reports/scrivener-layout-warning-a599ba3-20261126/`; the EPUB for local
Calibre review is
`generated/scrivener-layout-warning-a599ba3-20261126/book-03.epub`. It was
compiled at commit `a599ba355abe7e2136065442ec58f9a24ad91270`. Its comparison is
still provisional and requires investigation of source/reference differences
and human review.

## Book 03 regression-harness classification correction — 26 November 2026

The earlier 68/68 manuscript and 10/12 research-body counts were a harness
classification error, not two missing imported source bodies. Two source
documents have both a canonical manuscript destination and a secondary
reading-index record; the verifier chose the reading record first and compared
it with the complete source body. The verifier now gives manuscript
destinations (`scene` or `content_unit`) precedence and treats the reading
index as a research destination only when there is no manuscript destination.
The production project itself was not changed.

Synthetic coverage now exercises unknown-area front matter, research
destinations, unmapped bodies and mixed manuscript/reading destinations. The
Python regression tests passed: 23 tests. The corrected offline compile-only
report is `reports/scrivener-profile-fidelity-rerun-20261126/`; its EPUB is
`generated/scrivener-profile-fidelity-rerun-20261126/book-03.epub`. Against the
persisted Book 03 project from the same-day fresh import, all 80 body-bearing
source items reconcile: 70/70 manuscript bodies and 10/10 research bodies;
zero missing or changed bodies, zero Trash bodies. The source checksum
manifest passed before and after. EPUBCheck 4.2.6 reports zero errors and
warnings. Overall publication acceptance remains provisional: this was a
targeted compile-only run, the import still reports unresolved Scrivener
layout assignments, and human publication review remains outstanding.

The expanded private corpus remains excluded from Git. Do not attach or publish
the local content-bearing reports or generated EPUB.

## P0 #62 — empty previous-position command safety verified, 26 November 2026

The production terminal already handles empty location history as a non-fatal
status result. It preserves the editor buffer, cursor and dirty state; the
command dispatcher keeps the application running. The terminal unit test
`previous_position_without_history_is_a_nonfatal_menu_action_and_preserves_unsaved_text`
and the production Linux PTY
`empty_previous_position_from_navigate_menu_keeps_the_terminal_running`
both passed in this checkpoint. The PTY selects the command through Navigate,
checks the visible nonfatal message, reopens the menu and exits cleanly.

This verifies the reported empty-history failure automatically; it does not
establish human UAT. The issue's populated-history/boundary and ordinary
terminal acceptance criteria remain open for the author's retest. Continue to
the next independent P0 defect rather than waiting for that acceptance.

## UX interaction design proposal — awaiting author review, 26 November 2026

`docs/WIDE_UX_INTERACTION_DESIGN_v1.0_PROPOSAL.md` is a proposed, unapproved
interaction design for the author's review. It incorporates the manuscript-
centred direction supplied on 26 November: a hierarchical document editor
with integrated novel-development tools; common author-facing document nodes;
GrandView-inspired structural editing; Scrivener-informed document
organisation; WordPerfect-style semantic editing/Reveal Codes; Emacs-style
flexible panes; compact menus; and a shared focus/selection/identity model to
investigate. It offers alternatives, trade-offs, terminal mock-ups and explicit
decisions for review. No interface redesign or behavior change is authorized
by that draft, and the implementation remains unchanged pending approval.

GitHub's current open issue list and latest comments were refreshed through
`gh` on 26 November. #76 remains the P0 production terminal workflow/release
gate, and its human acceptance is still pending. Issues #7/#53 retain
actionable Scrivener profile fidelity work. Issue #75 is explicitly post-
freeze/vNext and is excluded from the v1.0 goal. Other open issues remain in
the persistent backlog and should be assessed against their latest comments.

The immediately actionable continuation is Scrivener publication-profile
mapping, without changing the terminal interaction model. Latest GitHub
comments report that the research-body fidelity improved to 12/12 in a fresh
Book 03 run, while two source section types still lack profile layout mappings;
the earlier 10/12 figure in the checkpoint below is superseded. Confirm this
from the local regression artifacts before relying on it. Human publication
acceptance remains open.

## Current checkpoint — Scrivener compile profile mapping, 26 November 2026

The current follow-up adds translation for a simple Scrivener chapter title
layout prefix containing one automatic chapter counter (for example,
`Chapter <$t>`). It becomes WIDE's shared `Chapter {number}` presentation rule;
the canonical binder title, scene names and prose remain unchanged. A
synthetic production import/profile test verifies sequential chapter headings,
scene separators, source preservation and no visible source variable. The same
batch fixes a relative-output-path bug in the profile compile service, with a
regression asserting that output is written beneath the canonical project
root exactly once.

Verification on this working tree:

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — passed.
- `cargo test --offline --workspace -- --test-threads=1` — passed: 356
  terminal unit tests, 3 end-to-end author workflows, 30 production PTYs,
  remaining workspace/integration/doc tests; one private-corpus PTY is
  intentionally ignored unless its protected fixture path is configured.
- Focused relative-project profile output test — passed.
- Fresh Book 03 regression `scrivener-numbered-layout-20261126` — fixture
  checksums passed before and after; 68/68 manuscript bodies and 10/12
  research bodies matched the current source on import/reopen; 0 Trash bodies
  were imported. The import status remains **incomplete** because two research
  bodies are not exact.
- A real imported `DEFAULT` profile EPUB was generated at
  `generated/scrivener-numbered-layout-20261126/book-03-project/exports/book-03-default-layout.epub`.
  EPUBCheck 4.2.6 reported zero errors and warnings, and the EPUB contains no
  literal unresolved Scrivener placeholders. Its fidelity remains provisional:
  seven source section-layout assignments remain unmapped and are reported to
  the author. They have not been guessed from binder names. The local details
  are in the ignored run directory `reports/scrivener-numbered-layout-20261126/`.

Human publication acceptance remains outstanding. Continue investigating
generic Scrivener per-format layout assignment recovery and the research-body
discrepancies, then return to the remaining v1.0 author-workflow and publication
backlog. This checkpoint does not supersede the Alpha UAT 36 build record or
claim that the broader #76 end-to-end journey audit has passed human review.

## Current checkpoint — Alpha UAT 36, issue #76 workflow audit, 26 November 2026

The focused command/workflow repair batch is frozen at commit
`328109b11be804d87b05ac383e8571a2aa6cc85f`. The isolated Linux x86_64 UAT
bundle is `test-builds/alpha-uat-36-328109b/`; launch it from the repository
with:

```sh
cd test-builds/alpha-uat-36-328109b
./run.sh "Authoring Audit Project"
```

`UAT-CHECKLIST.md` contains the acceptance steps. The bundle includes two
synthetic sample projects and sanitised synthetic Scrivener fixtures, with
preferences and HOME isolated inside the bundle. It does not include private
manuscript regression fixtures. `SHA256SUMS` was generated and verified after
packaging; SQLite integrity checks passed for all bundled sample projects.
The binary SHA-256 values and frozen source revision are in `BUILD-MANIFEST.md`.

The production Linux PTY authoring-audit workflow passed against the release
terminal executable. Formatting, workspace Clippy with warnings denied, and
the complete offline workspace suite passed at the frozen commit. The audit
covers section/chapter/scene creation, inclusion feedback, manuscript editing
and save, return to the originating outline by identity, searchable story-link
selection, and editable book type. Remaining registered commands are inventoried
in `docs/COMMAND_WORKFLOW_AUDIT.md`, with their individual verification status
made explicit. Human UAT is pending; no human-acceptance issue is closed.

Known limitation: Scrivener import is synchronous and may appear idle during a
long import. This remains open for #53/#76. Next implementation task is to
resume Scrivener per-format layout/inclusion integration with the shared compile
presentation controls, then continue the remaining actionable v1.0 backlog.

## Current checkpoint — issue #76 command/workflow audit, 26 November 2026

The current working batch audits the Alpha UAT 35 author-facing command
failures and preserves the in-progress Scrivener format-profile integration.
`docs/COMMAND_WORKFLOW_AUDIT.md` inventories 384 current or historically
advertised command entries, their expected effect, context, entry point and
verification status. Seven core actions are directly exercised by the new
production PTY workflow; the remaining registered commands are explicitly
marked as not individually exercised by this focused audit. Existing
issue-specific tests continue to cover other workflows.

The focused audit PTY creates a section, chapter and scenes through menus,
checks included/excluded feedback, edits and saves manuscript text, creates a
story link by selecting existing entities, and changes the book type through
File → Book Type. Regression coverage also verifies stable direct-prose
breadcrumbs and Escape navigation, persisted book-type changes without
restructuring, and menu-driven chapter prompts. The previously hard-coded
Outline return has been corrected to return to the originating outline view
(including Scene Cards) while restoring the selected document by identity.
Date/time insertion without an active manuscript identity is disabled with a
plain-language reason.

Automated verification on the current candidate working tree:

- Formatting and workspace Clippy with warnings denied — passed.
- Focused authoring PTY, direct-prose identity test, and three previously
  mismatching production PTY journeys — passed after updating their assertions
  to match the author-facing prompts and inclusion indicator.
- `cargo test --offline --workspace` — passed: 356 terminal unit tests, 3
  author workflow tests, 30 active production PTYs, all other unit/integration
  tests and doc tests. One protected-corpus PTY is ignored unless its private
  fixture path is configured. The run used the user's previously authorised
  loopback access for the local Ollama test.
- The earlier full run found four stale PTY expectations; those assertions were
  updated to recognise the explicit chapter prompt and the separate inclusion
  indicator. The rerun above passed.
- Synchronous Scrivener import still does not expose incremental progress; the
  import eventually completes in the tested fixture, but long imports may
  appear idle. This remains an open #53/#76 limitation.

The implementation is ready for an isolated Alpha UAT 36 build from its
verified commit. The release will exclude private manuscripts and use bundle-
local preferences and disposable synthetic projects. After recording and
packaging that commit, resume the persistent v1.0 backlog with the Scrivener
per-format layout/inclusion work already in progress. Human UAT remains
pending.

## Current checkpoint — Alpha UAT 35 and lifecycle integrity, 26 November 2026

Frozen Linux x86_64 release build from commit `b55c981b88d5702c769178964fa7506219ba0738` is available at `test-builds/alpha-uat-35-b55c981/`. Launch with:

```sh
cd /home/rick/Development/Active/WritersIDE/test-builds/alpha-uat-35-b55c981
./run.sh
```

The bundle has isolated preferences/HOME, disposable fiction and non-fiction
projects, and sanitised synthetic Scrivener fixtures. Its acceptance checklist
is `UAT-CHECKLIST.md`; SHA-256 entries are in `SHA256SUMS` and were verified
after packaging and after release-binary PTY runs. Release-binary PTYs passed
the fiction and non-fiction project-creation workflows. No private manuscript
corpus is included.

Verification on commit `b55c981`:

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — passed.
- `cargo test --offline --workspace -- --test-threads=1` — passed with
  loopback permission; includes 352 terminal unit tests, 3 author workflows,
  29 production PTYs (one protected-corpus PTY intentionally ignored unless
  its private fixture path is configured), all crate/integration tests and
  doc tests.
- Frozen release executable PTY smoke: fiction and non-fiction project
  creation/write/save paths passed. Bundle manifest and binary hashes are
  recorded in `test-builds/alpha-uat-35-b55c981/`.
- The protected ArrowMaking import PTY remains separately verified on the
  prior code commit `2dff9e0`; its fixture checksums passed before and after.
- Human acceptance of the Alpha UAT 30 lifecycle findings remains pending.

The latest #38 investigation found an edge case where a dirty buffer without a
document identity could be treated as a successful no-op Save. Save now fails
explicitly while preserving the buffer, and a rejected paste does not schedule
autosave. This does not reproduce the reported false external-hash conflict or
confirm why eleven scene rows disappeared. Hash-based conflict detection is
unchanged; those findings remain open for reproduction and human UAT.

The previous UAT 33/34 entries below are retained as historical build records.

Frozen Linux x86_64 release build from commit `f5bad1b93f878db4b409ae1e4fb3ae8920b47a0e` is available at `test-builds/alpha-uat-33-f5bad1b/`. Launch it with:

```sh
cd /home/rick/Development/Active/WritersIDE/test-builds/alpha-uat-33-f5bad1b
./run.sh
```

The bundle has isolated preferences/HOME data, disposable fiction and
non-fiction projects, and sanitised synthetic Scrivener fixtures. Its UAT
checklist is `UAT-CHECKLIST.md`; SHA-256 checksums are in `SHA256SUMS` and were
verified after packaging and frozen-binary PTY checks. The release executable
passed production PTY workflows for creating/writing/saving a fiction project
and creating/writing/autosaving/promoting a non-fiction chapter-only project.
The isolated bundle contains no private manuscript corpus.

Verification on commit `f5bad1b`:

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — passed.
- `cargo test --offline --workspace` — passed, including 351 terminal unit
  tests, 3 author-workflow tests, 29 production PTYs and all other workspace
  and documentation tests; one protected-corpus PTY is intentionally ignored
  unless its private fixture variable is configured.
- Focused PTYs for fiction, non-fiction, staged Scrivener import/edit/reopen/
  compile, and abrupt-exit recovery — passed.
- Protected ArrowMaking import PTY and fixture checksums — passed in the
  preceding checkpoint; original source remained unchanged.
- Alpha UAT 33 bundle checksums and frozen release executable workflows — passed.

Human acceptance is pending. The reported false external-change conflict and
eleven disappearing outline scenes from #38 still have not reproduced, and
their root cause is not established. Hash-based conflict detection remains
enabled. Issues #37, #38, #44, #46, #52, #53, #54 and #55 remain open for
human verification and any further author findings. Next, continue the
Scrivener per-format layout and inclusion integration, then the outstanding
approved v1.0 backlog.

Updated: 2026-11-26

## Current checkpoint — Alpha UAT 32 and direct Scrivener import, 26 November 2026

Frozen Linux x86_64 release build from commit `1c41b9e` is available at
`test-builds/alpha-uat-32-1c41b9e/`. Launch it with:

```sh
cd /home/rick/Development/Active/WritersIDE/test-builds/alpha-uat-32-1c41b9e
./run.sh
```

The build has isolated preferences and HOME data, synthetic fiction and
non-fiction samples, and a sanitised Scrivener fixture. Its acceptance
checklist is `UAT-CHECKLIST.md`; the frozen files verify against `SHA256SUMS`.
The launcher rendered a recognisable WIDE screen in an 80x24 PTY. The bundle
does not contain private manuscript fixtures.

In the normal Scrivener import browser, pressing Enter on a selected `.scriv`
project now starts automatic import to staging immediately and opens the
normal outline/editor workspace; no intermediate mapping confirmation is
required. F5 remains available. The production PTY test exercises this path,
then edits, saves, compiles and reopens the imported project. The isolated
protected ArrowMaking corpus PTY also passed, with fixture checksum manifests
verified before and after.

Runtime build commit `1c41b9e`; the expanded regression suite was run on the
current working tree (test-only changes after the frozen build):

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — passed.
- `cargo test --offline --workspace` — passed: 351 terminal unit tests, 3
  author-workflow tests, 29 active production PTY tests, and all other
  workspace and doc tests; one PTY is intentionally ignored without private
  corpus input.
- Protected ArrowMaking import via production PTY — passed; source checksums
  remained valid.
- Alpha UAT 32 checksums and 80x24 launcher rendering — passed.

Human acceptance remains pending. A stronger Scrivener PTY regression now
edits and autosaves a chapter-body document while the import is still
temporary, navigates to a scene, promotes the project, reopens both documents,
and compares the persistent scene count with the automatic import plan. The
synthetic and protected ArrowMaking workflows passed; private source checksums
were valid before and after. The reported false external-change conflict and
missing eleven scene rows from #38 still have not reproduced, and their root
cause is not established. This exercises a representative body row, not the
reported Introduction-specific interaction. Conflict detection remains
enabled. Issues #37, #38, #44, #46, #52, #53, #54 and #55 remain open for
further reproduction and human checks.
Next actionable development is the already-planned integration of Scrivener
per-format layouts and inclusion into persistent WIDE compile profiles, then
the remaining approved v1.0 backlog.

### #38 persistence reproduction follow-up — 26 November 2026

## Current checkpoint — Alpha UAT 30 and imported Scrivener profile fidelity, 26 November 2026

### Alpha UAT 30

Frozen Linux x86_64 release build from commit
`db7fec355569d6a76cc96057f820ba3f71fdf3a4` is available at
`test-builds/alpha-uat-30-db7fec3/`. Launch with
`cd test-builds/alpha-uat-30-db7fec3 && ./run.sh`. Its HOME, preferences,
projects and synthetic Scrivener fixtures are isolated in the bundle. The
checklist `test-builds/alpha-uat-30-db7fec3/UAT-CHECKLIST.md` focuses on staged
project creation/import, editing and compiling before Save, first Save/Save As,
unsaved close choices, crash recovery and Scrivener Trash/Project Notes/folder
classification. `SHA256SUMS` was generated and verified. No private manuscript
fixtures or generated publications are in the bundle. Human UAT is pending.
The isolated release executable was launched and exited through an 80×24 PTY
smoke check. The disposable projects were restored to their pristine template
state afterwards, and the bundle checksums were regenerated and verified.

### Imported Scrivener profile implementation in progress

The shared Compile IR now applies source `IncludeInCompile=No` to ordinary
manuscript entries in imported format assemblies. Scrivener layout
`Include Text=No` now suppresses only the corresponding Compile IR body while
preserving the configured title and the canonical editable text. The policy is
saved in the imported profile's `CompilePresentation`, keyed by stable assembly
unit IDs. Older serialized presentations default to no suppressed bodies.
Implementation details are in [Engineering Design v2.45
addendum](engineering-design-v2.45-addendum.md).

The new synthetic import-to-profile regression passes, including profile
artifact generation and confirmation that the ordinary project still contains
the editable source prose. `cargo fmt --all -- --check`, `git diff --check`,
workspace Clippy with `-D warnings`, and the full offline workspace suite
passed: 346 terminal unit tests, 3 author-workflow tests, 28 Linux PTYs (one
private-corpus PTY ignored), all other unit/integration tests, and doc tests.
The full run required the previously authorised loopback permission for the
local Ollama test.

A fresh production import/reopen/EPUB run of controlled book-03 is recorded
locally as `reports/profile-binding-20261126/`; source checksums were verified
before and after, 68/68 manuscript bodies and 12/12 research bodies matched
after import, and the generated EPUB passed EPUBCheck 4.2.6 with zero errors
and warnings. This run remains provisional. Compiling both imported profiles
exposed unresolved layout mappings for two Scrivener section types; the output
still includes internal Manuscript and Title Page headings. The report and
generated outputs contain private manuscript material and remain ignored and
local. This is concrete remaining #7/#53 work, not a fidelity pass.

## Current checkpoint — persistent working projects and deferred Save As, 26 November 2026

Design details: [Engineering Design v2.44 addendum](engineering-design-v2.44-addendum.md).


New projects and standalone Scrivener imports now open from WIDE-managed,
persistent staging storage. Authors can edit, autosave, navigate and compile
before choosing a permanent folder. The first explicit Save or Save As opens
the existing destination browser; it creates a validated independent copy,
then switches the open workspace to that copy. The original staged copy stays
available if saving fails. Linux uses `$XDG_DATA_HOME/writers-ide/staging`
(or `~/.local/share/writers-ide/staging`); macOS and Windows use their normal
per-user application-data locations. Staged directories are private on Unix.

Startup resumes staged projects after an abrupt exit. Save As writes a durable
promotion intent before copying; on restart WIDE verifies project identity
before cleaning up a duplicate staged copy. A missing destination clears the
intent and keeps the stage. An identity mismatch preserves both projects and
reports the conflict. New Project and import cancellation do not create a
permanent destination. Import remains separate from publication configuration.

The Scrivener automatic mapping also now propagates Trash exclusion to all
descendants, retains Project Notes roots as private research rather than
promoting them to manuscript sections, and does not flag normal empty
organizational folders as missing prose. The import PTY workflow exercises
source selection, default mapping, immediate editing, first Save As, reopen
and compilation. A separate PTY test simulates SIGKILL and verifies staged
work reopens; synthetic tests cover promotion recovery and identity conflicts.
The private real-project PTY command is:
`WIDE_PRIVATE_ARROWMAKING_PROJECT="$PWD/fixtures/back-catalogue/scrivener/ArrowMaking.scriv" cargo test --offline -p writers-ide-terminal --test pty_smoke protected_arrowmaking_project_imports_through_the_production_pty_without_source_changes -- --ignored --nocapture`.
Automated verification passed on the final working tree:

```text
cargo fmt --all -- --check
git diff --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline --workspace
```

The workspace run passed (346 terminal unit tests,
3 author-workflow integration tests, 28 Linux PTY tests passed; 1 private-corpus
PTY is separately run and 1 PTY is intentionally ignored without its private
fixture). The protected ArrowMaking import/edit/compile PTY also passed with its
source fingerprint unchanged. Fixture checksum verification passed before and
after.

The same batch includes #61's locale-aware date/time insertion controls and
their menu listing. These are automated results only; #44, #46, #52 and #53
remain open for human acceptance.

## Current checkpoint — Scrivener layout selection in Publish, 26 November 2026

Imported Scrivener compile layouts are now selectable from the curated
Publish menu through **Compile with Imported Layout…**. The author chooses a
readable layout name, then uses the existing Save As browser to choose the
WIDE output format and destination. The selected profile is passed through the
shared Compile IR pipeline and the resulting artifact is written to the
chosen location. The status message confirms the layout and output path. The
terminal no longer requires a CLI command or a binder UUID for this workflow.

Regression coverage verifies layout selection, visible chooser controls,
format selection, output-file selection and generated manuscript content.
The real Scrivener import PTY test now continues through this Publish workflow.
It passes with the protected `ArrowMaking.scriv` source; a synthetic Scrivener
project also passes import → layout selection → DOCX compile in the production
terminal. Workspace format, Clippy (`-D warnings`) and tests passed on the
current working tree: 341 terminal unit tests, 3 author-workflow integration
tests, 27 PTY tests (1 ignored), plus the other workspace and documentation
tests. The run also verified private fixture checksums before and after.

This is implementation evidence, not publication acceptance. The expanded
corpus still needs per-format inspection to confirm that all Scrivener layout
constructs have an equivalent WIDE presentation, and the author must verify
that the Publish workflow is discoverable and produces acceptable output.
Issues #7, #53, #68 and #73 remain open for further fidelity work and human
UAT. Alpha UAT 29 is frozen at commit `79739f05faa2bd2a2a3dbc404b689ab703081488`
in `test-builds/alpha-uat-29-79739f0/`. Launch it with
`test-builds/alpha-uat-29-79739f0/run.sh`; isolated preferences and disposable
fiction, non-fiction and synthetic Scrivener fixtures are bundled. SHA-256
checks are in that directory's `SHA256SUMS`; bundle checksum verification and
both sample project validations passed. The bundle contains no private
regression corpus content.

### Scrivener profile follow-up — 26 November 2026

Fresh book-03 regression at `reports/alpha34-profile-test-20261126/` confirms
that the normal compile path preserves all 68 manuscript bodies and produces
an EPUB passing EPUBCheck 4.2.6 with zero errors and warnings. The imported
project offers two saved Scrivener profiles; compiling the source's selected
EPUB profile applies its selected matter and document inclusion settings.
The profile still reports two source section types without layout assignments,
and the independent extraction report identifies two research bodies without
destinations. Those remain unresolved; the successful compile is not a fidelity
pass.

That real-profile exercise also exposed a CLI failure when a project is opened
using a relative path: its project-relative output was joined to the relative
input path and rejected by absolute-path safety validation. The publisher now
joins project-relative output against its already canonical absolute project
root. A regression test compiles a Scrivener-style profile from a relative
project path and verifies both the generated Markdown and its absolute output
path. The profile's missing section mappings remain an open publication task.

Verification for the path fix: targeted story-app regression passed; formatting
and workspace Clippy with warnings denied passed. The first parallel workspace
run hit a transient `ETXTBSY` while starting a temporary Pandoc mock; that test
passed in isolation, and the complete workspace suite passed on a serial rerun
(`--test-threads=1`), including 351 terminal unit tests, 3 author-workflow
tests, and 29 production PTYs (one protected-corpus PTY ignored by default).

## Current checkpoint — selected Scrivener matter import safety, 26 November 2026

Selected Scrivener front- and back-matter documents are now classified as
typed WIDE content units instead of falling through the `unknown` binder area
to Research. They remain editable in the project and are excluded from the
ordinary compile by default, preventing alternative edition matter from
leaking into a publication. The terminal import-mapping chooser exposes both
destination types. When multiple Scrivener formats assign conflicting front
or back roles to the same source document, the import plan leaves that item
unmapped for an explicit author decision.

Synthetic importer and application tests cover typed mapping, conflicting
roles, persisted prose and default compile exclusion. Verification passed:
`cargo fmt --all -- --check`, workspace Clippy with warnings denied, and the
full offline workspace suite with loopback permission for the existing local
Ollama test (339 terminal unit tests, 3 author-workflow tests and 26 PTY
tests, plus all other crate, integration and doc tests).

The fresh real-manuscript run is
`reports/scrivener-matter-compile-20261126/`; generated projects and EPUBs are
under `generated/scrivener-matter-compile-20261126/`. All current source
fingerprints were unchanged, manuscript bodies survived exactly (39/39,
36/36, 68/68), research bodies survived exactly (6/6, 6/6, 12/12), and all
three generated EPUBs passed EPUBCheck 4.2.6 with zero errors and warnings.
The run remains provisional and all three imports report incomplete; the
historical comparisons remain discrepant. Fixture checksums passed before and
after the run.

This does not complete the next publication task. Per-format layout proposals
are inspectable, but source-format inclusion overrides and selected matter are
not yet bound to named WIDE edition assemblies/configurations. The imported
matter is intentionally excluded until an author chooses it. Do not claim
publication fidelity or close #7, #53, #68 or #73; their latest comments still
require full source-format-to-edition integration and human publication UAT.
The next implementation step is to connect each reviewed Scrivener source
format to a persistent, user-selectable WIDE assembly/profile, applying its
per-document inclusion and matter ordering in the shared Compile IR without
changing canonical project inclusion or prose.

The GitHub issue register currently has 72 open v1.0 issues; #75 remains
outside the feature freeze. Latest issue bodies and the latest available
comments were refreshed on 26 November. Continue with the P0/data-integrity
and author-workflow priority order after the Scrivener profile integration.

## Current checkpoint — terminal offline thesaurus workflow, 26 November 2026

The offline MyThes foundation is now reachable as Review → Thesaurus. The
terminal prompts for a word, selects a locally installed dataset based on the
project language (or `WIDE_THESAURUS_DATA`), and displays read-only results in
the integrated Reading workspace. British English looks for `en_GB`, then
Australian and US English with a visible fallback notice. The implementation
does not call a network service or change manuscript content. A Linux PTY
regression exercises the menu, prompt, local lookup results and clean exit;
targeted terminal unit coverage verifies menu registration and locale selection.

This is still partial #33 work: dictionary definitions, broader locale filename
discovery and full author acceptance criteria remain. `cargo fmt --all -- --check`,
workspace Clippy with warnings denied, and the complete offline
workspace suite passed for this checkpoint. The suite included 339 terminal
unit tests, 3 author-workflow tests and 26 Linux PTY tests. The thesaurus only
supports locally available MyThes-compatible synonym data; human menu and
authoring acceptance remains outstanding.

## Current checkpoint — offline MyThes lookup foundation, 26 November 2026

Issue #33 now has an optional offline MyThes-compatible data reader in
`story-analysis` and a developer CLI lookup command:
`writers-ide thesaurus <local-MyThes-data.dat> <word>`. It parses senses,
parts of speech and synonyms, validates record counts where the source format
provides them, and decodes the encoding declared in the data file. No runtime
service or network call is used. The command was exercised against the
installed English US UTF-8 and Australian ISO-8859-1 datasets. Synthetic tests
cover lookup, multiple senses, case-insensitive terms, legacy encoding and
malformed files.

The author-facing lookup is documented in the newer checkpoint above. There is
no bundled dictionary data or dictionary-definition source; continue #33 after
this foundation and retain en-GB as the project language default.

## Current checkpoint — explicit Scrivener layout translation, 26 November 2026

Added an application-level translation from one selected Scrivener compile
format to WIDE's existing compile heading/separator controls. Translation uses
the import plan's explicit Section/Chapter/Scene assignments and project-local
section-type and layout IDs; it never classifies documents from their titles.
Unmapped layouts, conflicting heading policies, unsupported separator kinds,
layout title/body prefixes or suffixes, body-text suppression, and Scrivener
front/back-matter defaults are surfaced as warnings rather than silently
discarded or guessed. A synthetic application test verifies chapter and scene
heading suppression, a custom scene separator, unchanged source titles, and
warnings for an unmapped section type.

The translation is now exposed in `inspect-scrivener`, the import-plan preview
and the final Scrivener import report, so authors and testers can inspect a
proposal for every source format. This is still not completion of Scrivener
profile integration: proposals are not persisted as named WIDE edition
recipes, format-level body inclusion and front/back matter are not yet applied
to the shared Compile IR, and the terminal does not yet offer a profile
creation workflow. Issues #7, #53, #68 and #73 remain open.

Verification after the CLI exposure: `cargo fmt --all -- --check`, workspace
Clippy with warnings denied, and `cargo test --offline --workspace --
--test-threads=1` passed. The full run included 338 terminal unit tests, 3
author-workflow integration tests, 25 Linux PTY tests, all crate and CLI tests,
and all doc tests. The current source tree also contains unrelated user-owned
changes in `.gitignore`, `README_TMP.md`, `UAT/` and `tools/__pycache__`; do not
stage those paths.

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

Implementation commit: `365c098` (pushed); the accompanying regression
documentation is `e3b1ca8` (pushed). Current repository HEAD at this checkpoint
is `e3b1ca8`.

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

## Scrivener format assemblies — implementation in progress, 26 November 2026

Imported Scrivener compile formats now create separate WIDE manuscript
assemblies. Each profile retains the source-format identifier, translates
heading and scene-separator presentation rules, selects the configured default
front/back matter, and applies Scrivener's recoverable binder inclusion and
layout body-suppression settings without changing canonical inclusion. Profile
configuration is stored project-locally under
`.writers-ide/scrivener-profiles/`; each assembly remains editable through
WIDE's existing assembly operations. The CLI can list profiles and compile a
selected profile after the author explicitly chooses a WIDE output format and
project-relative destination. A Scrivener format ID is never guessed to mean
EPUB, DOCX, PDF or another WIDE renderer.

Synthetic application coverage verifies distinct retailer-style front/back
matter selection, private matter remains absent from ordinary compilation,
the selected assembly compiles through the standard publication pipeline, and
the result is saved to the selected destination. Remaining work includes
exposing profile selection and output choice in the terminal publishing
workflow, mapping all source layout constructs that WIDE cannot yet represent,
and exercising the profiles against real Scrivener projects whose compile
settings contain usable section-layout mappings. The three controlled golden
projects and expanded private corpus must be rerun before this integration is
ready for publication UAT.

### Imported Scrivener layout warnings in compile diagnostics — 27 November 2026

Warnings retained on an imported Scrivener edition profile now flow into the
shared compile artifact and recorded compile result as `SCR001` warnings. The
diagnostic names the profile in author-facing terms, explains that its source
format selections need review, and avoids exposing UUID-like layout identifiers
in the primary message. Profile compilation and ordinary compile-result
recording now use the same artifact path, so warnings are not lost between
preview and publication. The original source-layout IDs remain available in
the import report for detailed investigation; missing or uncertain layout
assignments remain warnings and are not guessed.

The synthetic regression test verifies warning propagation to a Markdown
compile result and checks that UUIDs are redacted from its diagnostic text.
Verification on this revision: the focused profile test, full offline workspace
tests (753 passed, 1 ignored; the ignored protected-project test requires the
private ArrowMaking fixture variable), workspace Clippy with warnings denied,
`cargo fmt --all -- --check`, and `git diff --check` passed. This diagnostics
change does not alter the Scrivener importer or renderer, so the private real
manuscript publication harness was not rerun for it. Publication-profile
fidelity remains incomplete and awaits further automated and human verification.

## Standalone Scrivener import UAT repair — verified in automation, 26 November 2026

The ordinary File → Import Scrivener as New Project workflow now proposes a
validated automatic mapping for supported binder structures, keeps ambiguous
planning content private, preserves empty outline placeholders, and treats
format-selected front/back matter ancestry as organisational sections so that
valid selections do not fail parent-kind validation. The author can choose an
explicit destination through the shared directory browser; the destination
path is shown, and source-subtree, symlink-alias, non-writable and existing
destination cases are rejected before creating a project. On success the
terminal opens the imported project in the unified Outline with an imported
scene selected. Import errors are translated to binder paths for the primary
message, with technical detail available separately.

Regression evidence on this revision: the synthetic PTY workflow completes
source selection → automatic mapping → explicit destination → import → edit
and save → compile DOCX → exit/reopen and content verification. The protected
`ArrowMaking.scriv` corpus project also completes the production terminal
workflow; its source fingerprint is identical before and after. Root-level
fixture SHA256SUMS verification passed before and after. These results verify
automation only; #53/#44 still require author UAT. The import PTY harness was
adjusted to accept the actual indented outline row marker, and an outdated
terminal assertion/XML fixture was corrected.

Verification on the same code revision: `cargo fmt --all -- --check`,
`git diff --check`, workspace Clippy with `-D warnings`, and the offline
workspace suite passed (including the loopback-only Ollama test with the
author-authorised socket permission). Focused protected-project command:

```sh
WIDE_PRIVATE_ARROWMAKING_PROJECT="$PWD/fixtures/back-catalogue/scrivener/ArrowMaking.scriv" \
  cargo test --offline -p writers-ide-terminal --test pty_smoke \
  protected_arrowmaking_project_imports_through_the_production_pty_without_source_changes \
  -- --ignored --nocapture
```

The source remains the authority; a successful import does not establish
publication fidelity. Scrivener per-format layout and inclusion settings have
also been translated into WIDE named assemblies and are available to the CLI,
but selection/output destination still need to be integrated into the
terminal Publish workflow and verified against the expanded private corpus.

### Regression planner parity for format-selected matter — 26 November 2026

The private controlled-book harness now applies Scrivener's explicit
format-selected front/back matter IDs when building its reviewable mapping.
Items selected under a private Project Notes root retain their private parent
classification while the selected documents receive their publication-matter
roles. Conflicting front/back role assignments remain Unmapped, and Trash
always remains excluded. This brings the harness mapping in line with the
production import planner instead of losing valid matter during review.

`python3 -m unittest discover -s tools -p 'test_real_manuscript_regression.py'`
passed 20 tests. A fresh book-03 full regression at
`reports/profile-matter-harness-20261126/` records selected FrontMatter and
BackMatter mappings, unchanged source fingerprints, and EPUBCheck 4.2.6 with
zero generated EPUB errors or warnings. Fixture SHA256SUMS passed before and
after the run. The import remains incomplete with unsupported mappings and
publication fidelity remains provisional; this harness correction does not
resolve Scrivener's missing section-layout rules or establish acceptance.

### Alpha UAT 30 project lifecycle repair — 26 November 2026

The immediate authoring priority has shifted from publication-profile work to
project creation, writing, saving and reopening. The New Project interaction
now opens a compact title and Fiction/Non-fiction chooser; Fiction is the
default. Creating a project allocates persistent application-managed staging
storage and opens the normal workspace without requiring a permanent folder.
The first explicit Save promotes the staged project through the existing
collision-safe Save As flow; later saves use that permanent location.

Open Project treats a folder containing `project.toml` as one project object.
Enter opens it rather than showing its internal files, and F5 uses the
currently focused file-browser selection. If an uncommitted or temporary
project is active, selecting a different project first offers Save, Discard or
Cancel; the target is validated before changing workspaces, and Cancel keeps
the current project open. The Scrivener automatic-import path continues to
create and open a staged project without compulsory mapping review.

Production PTY regression now exercises a new fiction project from its
title/type dialog through chapter and scene creation, prose editing, autosave,
first Save promotion, exit, reopen and another edit/save. A second PTY journey
creates a Non-fiction project, writes directly into chapter prose without
creating scenes, autosaves and promotes it; persistence is then verified by
reopening through the application service. Existing synthetic Scrivener PTY
coverage exercises import, editing, saving, reopen and compile. Unit tests
cover atomic project-folder opening, temporary-project Open cancellation, and
staging lifecycle; the reported false external-change conflict and missing
scene rows have not reproduced in the direct chapter-prose navigation/import
PTY regression. Conflict detection remains enabled. These are automated
results only; issues #37, #38, #44, #46, #52, #53, #54 and #55 remain open for
the author's requested UAT.

The report of an external-change conflict while switching to direct chapter
prose and the reported disappearance of eleven scene rows have not reproduced
in the new/import/save/reopen PTY journeys or the persistence suite. No root
cause is confirmed, and conflict detection has not been weakened. These two
reports remain open for further reproduction and source-project comparison.

Scrivener scene statuses are also resolved through the project's status-ID
table when binder metadata stores a numeric identifier instead of its label.
Recognised source labels map to WIDE's typed statuses; absent/empty source
status does not classify already-written prose as an idea. The raw imported
status field remains in provenance metadata. Synthetic importer coverage
verifies ID lookup and status classification. This corrects the Alpha UAT 30
outliner status presentation; human confirmation remains outstanding.

Verification on the lifecycle candidate:

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — passed.
- `cargo test --offline --workspace` — passed; 29 production PTY tests passed,
  one protected-corpus test is ignored by default, and doc tests passed.
- `WIDE_PRIVATE_ARROWMAKING_PROJECT=fixtures/back-catalogue/scrivener/ArrowMaking.scriv cargo test --offline -p writers-ide-terminal --test pty_smoke protected_arrowmaking_project_imports_through_the_production_pty_without_source_changes -- --ignored --exact --nocapture`
  — passed separately; fixture SHA256SUMS passed before and after.
- Fiction, non-fiction and synthetic Scrivener import workflows were exercised
  through the production terminal PTY. The private ArrowMaking project was
  imported via PTY into isolated staging storage and its source fingerprint
  remained unchanged.

The real user-managed Alpha UAT 30 directory was not modified. Frozen Alpha
UAT 31 is available at `test-builds/alpha-uat-31-84b3b05/`, built from
`84b3b0562cca3c63ff9e7d81f9066a89e52c672b`. Its launcher PTY smoke passed,
the 41-entry `SHA256SUMS` manifest verifies, and the acceptance checklist is
`test-builds/alpha-uat-31-84b3b05/UAT-CHECKLIST.md`. Preferences and project
data are isolated within the bundle. Scrivener's per-format presentation and
inclusion controls remain outstanding and resume as the next publication
task; author UAT for the lifecycle changes remains open.

Frozen Alpha UAT 34 is available at
`test-builds/alpha-uat-34-1073cf3/`, built from
`1073cf34fccab773cf998540cd5b2cd66e40a2e8`. It includes the imported
Scrivener status-ID resolution change. The offline workspace suite, formatting,
Clippy, release build, release-fiction PTY workflow and bundle checksum
verification passed. The complete workspace PTY suite covers both fiction and
non-fiction lifecycle workflows; a separate release-binary non-fiction run was
not verified because the creation-only PTY helper assumes an initial empty
scene view. Checklist: `test-builds/alpha-uat-34-1073cf3/UAT-CHECKLIST.md`.
Human verification remains pending. Launch with `./run.sh` from the bundle
directory; preferences and disposable project data are bundle-local.

### Issue #76 PTY verification — date/time insertion — 27 November 2026

The command audit now distinguishes menu registration from production workflow
evidence for Insert Date and Time. Two Linux PTY tests exercise the actual
terminal: with an active scene, the menu action inserts text, reports success,
and the saved text survives reopen; with an empty project, the action explains
that a manuscript document must be opened, does not report success, and exits
without creating a scene or mutating manuscript content. Other date and time
variants remain covered by application unit tests; they are not claimed as
individually PTY-verified.

Verification on the implementation revision: the complete offline workspace
suite passed (755 passed, 1 ignored protected ArrowMaking fixture test), all 32
runnable production PTY tests passed, workspace Clippy with warnings denied,
`cargo fmt --all -- --check`, and `git diff --check` passed. The PTY script
failed twice while being developed because it first expected an off-screen menu
entry to be visible without scrolling, then sent Escape and quit without
allowing the terminal event loop to process the first key; both test-harness
assumptions were corrected. No production behavior was changed in this batch.
Human workflow acceptance for #76 and related UI issues remains open.

Next actionable priorities remain: complete the author-facing Scrivener
publication-profile path and its real-corpus mapping, then continue the open
v1.0 defects and workflow verification. The proposed UX design remains pending
author review; no broad interaction redesign is authorised by these tests.

### Find and Replace field focus and caret — 27 November 2026

Find and Replace now displays a visible `▏` insertion marker in the focused
Find, Replace, or editable scope field. Left/Right, Home/End, insertion, and
Backspace operate at Unicode grapheme boundaries. Focus remains visibly
highlighted independently of the caret. A production PTY opens the panel via
Edit, types replacement text before running a search, moves to Find with Tab,
and closes with Escape; project content remains unchanged. A unit regression
places the caret within an accented grapheme sequence and verifies mid-field
editing and empty-field caret display.

Verification: focused editor test and production PTY passed; the complete
offline workspace suite passed (757 passed, 1 ignored protected ArrowMaking
test); workspace Clippy with warnings denied, `cargo fmt --all -- --check`, and
`git diff --check` passed. The PTY exercises the standard 80×24 terminal. The
broader Find/Replace workflow and other terminal sizes still require author
UAT; issue #43 remains open for that acceptance.

### Approved interaction contract — implementation checkpoint, 27 November 2026

The author-approved `interaction-design-v1.0.md` is now authoritative. The
previous proposal is retained for history and marked superseded. Current
implementation gaps are recorded here to guide code changes without delaying
authoring reliability on another large design document:

| Requirement | Current status | Evidence / next implementation step |
|---|---|---|
| HIER-01, HIER-02 | Partial; schema conflict | `NodeKind` is Section/Chapter/Group, scenes have chapter-only placement, and prose is in separate content units. Plan a data-preserving adapter/migration before enabling arbitrary typed nesting or interleaved prose. |
| HIER-03 | Partial | Existing transactional create/move/archive/restore operations cover current types; complete typed-tree operations and structural undo integration. |
| OUT-01, OUT-02 | Partial | Outline and Cards are separate presentations over shared data; columns/presets and effective aggregate word counts need completion. |
| OUT-03 | Partial | Inclusion and expansion are independently visible; complete mixed inclusion, unplaced, diagnostics, unsaved and ASCII indicators consistently. |
| OUT-04 | Partial | Hoisting/filtering and presentation-only sorting require a unified-tree audit. |
| OUT-05 | Partial | Cards share identities/order/inclusion with the outline but remain a distinct workspace; consolidate presentation state without losing the card layout. |
| WS-01, WS-04 | Partial | Recursive split geometry exists; enforce the four-pane limit and test minimum sizes, collapse/restore on resize, swap and close-last-pane behavior. |
| WS-02, WS-05 | Partial | Existing workspace/session persistence exists; add approved presets and verify multi-project isolation/restoration. |
| WS-03 | Partial | Reference panes and pinned/following views exist; prove multiple editors share one buffer, undo and dirty state. |
| KEY-01, KEY-02 | Partial | Registry and user bindings exist; audit proposed global keys, add Standard/WordStar/custom profiles and conflict-safe migrations. |
| KEY-03 | Partial | Outline/card key actions exist; reconcile pane Tab behavior, selection, structural editing and visible effective bindings. |
| KEY-04, KEY-05 | Partial | Shared registry/dispatcher exists; verify modal → prefix → global → view → application → printable precedence and terminal protocol fallbacks. |
| MENU-01, MENU-02 | Partial | Menus are flat; persistent roots and context roots are not yet the approved set, and dynamic roots beyond Outline remain. |
| MENU-03 | Implemented in part | This checkpoint moves section/chapter/scene/document structure creation, renaming, movement and inclusion into the Outline menu; Insert retains prose insertion. Remaining structural actions and other context roots need migration. |
| MENU-04 | Partial | Command palette preserves advanced commands; complete command-to-route classification and contextual availability tests. |
| ED-01, ED-04 | Partial | Markdown-backed rendered presentation, formatting marks and Unicode editing exist; semantic editing coverage, Reveal Codes, inline diagnostics and document language controls remain. |
| ED-02, ED-03 | Partial | Individual and direct chapter prose editing, journalled autosave and revision restore exist; continuous multi-unit editing, stable cursor return and shared-buffer guarantees remain. |
| PUB-01, PUB-02 | Partial | Named profiles, inclusion previews and Compile IR exist; complete profile overrides, front/back matter selection, variables and preview/output workflow remain. |
| PUB-03 | Partial | Privacy boundaries and Scrivener mapping exist; real-corpus profile fidelity and unsupported-setting diagnostics remain. |
| SHELL-01, SHELL-02 | Implemented with UAT open | Temporary project creation/import, autosave, promotion, recovery and direct project opening are automated; keep #44/#53 human acceptance open. |
| SHELL-03, SHELL-04 | Partial | Identity/save indicators and progress/error feedback exist in key workflows; complete narrow-screen priority, long-operation progress and cancellation. |
| ACC | Partial | Production PTYs cover core fiction, non-fiction, import, save/reopen, compile and restore paths; broaden the approved workflow matrix and keep human acceptance separate. |

Dependency order under #77: (1) integrity and shared active-document/buffer
identity; (2) command-dispatch precedence and project lifecycle regressions;
(3) safe typed-hierarchy migration plus unified outliner over existing services;
(4) dynamic flat context menus; (5) four-pane workspace and keyboard profiles;
(6) semantic/continuous editing; (7) named publication profiles and compile
presentation; (8) full PTY/regression and human UAT. A new menu checkpoint has
begun with Outline ownership of structural actions; it is not a completed menu
redesign.

### Outline menu ownership regression repair — 27 November 2026

Section, chapter, scene and direct-prose document creation are no longer listed
under Insert. They are available through the context-sensitive Outline menu in
both Outline and Scene Cards, for fiction and non-fiction workspaces. Structural
rename, movement, inclusion and recoverable removal actions are exposed there;
the ordinary Edit menu is reduced accordingly. Empty-state guidance now names
the Outline menu. The non-fiction chapter reorder test and heading rename test
use the Outline menu; an editor menu test verifies that structural creation is
absent from Insert and reachable from Outline. Focused menu unit tests passed
(49/49), and individual PTYs passed for scene rename, heading rename and the
production menu authoring journey. The initial full test run exposed two stale
PTY assumptions: the removed-scenes command was still sought under Scene, and
the heading-rename script relied on a transient toast. Both now follow Outline;
the rename test checks the visible changed heading and its Rust wrapper checks
persisted state after reopen. Formatting and warnings-denied workspace Clippy
passed. Workspace unit and service integration tests passed; the final full
terminal PTY run passed 39 tests, with one protected private-corpus test ignored
because its opt-in path was not configured. This change is automatically
verified and remains awaiting human acceptance under #54/#77.

### New Project mnemonic dispatch regression coverage — 27 November 2026

Extended the production authoring PTY journey for #58: File-menu lowercase and
uppercase N mnemonics, Ctrl+N while the File menu is focused, and Ctrl+N from
the manuscript workspace must all open the same New Project dialogue; each
cancel path preserves the active manuscript. The expanded PTY journey passed.
The author's terminal/SSH key-delivery UAT remains outstanding.

### Frozen author test build — Alpha UAT 37, 27 November 2026

Alpha UAT 37 is frozen at source commit `322a044735937e793c4708a6d9413c687f189031` in
`test-builds/alpha-uat-37-322a044/`. Launch with:

```sh
cd test-builds/alpha-uat-37-322a044
./run.sh "Authoring Audit Project"
```

The bundle uses isolated preferences and four disposable synthetic projects;
private manuscripts and publications are excluded. `UAT-CHECKLIST.md` covers
the Outline menu, structure changes, inclusion/recovery, New Project keyboard
dispatch, editing, persistence, non-fiction and publishing. The release binary
passed production PTY journeys for fiction creation/write/autosave/first Save
and the menu-driven authoring audit. All four bundled project databases passed
SQLite integrity checks and bundle `SHA256SUMS` verified. Executable SHA-256:

```text
91b509fea3e2ca2c3dad3b404d575310a8195e7068664ca6f07cc56f72ab3143  bin/writers-ide-terminal
befdae533576d1d9616de3c86b1476f76ec8250d6739cd0329b87b337b444d67  bin/writers-ide
```

Workspace formatting and warnings-denied Clippy passed. Workspace unit and
service integration suites passed; after repairing two stale PTY assertions,
the production PTY suite passed 39 tests and ignored one private-corpus test
because its local fixture path was not configured. The expanded #58 shortcut
PTY journey passed for lowercase/uppercase N and Ctrl+N both inside and outside
the File menu. Human UAT remains required for #44, #54, #55, #58 and #76. Alpha
UAT 36 is retained as the only older Alpha bundle; Alpha UAT 37 brings the total
to two. Continue the v1.0 backlog after this checkpoint; the next priority is
the outstanding P0 rendering and key-delivery defects, then the remaining #77
interaction requirements.

### Editor render-span regression repair — 27 November 2026

Inspection of the live editor renderer found that, despite the earlier rendering
addendum describing coalesced style runs, the implementation still emitted one
Ratatui span per grapheme. Adjacent graphemes with identical styles are now
combined, while cursor, selection, semantic formatting, mention decoration and
formatting-mark style boundaries remain separate. A focused unit test verifies
both coalescing and those style boundaries. The production full-screen PTY test
passed its manuscript/reference transition and resize checks at 80×24 and
201×60. Formatting and warnings-denied workspace Clippy passed. The complete
offline workspace suite passed: 359 terminal unit tests, 3 author-workflow
tests, 39 PTY tests (one protected private-corpus test skipped because its
opt-in path was unset), and all other workspace unit, integration, CLI and doc
tests. The first focused PTY attempt captured only an Escape byte before the app
rendered; its immediate rerun passed, and the full PTY suite also passed. This
repair removes an implementation mismatch and reduces per-character terminal
render spans; it does not prove the exact cause of the author's scattered-glyph
screenshot. Issue #56 remains open for human verification. No Alpha bundle was
created at that checkpoint; Alpha UAT 36 was subsequently removed to keep the
number of retained Alpha builds within the requested limit.

### Alpha UAT 38 and production PTY harness correction — 27 November 2026

Alpha UAT 38 is frozen at product commit `0a50253800a0276ce8744bde45198a0306598bca`
in `test-builds/alpha-uat-38-0a50253/`. The bundle has isolated preferences and
four disposable synthetic projects; private manuscripts and publications are
not included. Its launch command is:

```sh
cd test-builds/alpha-uat-38-0a50253
./run.sh
```

The fiction-creation PTY harness now accepts both supported entry states: the
first-run Start screen and an already-open blank project's File menu. It emits
the captured screen and output tail when project-type selection or first-save
promotion fails, which makes future workflow failures diagnosable. Both the
updated harness and the release binary completed the fiction creation, scene,
write, save and exit journey. The full production PTY suite passed 39 tests;
one opt-in private-corpus test was skipped because its local fixture path was
not configured. Formatting, workspace Clippy and the full offline workspace
suite passed at the frozen product revision. The PTY harness-only changes do
not alter the runtime binary. Bundle checksums, shell syntax and all four
sample-project SQLite integrity checks passed. Alpha UAT 37 is the only older
numbered Alpha bundle retained, so there are two numbered Alpha builds total.

Human checks in `test-builds/alpha-uat-38-0a50253/UAT-CHECKLIST.md` remain
outstanding, especially resizing and manuscript/reference rendering. Issue
#56 remains open: span coalescing is a verified implementation improvement,
but the screenshot's exact rendering cause has not been reproduced.

## Alpha UAT 37 keyboard, outline and editor corrections — 27 November 2026

The latest uncommitted authoring repair batch addresses the Alpha UAT 37
regressions covered by the current interaction notes and terminal tests. Fiction
and non-fiction now enter the same unified Outline presentation; chapter rows
show total words including direct prose and descendant content; Outline refresh
retains the selected structural identity; and direct chapter prose opens from
the chapter instead of exposing its storage-only content row. Structural
creation, reordering, promotion/demotion, rename and removal remain available
from the Outline workflow, with named key shortcuts correctly parsed by the
shared command dispatcher. The file browser now hides dotfiles by default and
provides visible direct-path and hidden-file controls. A long-document PTY test
checks cursor visibility, insertion while scrolled, terminal resize and saved
content.

The production test run found no failures after aligning older PTY expectations
with the approved Outline interactions. Verification passed on this working
tree: `cargo fmt --all -- --check`,
`cargo clippy --offline --workspace --all-targets -- -D warnings`, and
`cargo test --offline --workspace`. Results include 362 terminal unit tests,
3 end-to-end author-workflow tests, and 40 production PTY tests passed; one
protected private-corpus PTY was ignored because its opt-in fixture variable was
not set. The Ollama loopback test passed with the author's previously approved
socket permission. The workspace test run completed successfully. GitHub was
unavailable from this session while attempting to read current issue comments,
so issue status has not been updated from stale data.

This is automated evidence only. Alpha UAT 38 remains the latest frozen human
test build at commit `0a50253800a0276ce8744bde45198a0306598bca`; no UAT 39
bundle has yet been made from this working tree. The author should retest
U37-01 through U37-19 using the next frozen candidate. The exact human outcomes
for each numbered finding remain to be entered after UAT, and #77/#44 remain
open for human acceptance. Keep the current changes uncommitted until the
corresponding verified commit and UAT bundle are produced.

### Alpha UAT 39 — Alpha UAT 37 authoring fixes — 27 November 2026

Frozen Linux x86_64 release build from commit
`88c8ea546b87879825aae38495011fbc9ff31656` is available at
`test-builds/alpha-uat-39-88c8ea5/`. From the repository root, launch with:

```sh
cd test-builds/alpha-uat-39-88c8ea5
./run.sh "The Glassmaker's Compass"
```

`UAT-CHECKLIST.md` has concise checks U37-01 through U37-19 covering the
unified outline, structural operations, inclusion and recovery, counts,
editor viewport, panes, persistence, file browser and proofreading. The bundle
contains isolated preferences and four synthetic disposable projects copied
from UAT 38; it contains no private regression manuscripts. `SHA256SUMS`
verifies both executables, launch script, manifest and checklist. The release
launcher was opened in a PTY, the menu was displayed and the application exited
cleanly. All four bundled SQLite databases passed `PRAGMA integrity_check`.
Automated implementation verification is recorded above; human acceptance is
pending. Alpha UAT 37 was removed so Alpha UAT 38 and Alpha UAT 39 are the only
two retained numbered Alpha bundles.

## Production PTY coverage for independent split-pane navigation — 27 November 2026

Added a terminal-level author workflow for #25/#77: open the View menu, split
the active manuscript window side by side, scroll the new pane, switch focus
through View → Next Window, then scroll the original editor pane. The test
reconstructs the rendered 80×24 terminal and verifies each pane retains its
separate visible position; reopening confirms the manuscript bytes were not
changed. The focused PTY passed. Follow-up verification passed `cargo fmt --all
-- --check`, workspace Clippy with warnings denied, and the full offline
workspace test suite: 362 terminal unit tests, 3 author-workflow tests, 41
production PTY tests and all other workspace/integration/doc tests passed; one
protected ArrowMaking PTY was intentionally ignored because its local fixture
path was not configured. This demonstrates independent scrolling and focus in
the tested split layout. It does not establish complete pane navigation parity
or human usability; those remain open for UAT.

## Review-menu proofreading workflow — 27 November 2026

Added a production PTY workflow that opens a scene, runs Review → Grammar
Check, accepts a suggested correction, saves, exits and verifies the corrected
text after reopening. Exercising the workflow found that manual checks ran
synchronously and could hold the terminal while the embedded grammar engine
worked. Manual single-document checks now reuse the existing background worker,
return immediately with progress feedback, preserve passage-language
overrides, and show their results in a focused review pane. The pane is labelled
as a review rather than read-only, and its footer exposes the apply, ignore and
return keys. The full verification run passed: formatting, workspace Clippy
with warnings denied, workspace tests (including 363 terminal unit tests, 3
author workflows and 42 production PTY workflows; 1 private-corpus test was
ignored because its path was not configured). The new proofreading PTY test
verified correction, saving and reopening. Human UAT is still required for the
review interaction.

### Current-document spelling review — 27 November 2026

Review → Spell Check now uses the open scene or direct-prose document, inherits
its language, and displays candidates in a focused results pane instead of
scanning unrelated manuscript documents and placing a long summary in the
status line. The operation leaves prose unchanged. A production PTY test uses
two scenes with distinct misspellings to verify the current-document scope and
reopens the project to verify persistence. The current implementation lists
candidate words and counts; interactive misspelling navigation and replacement
remain outstanding under #33 and U37-11.

Verification passed: formatting, warnings-denied workspace Clippy, and the
full offline workspace suite (363 terminal unit tests, 3 author-workflow tests,
43 production PTY tests passed; 1 opt-in protected-corpus PTY ignored). The
first full run had one transient scene-rename PTY with an empty screen; it
passed immediately in isolation and passed in the final full run. The document
export test that failed in the preceding run also passed in isolation and in
the final full run. This evidence is automated; human UAT remains open.

### Alpha UAT 40 — proofreading scope and responsiveness — 27 November 2026

Frozen Linux x86_64 release build from commit `d8fa071` is available at
`test-builds/alpha-uat-40-d8fa071/`. Launch from the repository root:

```sh
cd test-builds/alpha-uat-40-d8fa071
./run.sh "The Glassmaker's Compass"
```

The bundle uses isolated preferences and disposable synthetic projects; it
contains no private manuscript material. Its checklist covers U37-01–U37-19
and adds an explicit two-document Spell Check scope exercise. Checksums passed,
four sample project databases passed SQLite integrity checks, and the release
binary completed a PTY Review-menu launch and exit smoke test. Verification at
the frozen source commit passed formatting, workspace Clippy with warnings
denied, and the full workspace suite (363 terminal unit tests, 3 author-workflow
tests, 43 PTY workflows; 1 protected-corpus PTY skipped because its local path
was not configured). Human UAT remains pending. Alpha UAT 38 was removed to
retain only Alpha UAT 39 and Alpha UAT 40.

### U37-11 spelling-result navigation — implementation in progress

Review → Spell Check now lists each occurrence with a one-based line and
grapheme-column location. Enter returns to the source document, scrolls a
misspelling below the viewport into view, selects its exact source range, and
marks the affected line with `!` while preserving the surrounding prose.
Navigation refuses stale source text. A production PTY regression now checks a
misspelling on line 41, verifies it is visible and marked in context, and
reopens the project to confirm both documents are unchanged. A unit test covers
repeated Unicode words, combining-mark columns and UTF-8 source ranges. This
advances U37-11; interactive suggestion/replace/ignore and dictionary controls
remain outstanding under #33.

Verification passed formatting, warnings-denied workspace Clippy, the new
Unicode occurrence unit test, and the full offline workspace tests apart from
one load-sensitive fiction reopen PTY timeout. That workflow passed when run
alone; its autosave wait was raised from five to fifteen seconds. The complete
production PTY suite then passed serially (43 passed, 1 protected-corpus test
ignored because its private path was not configured). The full workspace run
had 364 terminal unit tests, 3 author workflows, and all other crate and
integration tests pass. Human acceptance remains open.

### Alpha UAT 41 — spelling navigation candidate — 27 November 2026

Frozen Linux x86_64 release build from pushed commit `87156d5` is available at
`test-builds/alpha-uat-41-87156d5/`. Launch it from the repository root with:

```sh
cd test-builds/alpha-uat-41-87156d5
./run.sh "The Glassmaker's Compass"
```

The bundle has isolated preferences and disposable synthetic projects. Its
checklist covers U37-01 through U37-19, including the new occurrence-level
Spell Check navigation. The release PTY smoke opened the bundled Glassmaker
sample at 80×24, verified the rendered title and prose, and exited cleanly via
Ctrl+Q. `SHA256SUMS` verifies both release executables, launcher, manifest and
checklist; all four sample project databases pass SQLite `integrity_check`.
One attempted test reused an assertion for a different sample manuscript and
was invalid; the replacement sample-specific PTY smoke passed. The complete
workspace suite had one load-sensitive PTY timeout under parallel load; that
workflow passed in isolation, the PTY timeout was increased, and the complete
PTY suite passed serially (43 passed, 1 protected-corpus test ignored). The
workspace suite was not rerun after that test-only timeout change. Human UAT is
pending. Alpha UAT 39 was removed; Alpha UAT 40 and 41 are the two retained
numbered builds.

### Alpha UAT 40 follow-up — new authoring priorities

The author’s consolidated UAT 40 report confirms that vertical editing,
word-count totals, layout persistence, pane switching, scene renaming, direct
chapter prose, date/time and special-character insertion, Spell Check and
offline thesaurus improved. Preserve these results. New top priorities are a
global directional pane controller, dependable internal copy/paste across
SSH panes, generic outliner operations, read-only external text viewing,
formatting-mark/tab display correctness, interactive proofing and configurable
document statuses. Human acceptance remains open. The immediate next task is
to route the approved Alt+Arrow, F6/Shift+F6 and View-menu pane navigation
through one global command path, with regression coverage for focus and
independent pane state. This direct Alt+Arrow default was superseded by the
author's later Ctrl+Backslash prefix proposal below.

### Window prefix compatibility and notation — active implementation

The author subsequently proposed a GNU Screen-style prefix, replacing direct
Alt+Arrow pane navigation as the default. WIDE now uses Ctrl+Backslash followed
by a separate key: directional arrows, stable pane numbers 1–4, Tab/Shift+Tab,
H/V split, X close, M maximise/restore, P saved layout, ? help, or Esc cancel.
The same actions remain available from the View menu and F6/Shift+F6. Direct
Alt+Arrow commands remain user-bindable but have no default assignment. A
user-defined binding for the prefix event takes precedence over the default.

**Terminal byte detail:** the conventional terminal encoding of Control+Backslash
is ASCII 0x1C (FS). In raw mode, Crossterm's Unix input parser decodes this as
`Ctrl+4`; conventional Ctrl+4 uses the same byte and cannot be distinguished.
WIDE accepts `Ctrl+4` as a configuration alias and displays the canonical
`Ctrl+\` spelling. A person who needs Ctrl+4 for another command must rebind
the window prefix. `Ctrl+\` is the simultaneous two-key chord; a comma denotes
the following independent key (for example, `Ctrl+\, 1`). A literal plus is
never part of the prefix. Ctrl+/ remains a separate simultaneous chord.

| Environment | Evidence and result |
|---|---|
| Linux PTY, Crossterm raw mode | **Tested:** injected byte 0x1C activates the prefix and the next independent arrow/number key moves focus; Esc cancels, invalid keys (including a literal `+`) are consumed without editing, and shutdown restores ISIG and VQUIT. |
| GNU Screen and tmux | **PTY integration passed:** the production prefix test traversed the installed GNU Screen and tmux layers, including Escape cancellation, directional and numbered focus, and clean teardown. This verifies the tested Linux PTY setup only; it does not establish compatibility with arbitrary host bindings or other terminal emulators. GNU Screen defaults to C-a and tmux to C-b; defaults alone do not establish behavior under custom host bindings. |
| Terminator | **Not interactively tested:** its documented defaults use Ctrl+Shift+O/E for splits; those chords are no longer WIDE's defaults. The current session has no usable X display for this terminal GUI. |
| GNOME Terminal, Konsole, XTerm, Windows Terminal, PuTTY, macOS Terminal, iTerm2, native Linux/BSD consoles | **Not interactively tested.** Konsole and the macOS/Windows environments are unavailable here; the presence of a command or protocol documentation is not an interaction test. Windows Terminal permits configurable keybindings, so a host mapping can still consume keys. |
| SSH and keyboard layouts | **Not interactively tested:** no SSH PTY or physical UK/US/international keyboard is attached. The PTY regression checks bytes received by WIDE, not which physical key a remote client emits. |

The production PTY workflow has passed for the bare Linux PTY and the installed
GNU Screen/tmux PTY layers. It creates two editor panes, verifies independent scrolling, sends raw 0x1C and
separate arrows/numbers (with an explicit assertion for the independent bytes
0x1C then 0x31 selecting pane 1), tests help/cancel/invalid/literal-plus
sequences, maximise/restore, F6/menu fallback and close/split, then verifies
manuscript text and terminal mode after exit. A separate startup test verifies
0x1C alone visibly activates the prefix after raw mode is active without
raising SIGQUIT. Unit tests cover `Ctrl+\\`/`Ctrl+4` parsing, TOML keymap
round-trip, canonical shortcut formatting, menu label, user-binding
precedence, `Ctrl+Shift+O` simultaneous modifiers and comma-separated sequence
notation. Formatting and Clippy passed. The latest full workspace run passed
all suites, including 377 terminal unit tests, three author-workflow tests, and
46 production PTY tests; one optional private-corpus PTY test was ignored
because its local path was not configured. The nested Screen/tmux prefix test
passed on this run. The author has not human-accepted these changes.

## Application-wide keyboard consistency checkpoint — 27 November 2026

Implemented the approved keyboard interaction standard across the shared command
router, window-management prefix, authoring dialogues and editing keymap profiles.
The `Ctrl+\\` prefix is centrally decoded and cancellable, with arrows, numbered
pane focus, next/previous pane, split, close, maximise, layout selection and help
commands. The production PTY sends ASCII `0x1C` and then a separate ASCII `0x31`
and verifies the visible result `Focused pane 1`; it sends no literal plus key.
The same prefix route is exercised from non-modal workspaces and on startup,
including verification that terminal signal and VQUIT settings are restored.

Global Save now remains available from the outliner: temporary projects open the
first-save destination browser, while permanent projects acknowledge their
transactionally saved structural changes. Standard global shortcuts take
precedence over WordStar editor defaults. No-function-key PTY coverage exercises
editing, save, find, menu, return to outline, quit and persistence using ordinary
keys. Dialogue/file-browser controls expose Enter, Esc, Tab/Shift+Tab, arrows
and Space paths; function-key bindings remain optional alternatives.

Verification at this checkpoint: `cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets --offline -- -D warnings` passed.
Focused tests passed for global Save from the outline, prefix activation in all
non-modal workspaces, prefix layout selection, raw Ctrl+/ terminal decoding,
keybinding precedence, project keybinding persistence, file-browser import
controls, no-function-key authoring PTY, and Screen/tmux PTY prefix workflow.
`cargo test --workspace --offline` passed: 377 terminal unit tests, 3
author-workflow tests, 46 production PTY tests passed and 1 optional private
fixture PTY test was ignored. Human UAT remains required; other terminal
emulators have not been interactively tested.

### Frozen Alpha UAT 42 — 27 November 2026

The Linux x86_64 release bundle is `test-builds/alpha-uat-42-dac9c35/`, frozen
from product commit `dac9c3530ada24b5d8693a07626cdfcf558e9757`. Launch from the
repository root:

```sh
cd test-builds/alpha-uat-42-dac9c35
./run.sh "The Glassmaker's Compass"
```

Use `./run.sh "Chapter-Based Sample"` for the synthetic non-fiction sample.
The bundle has isolated HOME/preferences, disposable projects, a concise
`UAT-CHECKLIST.md` and `SHA256SUMS`; checksum verification passed. A release
binary PTY smoke at 80×24 rendered the sample prose and exited cleanly using
the visible Quit Without Saving action. No private manuscript content is
included. Alpha UAT 41 and 42 are the two retained numbered Alpha builds.
Human acceptance remains pending.

Next implementation priorities from the latest author direction: verify and
complete cross-document internal clipboard use (including external text), then
compatible node conversion, bulk structural insertion, project-configurable
statuses, remaining command-syntax prompts, and interactive spelling and
selected-text thesaurus workflows. Skip work already fully implemented and
verified. Publication-profile fidelity, new import formats, reusable templates
and nonessential features are deferred until after the allowance reset.

### Post-UAT-42 clipboard workflow correction — 27 November 2026

The production PTY now exercises copying from one manuscript document into a
second, then selecting text from an external read-only file and pasting it into
the manuscript. It exposed that Ctrl+Backslash + Tab cycled reference buffers
without restoring manuscript focus when no explicit pane layout existed.
`focus_window_next` now promotes open reference buffers to the shared pane
layout before cycling. The focused production PTY passed, including save/reopen
verification and proof that the external file remains unchanged. Two prefix
unit tests, the reference-window menu test, terminal Clippy with warnings
denied, and formatting passed. The full workspace suite has not yet been rerun
for this post-UAT-42 change. The existing UAT 42 remains frozen at its declared
source commit; this correction is in the next development checkpoint.

### Compatible outline conversion checkpoint — 27 November 2026

Added a guarded schema migration and transactional conversion service for
section, chapter and group node kinds. SQLite rejects conversions that would
make child hierarchy, placed scenes or heading prose invalid. The Outline menu
now offers **Convert Selected Section or Chapter…** with a keyboard-selectable
dialogue. The current author-facing outliner supports section/chapter targets;
Group remains unavailable there because the outliner does not render Group
nodes. Failed conversions leave the dialogue open and the hierarchy unchanged.

Verification: storage tests passed (98), app tests passed (64), terminal unit
tests passed (379), author workflow tests passed (3), and the production PTY
suite passed 46 tests with one optional protected private-fixture test ignored.
The full PTY run's Screen/tmux test was denied temporary socket creation by the
sandbox; rerunning that test with the previously authorised socket access passed.
Formatting, targeted Clippy with warnings denied, and `git diff --check` passed.
No new UAT build has been made; Alpha UAT 42 remains frozen at `dac9c35`.
Conversion has automated verification but has not had human UAT.

### Bulk outline insertion checkpoint — 27 November 2026

The Outline menu now has **Insert Multiple Items…** for numbered chapter and
section runs, and can create a numbered run of scenes in the selected chapter
or as unplaced scenes. Its visible dialogue accepts item type, quantity
(1–100), a `{n}` naming pattern and starting number. Structural and scene
batches are transactional; sibling positions are compacted safely, manuscript
files are staged and cleaned on failure, and invalid input leaves the outline
unchanged.

Focused verification passed for transactional storage insertion/persistence
and the terminal dialogue, rendering, sequential chapter names, nested section
names, scene type selection and invalid-count rollback. A production 80×24 PTY
test used the Outline menu to create five chapters and five scenes, exited,
reopened the project and verified their names, order and chapter placement.
Storage scene-batch tests, the focused terminal test, formatting and targeted
Clippy with warnings denied passed. Human UAT remains pending. No new Alpha
build has been generated; Alpha UAT 42 remains frozen at `dac9c35`.

### Project manuscript status choices — 28 November 2026

Schema migration 54 adds ordered status choices to each project with
author-facing default labels. The Outline menu now provides a status picker for
the selected scene. Tools > Manuscript Statuses lets authors rename,
enable/disable, reorder and add custom choices to the project's statuses; existing scene status codes
remain stable, including imported and archived states. At least one choice must
remain enabled, and changes are saved transactionally.

Migration 55 extends project status definitions with stable custom status IDs.
The status settings workflow now supports adding named stages, and the status
picker can assign them. Regression coverage verifies project persistence,
scene assignment after reopen, and direct SQL rejection of malformed custom
status codes. Outline, Scene Cards and related result views render configured
status labels instead of internal custom IDs. The storage suite passed (102
tests), the production status-configuration PTY and focused label-rendering
test passed, and targeted Clippy with warnings denied passed.
Human UAT remains pending; Alpha UAT 42 remains frozen.
The production PTY also assigns “Third Draft” through Outline > Set Scene
Status, then reopens the project and verifies the assignment persisted.

### Author-facing argument prompt correction — 28 November 2026

Registered commands that need free-form arguments now keep the internal
dispatch prefix out of the visible input line. The screen presents the
command's author-facing description and only displays the details the author
types. Backspace cannot erase the hidden dispatch prefix; Enter still routes
the same arguments through the existing command dispatcher, and Esc cancels.
Scene move and planning-field palette actions route to their existing
selection workflows instead of requesting command syntax.

Verification passed with a rendered terminal regression test asserting the
internal command name is absent while user input remains visible, the scene
status production PTY regression, formatting and terminal Clippy with warnings
denied. This does not replace dedicated forms for every advanced command;
advanced command-palette operations remain available, while broader dialog
work is still outstanding.

### Interactive spelling correction — 28 November 2026

Spell Check results can now correct the selected occurrence with `C`, using
the first suggestion from the configured Aspell language. The correction
validates the reviewed source and exact occurrence before replacing text,
adds the change to the editor undo stack, and returns focus to the manuscript.
It does not silently change prose; the author explicitly accepts the action
and can undo it before saving.

The production Linux PTY regression checks the current-document result,
applies a custom correction, saves, and verifies the persisted text after
reopening while confirming another scene remains unchanged. Press `C` accepts
Aspell's first suggestion; `R` opens a custom replacement prompt. Both edits
are undoable. The subsequent selectable-suggestions checkpoint below
supersedes that limitation. Production PTY, formatting and terminal Clippy
checks passed.

### Selected-text thesaurus — 28 November 2026

Review > Thesaurus now uses an active manuscript selection automatically and
opens local results in a second pane. `[` and `]` choose among the displayed
synonyms; `R` applies the current choice to the original selection. The edit is
undoable and remains in the normal save flow. Without a selection, the existing
word-entry and standalone results workflow remains available. A production PTY
regression selects “colour”, looks up the local dictionary, applies “hue”,
saves and verifies reopening; a focused test cycles to and applies a non-first
synonym. Formatting and terminal Clippy passed. Human UAT remains open.

### 28 November terminal regression checkpoint

After the spelling correction and selected-text thesaurus increments, the full
terminal package suite passed: 381 unit tests, 3 author-workflow tests and 50
production PTY tests passed; 1 private-corpus PTY test was skipped because its
local fixture path was not configured. The nested Screen/tmux PTY gate first
hit the sandbox socket restriction, then passed when retried with the
previously authorised socket permission. `cargo fmt --all -- --check`,
terminal Clippy with warnings denied and `git diff --check` passed. The frozen
Alpha UAT 42 bundle at `test-builds/alpha-uat-42-dac9c35/` passed its SHA-256
manifest verification; its source remains frozen at `dac9c35`.

### Multiple spelling suggestions — 28 November 2026

Spell Check now offers `S` to open an author-facing suggestion selector. Up/Down
chooses among local Aspell candidates, Enter applies the selected correction,
and Esc cancels. The chosen replacement follows the existing source/range
validation, undo and save path; `C` remains the quick accept-first action and
`R` remains custom replacement. A production Linux PTY selected a non-first
suggestion, saved it, reopened the project and verified another document stayed
unchanged. The existing production PTY custom-correction regression also
passed. Targeted terminal Clippy with warnings denied and formatting passed.
Human UAT remains open.

The complete terminal package suite passed after these authoring changes:
384 unit tests, 3 end-to-end author workflows, and 51 production PTY tests
passed; 1 private arrow-making import PTY test was skipped because its local
fixture path was not configured. The package was run offline. Formatting,
terminal Clippy with warnings denied, and `git diff --check` passed at the
custom-status checkpoint.

### Frozen Alpha UAT 43 — 28 November 2026

The current authoring fixes are packaged at
`test-builds/alpha-uat-43-c2bc4d0/`, frozen from
`c2bc4d05aa388b7867795bbc0a943d1b94b9ad0f`. Launch from the repository root:

```sh
cd test-builds/alpha-uat-43-c2bc4d0
./run.sh "The Glassmaker's Compass"
```

`UAT-CHECKLIST.md` covers cross-document and external-file clipboard use,
outline conversion and bulk insertion, custom status assignment, spelling
suggestions, selected-text thesaurus replacement, save/reopen and publishing.
The release binary was launched in an 80×24 PTY, displayed the bundled sample
workspace and exited cleanly. `sha256sum -c SHA256SUMS` passed for the binary,
launcher, manifest, checklist and disposable sample projects. At generation,
UAT 42 and UAT 43 were retained; UAT 42 was later removed when UAT 44 was
packaged, leaving only UAT 43 and UAT 44. Human acceptance remains pending.

Workspace verification: formatting and workspace Clippy with warnings denied
passed. The first offline workspace test run was blocked by the sandbox's
loopback socket restriction; the Ollama test passed when retried with the
authorised socket permission. In the elevated full-workspace rerun, all
reported crate tests passed except one terminal project-creation PTY test that
could not find its outline menu; the same PTY test passed immediately when run
alone, and the complete terminal package suite had passed. Treat workspace
tests as not yet clean in aggregate; investigate the intermittent PTY result
before claiming a full workspace pass.

The flaking project-creation PTY check was tightened to wait for the Outline
workspace, title-dialogue dismissal and the new chapter's selected state
instead of accepting stale prompt text. Its focused production PTY passed
twice after that change. The complete workspace suite has not yet been rerun
after the test-harness correction.

### Spell Check Ignore Once — 28 November 2026

The Spell Check results pane now accepts `I` to dismiss only the selected
occurrence from the current review; it does not change manuscript text or add a
persistent dictionary entry. The visible result count and list update, and the
80×24 footer exposes the shortcut. A production PTY ignored the first of two
same-document occurrences, corrected the second, saved/reopened, and verified
the ignored prose and a different scene were unchanged. The focused state test,
PTY, terminal Clippy, formatting and diff checks passed. Human UAT remains
open.

### Frozen Alpha UAT 44 — 28 November 2026

Alpha UAT 44 is at `test-builds/alpha-uat-44-e92f21e/`, frozen from
`e92f21ebf1f5ddeef5a7ed99f6aa04b133dace61`. Launch from the repository root:

```sh
cd test-builds/alpha-uat-44-e92f21e
./run.sh "The Glassmaker's Compass"
```

The checklist includes the new Ignore Once proofreading action. The bundled
release binary launched the synthetic sample in an 80×24 PTY and exited
cleanly; its checksum manifest passed. Only Alpha UAT 43 and 44 are retained.
The Ignore Once state and production PTY tests passed, but a complete workspace
test run has not yet passed cleanly in aggregate; see the verification note
above. Human acceptance remains pending.

### Project spelling dictionary — 28 November 2026

The current-document Spell Check results pane now accepts `D` to add the
selected word to a language-specific project dictionary at
`.writers-ide/dictionaries/<language>.txt`. Matching results disappear from
the current review, and the pane confirms the change without altering prose.
Later Spell Check runs load the saved list. Dictionary writes use a synced
same-directory temporary file and atomic replacement; invalid language tags,
multiword entries and symlinked dictionary paths are rejected.

Focused state tests, the spelling correction PTY and a production PTY that adds
a word, exits, reopens the project and confirms the word is no longer flagged
passed. Terminal Clippy with warnings denied, formatting and diff checks passed.
The full workspace suite was not rerun for this increment; human UAT remains
pending.

### Workspace verification and spelling-test correction — 28 November 2026

`cargo test --workspace --offline` passed after correcting a stale PTY
expectation in the non-first spelling suggestion test (the test's fixture
contains `Baad word.`, not a second occurrence). Results: 386 terminal unit
tests, 3 end-to-end author workflows, 52 PTY tests passed, 1 protected
private-corpus PTY test skipped because its local fixture path was not set; all
other workspace and doc tests passed. Workspace Clippy with warnings denied,
formatting and `git diff --check` passed. The skip is not reported as a pass.
Human UAT remains pending.

### Frozen Alpha UAT 45 — 28 November 2026

Alpha UAT 45 is at `test-builds/alpha-uat-45-0017fcc/`, frozen from
`0017fcc9c0517df62fc9bfaccb32f68876a7d6ff`. Launch from the repository root:

```sh
cd test-builds/alpha-uat-45-0017fcc
./run.sh "The Glassmaker's Compass"
```

The bundle includes isolated preferences and disposable synthetic fiction and
non-fiction projects. It adds the project dictionary workflow to the Alpha 44
proofreading actions. The release executable passed the production PTY fiction
creation/write/save/exit smoke workflow, and all bundle checksums verified.
Alpha 43 was removed after Alpha 45 passed, leaving only numbered Alpha 44 and
45 bundles. Human acceptance remains pending.

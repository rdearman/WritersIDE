# Scrivener import fidelity report — Issue #35

**Report scope:** local, read-only import test of the author-provided
`test-fixtures/scrivener/Adderand.scriv` project. This report contains no
manuscript prose, document titles, source identifiers, or source fingerprint.
The original project is excluded from Git.

## Detected format and inventory

- Scrivener application metadata: `SCRWIN-3.1.1.0` (Scrivener for Windows
  3.1.1.0).
- Manifest revision: `2.0`.
- Main manifest and the manifests inside `binder.autosave` and `binder.backup`
  were structurally equivalent; both archives contained the main manifest.
- Binder inventory: 5 items — 1 Draft folder with 1 text document, 1 empty
  Research folder, and 1 Trash folder with 1 text document.
- The selected import mapping produced 1 chapter, 1 scene, 1 research
  reference, and 2 explicitly skipped Trash items. Binder order and parent
  links were retained. The selected text document remained a single scene.
- The imported source inventory recorded all 5 binder records and 3
  entity-provenance links. The `Both` import also retained a hash-verified copy
  of the complete Scrivener project in the destination's
  `research/imported_sources` directory.

## Manuscript and formatting checks

- The manuscript RTF file was 55,366 bytes. The importer extracted 37,417
  Markdown characters (6,598 whitespace-delimited tokens).
- The imported scene text exactly matched the importer's read-only RTF
  extraction, both before and after persistence. A test confirms that 366
  non-ASCII characters from RTF Unicode escapes survived extraction.
- Bold and italic RTF content is represented using Markdown emphasis. The test
  confirms both emphasis forms are present in the imported scene.
- The RTF contained paragraph controls and 3 explicit line-break controls.
  The current importer normalizes both `\par` and `\line` to Markdown paragraph
  separators; line-break-versus-paragraph semantics are therefore not
  preserved.
- No RTF picture or object controls were found in the manuscript document.
- No synopsis or notes files were present for the imported manuscript item.
- The binder title and compile-inclusion value were inventoried. Source binder
  IDs and raw binder fields remain traceable through import provenance. UI
  selection state and Scrivener-specific `TextSettings` are not converted to
  Writer's IDE editing metadata.

## End-to-end verification

The integration test used the actual source directory and imported into a new
temporary Writer's IDE project in `Both` mode. It then:

1. Compared SHA-256 hashes for every source file before and after import.
2. Compared the source-tree fingerprint before and after import.
3. Verified the imported chapter, scene, research reference, scene title,
   ordering, provenance counts, Unicode and inline emphasis.
4. Appended a test-only edit to the imported scene, closed the project, reopened
   it, and verified the saved text and full-text search result.
5. Compiled the edited project to EPUB and verified the test edit appears in
   the EPUB archive.
6. Registered this project and a second, distinct licensed Scrivener for Mac
   fixture as separate books in one series, checked their stable distinct
   project identities and read-only series references, and verified the
   second book's imported prose remained intact.
7. Created Book 3 in that series, added a chapter-level prose unit without
   creating a scene, drafted immediately, reopened the project, verified series
   membership, and compiled an EPUB containing the new text.
8. Re-inspected the retained `Both`-mode project copy and verified its source
   fingerprint matches the original.

The test project and its edited manuscript are temporary and are not committed.
The real-project test runs when the excluded user fixture is present; otherwise
it reports a local skip. A separate sanitized Windows-shaped regression
fixture runs in the regular test suite but is not treated as real-project
compatibility evidence.

## Unsupported or unverified fidelity

- The import run is reported as incomplete because the importer conservatively
  flags RTF font, colour, layout, and embedded-object fidelity for review.
- Font selection, font sizes, colour, page layout, headers/footers, and other
  Scrivener compilation settings are not translated into Writer's IDE styling
  or compile configuration. The complete source project is retained in `Both`
  mode, but those settings are not active in the editable manuscript.
- The `\line` to paragraph-separator normalization is a confirmed formatting
  semantic loss.
- Scrivener's search index, writing history, UI state, style catalog, and
  compile configuration remain reference-only inside the preserved source
  project; they are not imported as editable Writer's IDE entities.
- This project has only one manuscript text document. It does not validate
  large or multi-chapter imports, rich synopsis/notes coverage, embedded media,
  complex styles, malformed-source recovery, or every Scrivener platform and
  version.
- A Scrivener 2 project is still inventory-only for unsupported manifest
  revision 1.0. No broad Scrivener version-support claim is made.

## Result

The actual Windows 3.1.1.0 project passes the tested read-only discovery,
explicit mapping, staged `Both` import, source immutability, editable scene
save/reopen, search, and EPUB smoke path. Compatibility remains experimental
and bounded to the tested project structure and preserved formatting subset.

## Terminal workflow follow-up

The terminal Series menu now exposes series creation and selection, current-book registration, Scrivener import, new-book creation, and read-only series references through the central command registry. The mapping screen lets the author inspect each binder item and adjust its destination or inclusion before import. Commit validates the mapping and refuses unsaved manuscript buffers or active writing challenges before creating staging data. A sanitized-fixture terminal integration test covers preview, mapping, import, series registration, imported emphasis, and source fingerprint preservation. The actual author-provided project remains covered by the separate read-only importer integration test described above; it is not copied into terminal regression fixtures or Git.

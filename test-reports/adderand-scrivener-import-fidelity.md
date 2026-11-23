# Adderand Scrivener import fidelity report

**Tested from commit:** `631c1b2`  
**Source fixture:** `test-fixtures/scrivener/Adderand.scriv` (excluded from Git)  
**Destination:** temporary project under `/tmp`; no source files were changed  
**Purpose:** verify real-project inventory, import, editing, reopen and EPUB compilation. This report contains no manuscript prose.

## Source identification and inventory

- The manifest reports format revision `2.0` and creator `SCRWIN-3.1.1.0` (Scrivener for Windows 3.1.1.0 creator metadata). The importer correctly warns that manifest revision alone does not establish broad application/version compatibility.
- The binder contains **5 items** in **3 root groups**: a draft folder with one text document, a research folder, and a trash folder with one text document.
- The project contains **14 files**. The only manuscript `content.rtf` is 55,366 bytes.
- The proposed plan maps the draft folder to a chapter and its text document to a scene, maps the research folder to a research record, and skips the trash folder and its child. The import report records 5 source items, 3 destination provenance records, 1 scene, 1 structural node, and 1 reading/research record.
- The import report status is `incomplete`: two binder items were explicitly skipped and the manifest compatibility warning remains. The parser reported no unsupported constructs.
- The imported project retains a byte-for-byte copy of the source project under `research/imported_sources/`.

## Manuscript and formatting checks

The importer extracted the RTF into 37,417 characters of Markdown. Before any test edit, the imported scene file exactly matched the Markdown produced by the Scrivener inventory parser: both were 37,417 characters with SHA-256 `0db9dc8cb6544dd01c02913a5c3366b5c9b7a0e23b58c707b61b8c04f52a4b1d`.

The RTF contains paragraph, line-break, bold, italic and Unicode controls. The importer reported no unsupported constructs, and the generated Markdown includes emphasis markup. This verifies fidelity between the parser's staging output and the imported manuscript file; it does **not** independently compare visual formatting against Scrivener's own renderer, so exact typography and any application-specific formatting remain unverified.

## Editing and publishing check

In the temporary imported project, a test marker was appended through the Writer's IDE save service. A separate CLI process reopened the project and found the marker through project search; `writers-ide inspect` reported `Project is valid.` The edited project compiled to EPUB. The EPUB passed ZIP integrity checks, contained `META-INF/container.xml` and an OPF package, and included the saved marker in its XHTML content. The generated EPUB was 19,574 bytes.

## Source integrity

SHA-256 checksums were recorded for every source file before and after inventory/import. All **14 source files remained byte-identical**. Import and editing were performed only in a separate temporary Writer's IDE project. Neither the original project nor its manuscript content was added to Git.

## Limits and follow-up

- This fixture verifies one Scrivener for Windows creator string and manifest revision. It does not establish compatibility with other Scrivener versions or platforms.
- The test project is temporary and was not packaged or committed. The report is the only committed artifact from the real manuscript test.
- The existing sanitized synthetic fixtures remain the automated regression basis; human review is still needed for visual formatting and any binder semantics not represented by this small fixture.

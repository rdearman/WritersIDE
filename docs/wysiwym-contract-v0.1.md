# WYSIWYM document and Compile IR contract (draft)

**Status:** Prototype contract for Issue #21; not frozen pending review against an author-provided formatted chapter.

## Authoring and storage

- Canonical prose remains UTF-8 Markdown in portable project files. The application must not replace it with raw LaTeX or a `.lyx` document.
- Project hierarchy, stable content-unit identity, character/plot metadata, and other structured facts remain outside the prose files.
- Plain prose remains in Markdown files; paragraphs are not stored as individual SQLite rows.
- Markdown parsing produces a rebuildable semantic projection. The original byte sequence remains authoritative and is returned unchanged by source serialization.
- The current parser prototype uses pulldown-cmark 0.9.6 with tables, footnotes, strikethrough, and task-list extensions enabled. Its syntax and extension set are provisional until a real chapter has been round-tripped and its unsupported constructs reported.

## Content units

Every composed document unit has a stable identifier, semantic kind, title, optional parent identifier, and ordered position in the Compile IR. Initial kinds are front matter, section body, chapter body, scene, and back matter. A fiction scene retains its existing UUID when moved or renamed. Non-fiction chapter prose can be represented directly as a chapter-body unit; it does not require fabricated scenes.

Groups (sections and chapters) and content units are separate typed objects. Composition order is explicit in the IR arrays; parent references express hierarchy and are validated for missing IDs, duplicate IDs, and cycles. A project may contain no scenes.

## Semantic projection

`SemanticDocument` stores the exact source string, SHA-256 fingerprint, projection schema version, and semantic nodes. Each node has a UTF-8 byte range and an optional parent node index. Current parsed semantics include headings, paragraphs, emphasis, strong emphasis, strikethrough, quotations, lists/items, code blocks, links, images, tables, footnotes/references, breaks, rules, task markers, and HTML. These are semantic observations; a target renderer chooses their presentation.

Typed embedded objects are planned as distinct semantic insets rather than arbitrary TeX: footnotes/endnotes, citations, figures and captions/alt text, tables, cross-references, index terms, and equations. Their canonical Markdown-compatible encoding and fallback syntax remain unresolved pending the real-chapter review. Unsupported constructs must remain in source and produce a loss/unsupported-content report during import or export; they must never be silently removed.

Source ranges are half-open UTF-8 byte offsets into the exact fingerprinted source. Editorial anchors and review comments must include stable content-unit identity, source fingerprint, quote/context, and ranges. Edits require unique reattachment; missing or ambiguous matches stay unresolved for author review.

## Compile IR

`CompileIr` is renderer-neutral and carries a schema version, project title, ordered structural groups, and ordered content units. A unit carries stable identity, semantic kind, optional parent, display title, and ordered source segments. A segment carries the source entity ID and a semantic document with its exact source fingerprint.

Renderers produce an output plus a source map. Each map entry relates an output byte range to the source entity ID, source fingerprint, and source byte range. Generated headings and separators may have no prose source range. Inclusion segments retain their actual referenced source IDs; they are not attributed to the including scene. A renderer must report unresolved references and omitted/unsupported content.

The current Markdown compiler passes through this IR and preserves its existing output. It does not yet render structural groups, semantic insets, or print layouts. The IR supports independent renderers for DOCX, EPUB3, HTML, and LuaLaTeX/PDF; no renderer may place page geometry into canonical prose.

## Versioning, invalidation, and migration

The parser projection is version 1 and Compile IR is version 1 in the prototype. Both are derived in memory and rebuildable; neither is stored as authoritative project data. Compile cache fingerprints include the IR representation and semantic schema version. Source coordinates are UTF-8 bytes, not grapheme indices or terminal-cell columns.

No SQLite migration is required for this prototype. Before persisting semantic insets, source-format IDs, or templates, validate the encoding against a real author chapter and define backward-compatible migration and loss-report behavior. Editing continues to use exact Markdown source and the existing durable write journal.

## Prototype verification and gate

Synthetic fixtures currently cover UTF-8 text, headings, paragraphs, emphasis, quotations, lists, code, tables/footnotes, source-range boundaries, exact source preservation, stable unit hierarchy, cycle/missing-parent validation, inclusion provenance, and output-to-source maps. These tests establish a prototype only.

The Issue #21 architecture gate remains open: run Markdown → semantic projection → Compile IR → Markdown against a real formatted chapter, compare byte and semantic fidelity, enumerate unsupported formatting/insets, and revise this contract before freezing the canonical syntax or storage schema. The repository currently contains no author-provided manuscript fixture.

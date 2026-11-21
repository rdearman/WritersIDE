# WYSIWYM document and Compile IR contract

**Status:** Foundation contract for Issue #21, validated against the locally supplied real Scrivener manuscript after read-only RTF extraction. Core Markdown and Compile IR boundaries are fixed for this implementation; specialized semantic-object encodings remain deliberately open until real authoring demonstrates a need.

## Authoring and storage

- Canonical prose remains UTF-8 Markdown in portable project files. The application must not replace it with raw LaTeX or a `.lyx` document.
- Project hierarchy, stable content-unit identity, character/plot metadata, and other structured facts remain outside the prose files.
- Plain prose remains in Markdown files; paragraphs are not stored as individual SQLite rows.
- Markdown parsing produces a rebuildable semantic projection. The original byte sequence remains authoritative and is returned unchanged by source serialization.
- The parser uses pulldown-cmark 0.9.6 with CommonMark plus tables, footnotes, strikethrough, and task-list extensions. The parser version and enabled extensions are part of projection schema version 1. A real imported manuscript exercises paragraphs, emphasis, Unicode source ranges, exact Markdown retention, Compile IR rendering and source mapping; this validates post-RTF Markdown semantics, not Scrivener RTF conversion fidelity. New parser semantics require a schema-version change and regression coverage.

## Content units

Every composed document unit has a stable identifier, semantic kind, title, optional parent identifier, and ordered position in the Compile IR. Initial kinds are front matter, section body, chapter body, scene, and back matter. A fiction scene retains its existing UUID when moved or renamed. Non-fiction chapter prose can be represented directly as a chapter-body unit; it does not require fabricated scenes.

Groups (sections and chapters) and content units are separate typed objects. Composition order is explicit in the IR arrays; parent references express hierarchy and are validated for missing IDs, duplicate IDs, and cycles. A project may contain no scenes.

## Semantic projection

`SemanticDocument` stores the exact source string, SHA-256 fingerprint, projection schema version, and semantic nodes. Each node has a UTF-8 byte range and an optional parent node index. Current parsed semantics include headings, paragraphs, emphasis, strong emphasis, strikethrough, quotations, lists/items, code blocks, links, images, tables, footnotes/references, breaks, rules, task markers, and HTML. These are semantic observations; a target renderer chooses their presentation.

The model distinguishes semantic observations from typed embedded objects. Markdown-native headings, paragraphs, emphasis, quotations, lists, code, links, images, tables and footnotes are parsed as typed `SemanticKind` nodes. Import-only constructs not representable by the canonical Markdown subset must be retained in source/provenance and listed in the import loss report. Future typed insets (endnotes, citations, figures/captions, cross-references, index terms and equations) must use stable IDs where identity matters, a typed payload, source range, portable Markdown fallback and an explicit renderer loss diagnostic. Their canonical syntax and persistence schema are not frozen without a real authoring example; raw arbitrary TeX is not a fallback. No parsed Markdown bytes may be silently rewritten or dropped.

Source ranges are half-open UTF-8 byte offsets into the exact fingerprinted source. Editorial anchors and review comments must include stable content-unit identity, source fingerprint, quote/context, and ranges. Edits require unique reattachment; missing or ambiguous matches stay unresolved for author review.

## Compile IR

`CompileIr` is renderer-neutral and carries a schema version, project title, ordered structural groups, and ordered content units. A unit carries stable identity, semantic kind, optional parent, display title, and ordered source segments. A segment carries the source entity ID and a semantic document with its exact source fingerprint.

Renderers produce an output plus a source map. Each map entry relates an output byte range to the source entity ID, source fingerprint, and source byte range. Generated headings and separators may have no prose source range. Inclusion segments retain their actual referenced source IDs; they are not attributed to the including scene. A renderer must report unresolved references and omitted/unsupported content.

The current Markdown compiler renders structural headings and source segments through this IR. Semantic insets without a supported target representation must be diagnosed, not silently omitted. Independent renderers include Markdown/plain text and local Pandoc-backed DOCX, EPUB3, HTML, direct TeX, and LuaLaTeX/PDF; no renderer may place page geometry into canonical prose. Source maps refer to UTF-8 bytes in the canonical Compile IR stream, not final page coordinates.

## Requirement contract and traceability

| Requirement | Contract | Implementation evidence | Allocation/status |
|---|---|---|---|
| REQ-WYS-001 Canonical source | Project prose is UTF-8 Markdown in portable content-unit files; no raw TeX or `.lyx` canonical files. | Existing scene/content-unit files and journal-backed save APIs. | Foundation implemented; inherited from Phase 1 and #20. |
| REQ-WYS-002 Semantic projection | Parse source into a versioned, rebuildable tree with stable source fingerprint and UTF-8 byte ranges; retain exact source bytes. | `SemanticDocument::{parse, validate_ranges, to_markdown}`; synthetic tests and conditional real-manuscript test. | Foundation implemented; no database migration. |
| REQ-WYS-003 Composition | Compile units/groups have stable IDs, explicit parent/order, type and source segment identity; scene UUIDs are unchanged. | `CompileIr::{validate, render_markdown}` and `story_pipeline::compile_ir`. | Foundation implemented; #20 content-unit storage supplies chapter prose without scenes. |
| REQ-WYS-004 Source mapping | Renderer output ranges map to source entity ID, source fingerprint and source UTF-8 range. | `CompileArtifact::source_map`; real-manuscript integration test checks stable ID/hash and range bounds. | Foundation implemented; visual page-coordinate maps remain deferred. |
| REQ-WYS-005 Typed embedded semantics | Supported Markdown constructs have typed semantic nodes. Future insets have typed payloads/stable IDs where needed, portable fallbacks and explicit renderer diagnostics. | `SemanticKind` covers the current Markdown subset; no persistent inset schema exists. | Partial; new inset encodings wait for concrete authoring examples. |
| REQ-WYS-006 Loss accounting | Import/render targets must report unsupported or omitted formatting instead of silently dropping it. | Import staging records unsupported formatting and fidelity warnings; renderer diagnostics cover publication checks. | Partial; target-specific semantic inset loss reports remain #6/#7/#21 work. |

API/storage boundary: `story-document` owns renderer-neutral parse/IR types and does not depend on SQLite or terminal UI; `story-pipeline::compile_ir` composes current application input; `WriterApp::compile_artifact` renders through the selected target. Semantic trees and Compile IR are derived in memory and cached only by versioned input fingerprints. Stable content IDs, canonical Markdown hashes and the existing Phase 1 write journal remain authoritative. Thus parser evolution does not require a SQLite migration unless a future inset or new author-authored metadata becomes persisted.

Traceability: #1 defines stable links, anchors, source evidence and non-mutating compilation; #2 supplies assemblies, editor sessions and passage review; #7 owns target compilation; #19 owns DOCX review/source revision mapping; #20 owns fiction/nonfiction content units. The present milestone freezes only canonical source, current parser subset, Compile IR ordering/identity and byte-map conventions. Later publication renderer work remains allocated to #7/#21.

## Versioning, invalidation, and migration

The parser projection is version 1 and Compile IR is version 3. Both are derived in memory and rebuildable; neither is stored as authoritative project data. Compile IR version changes are explicit and renderer/cache keys include the version. Compile cache fingerprints include the IR representation and semantic schema version. Source coordinates are UTF-8 bytes, not grapheme indices or terminal-cell columns.

No SQLite migration is required for this prototype. Before persisting semantic insets, source-format IDs, or templates, validate the encoding against a real author chapter and define backward-compatible migration and loss-report behavior. Editing continues to use exact Markdown source and the existing durable write journal.

## Prototype verification and gate

Synthetic fixtures cover UTF-8 text, headings, paragraphs, emphasis, quotations, lists, code, tables/footnotes, source-range boundaries, exact source preservation, stable unit hierarchy, cycle/missing-parent validation, inclusion provenance, and output-to-source maps. A conditional local integration test additionally runs the imported author-provided Scrivener manuscript through the semantic projection and real application Compile IR, checking exact source bytes, paragraph/strong/emphasis nodes, Unicode ranges and stable source-map identity. The source project is excluded from Git and no manuscript text is embedded in tests or documentation.

The core architecture gate is met for the supported Markdown subset: Markdown → semantic projection → application Compile IR → Markdown renderer. This does not establish fidelity of the preceding RTF-to-Markdown conversion, validate specialized insets absent from the sample, or complete the publication renderer acceptance tests. Those remain explicit Issue #21 work; final author playtesting is still required before freezing any new inset syntax or persistence schema.

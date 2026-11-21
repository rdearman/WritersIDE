# ADR-0001: WYSIWYM authoring, fiction/non-fiction documents and professional publishing

Status: **Accepted, 21 November 2026**. Supersedes contradictory requirements in the original SRS v0.1 and engineering design v0.1. Full normative requirements and acceptance criteria: [#19](https://github.com/rdearman/WritersIDE/issues/19), [#20](https://github.com/rdearman/WritersIDE/issues/20), [#21](https://github.com/rdearman/WritersIDE/issues/21).

## Decision
1. Writer's IDE uses LyX-inspired **What You See Is What You Mean** principles, **not** LyX's native file format or a LyX clone.
2. Canonical manuscript prose is readable, Git-friendly UTF-8 **Markdown** in stable-ID content units, supplemented by typed semantic objects/metadata when Markdown cannot faithfully express meaning. **Raw LaTeX is not canonical prose.**
3. Fiction may use chapters with scenes; non-fiction may use **chapters with prose directly** and optional nested sections, with **no mandatory or fabricated scenes**. Design a backwards-compatible general document/content-unit layer preserving existing scene UUIDs and constraints.
4. Shared semantic document model distinguishes prose, structure, embedded objects and presentation. It feeds a source-mapped Compile IR and independent DOCX, EPUB3, HTML and LaTeX/PDF renderers.
5. **LuaLaTeX** is the preferred professional print typesetting backend; XeLaTeX may be optional. Generated .tex and logs are inspectable artefacts. Fonts, trim, page layout, chapter styling and other print design live in **book-design profiles**, separate from project templates and prose.
6. **Project templates** supply initial document structure, metadata and defaults; built-in fiction/non-fiction and user-created templates are supported without locking an existing book to its original template.
7. External reviewers use normal **DOCX Track Changes and comments**. Writer's IDE generates a versioned review copy and imports feedback against base/ours/theirs; the author accepts/rejects/defers changes in the **terminal**. The browser never edits canonical prose.
8. Terminal performs **all writing and planning**; optional localhost browser performs planning, read-only compiled document review and non-prose annotation management.
9. No mandatory planning fields, unsolicited subjective critique, cloud service or real-time collaborative editing.
10. **Feature freeze:** after reconciling and implementing the agreed backlog, the author will write a real book. Do not add speculative new features until actual writing exposes a problem. Correctness, recovery and usability blockers are exceptions.

## Architectural contracts to settle before implementation
- General content-unit hierarchy, direct chapter prose and ordering/mixed-content semantics.
- Markdown subset and semantic object encoding, parser/serializer round-trip and explicit loss reporting.
- Source anchors, stable IDs, split/merge lineage, revision identity and Compile source maps.
- DOCX review manifest, OOXML tracked-change extraction and three-way merge.
- Document class, book-design profile and renderer/toolchain boundaries.
- Migrations preserving Phase 1 scenes, SQLite integrity, journal and Git history.

## Traceability / supersessions
- Original SRS 'scene is fundamental unit': remains true for **fiction scene workflows**, not a universal prohibition on non-fiction chapter-level prose. #20 and #21 supersede it where necessary.
- Original SRS 'future desktop frontend': current approved interface contract is **terminal-only canonical prose; optional localhost browser for planning/read-only review** (#17).
- Issue #2 exclusion of copyeditor workflows: superseded for **asynchronous DOCX review round-trip** by #19. Real-time multiuser writing remains excluded.
- Issue #7 Compile pipeline: extended by #21 with semantic IR, LuaLaTeX backend, publication styles and source mapping.
- Issue #18 author-led playtesting: remains the acceptance gate for future feature proposals.

## Implementation and validation
Do not change foundational schema during active Codex work without an explicit migration plan. Prototype a formatted chapter and DOCX review round-trip before freezing semantic storage and Compile IR. Test fiction, non-fiction without scenes, Unicode, citations/notes, LaTeX PDF, EPUB, DOCX tracked-change conflict resolution, backup and restore. Maintain an SRS → issue → code → automated test → author playtest traceability matrix.

This ADR is a decision addendum; it does not claim that the old standalone SRS/design documents have been rewritten in full.

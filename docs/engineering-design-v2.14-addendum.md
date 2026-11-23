# Engineering Design v2.14 Addendum — Author Workflow Entry Points

**Status:** Implemented engineering slice; human acceptance outstanding  
**Date:** 2026-11-23

This addendum records the project-start and chapter-outline integration completed after the initial workflow audit. It extends the existing story model and does not change the Phase 1 persistence architecture.

## No-argument startup

The terminal binary continues to accept an explicit project path. When none is provided, it creates a disposable startup project beneath the operating system temporary directory and opens the existing New Project file browser. The temporary directory guard remains alive for the application lifetime and removes the starter project on exit. This project is omitted from recent-project history. No background project is created in the user's home directory. New Project continues to create a separate destination and explicitly asks for Fiction or Non-fiction.

## Book-type workflow

- Fiction selection opens Scene Cards after project creation. Insert creates a titled, unplaced scene; its UUID remains application-internal.
- Non-fiction selection opens Outline with no generated scene or content unit.
- The outline renders structural nodes in position order. Sections are navigable/collapsible parent rows, and chapters appear beneath them. Chapter and section direct-prose units remain content units rather than scenes.
- Insert → New Chapter in Section captures the selected section from the current outline context and asks only for a title. The database API remains unchanged and receives a stable `NodeId` internally.
- Edit → Write Selected Chapter opens the selected chapter's existing `ChapterBody`, or creates one through the existing content-unit service, then uses the same journal-backed editor and save path as scenes.
- A chapter without direct prose remains a structural heading. No synthetic prose document is created merely by selecting a chapter.

## Contracts and failure handling

- New Project refuses creation until a book type is selected and retains the existing two-step overwrite/create confirmation.
- Chapter prose creation is blocked while another editor buffer is dirty.
- Nested chapter creation requires a selected section; failure leaves structure unchanged and produces a concise message.
- Reordering, manuscript persistence and compilation continue to use existing application services and transactions.
- Markdown compilation includes chapter-level prose and excludes absent scene objects naturally.

## Verification added

- No-argument startup creates a project in the system temporary directory and removes it when its guard is dropped.
- Explicit project paths bypass temporary startup creation.
- Compact file-browser test checks that the book-type choice and actions are visible.
- Fiction project creation opens Scene Cards; sixty unique scenes can be created through Insert without menu navigation.
- Non-fiction project creation opens Outline without scenes.
- Section/chapter outline selection, UUID-free nested chapter creation, direct chapter prose editing, journalled save, and Markdown compilation are covered by terminal tests.
- A dedicated author-workflow integration suite exercises 60-scene fiction persistence, ordering, exclusion and compilation; non-fiction sections, direct prose, reorder and Markdown/DOCX/EPUB compilation; and sanitized Scrivener import, continuation, reopen, compile and source-fingerprint preservation.

## Limitations and remaining workflow work

The current change establishes author-facing entry points and integration tests; it does not complete the full UAT package. Non-fiction section/chapter keyboard reordering, richer hierarchy commands, PTY-driven end-to-end journeys, timed challenge integration into the fiction journey, PDF acceptance, and opening compiled files in a desktop reader remain to be completed or explicitly verified. Human usability must still be evaluated in a frozen build.

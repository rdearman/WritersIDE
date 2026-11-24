# Engineering Design v2.26 Addendum — Recoverable Subtree Deletion Acceptance

This addendum extends the populated-heading removal contracts in v2.24 and v2.25.

## Contract

- Deleting a populated chapter is an explicit choice in the removal dialogue; it is distinct from moving scenes to Unplaced or rehoming them to another chapter.
- The dialogue previews that removed scenes and chapter prose will be omitted from Compile and can be restored.
- The subtree is retained in project recovery storage with its existing identities and prose.
- Restore after closing and reopening the project reinstates the chapter, scene placement, chapter-owned prose, and Compile output.

## Verification

A Linux PTY test selects the recoverable subtree-deletion choice through the real Navigate → Outline → Delete workflow. It reopens the project, confirms the removed scene and chapter prose are absent from normal Compile output but retained in recovery, restores the removal, and verifies the original scene identity, placement, and text.

All PTY tests in this integration-test binary now share a mutex because concurrently running full-screen terminal subprocesses could make key-event timing nondeterministic. This serialises only PTY subprocess tests; application code and other test suites are unaffected.

Automated results support implementation confidence but do not constitute human usability acceptance. Issue #63 remains open for UAT and the remaining lifecycle matrix, including nested non-fiction subtree deletion and dependency/reference checks.

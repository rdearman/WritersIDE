# Engineering Design v2.23 Addendum — PTY Removal Dialog Coverage

Date: 24 November 2026  
Status: Automated PTY acceptance added; human verification remains open under #63.

## Context

This addendum extends [Engineering Design v2.22](engineering-design-v2.22-addendum.md). It adds integration coverage for the existing recoverable removal interactions; it does not change the removal policy or persistence schema.

## Terminal interaction contract

- From the normal Navigate menu, an author can reach Scene Cards and invoke Delete on the selected scene. The visible confirmation names the scene and exposes both confirmation and cancellation keys.
- From the same menu, an author can reach the Reference Library and invoke Delete on the selected imported reference. The confirmation identifies the reference and states that the source file and notes remain preserved.
- Cancelling either dialogue leaves the scene, reference record and source file intact. The test reopens the project and verifies scene prose and reference identity remain present.
- The application is launched and controlled through a Linux PTY at 80×24. The test follows the visible menu route rather than invoking removal service methods directly.

## Verification

- `cargo fmt --all -- --check` passed.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` passed.
- `cargo test --offline -p writers-ide-terminal` passed all 298 unit tests, 3 author-workflow tests and, on the final PTY rerun, all 4 PTY tests.
- The PTY harness matches visible text fragments independently because terminal cursor-position updates may separate adjacent words in the emitted byte stream.
- This validates rendering and cancellation dispatch only. Confirmation execution, broader recovery behavior and human usability remain covered by their respective tests and UAT.

## Remaining work

The broader #63 lifecycle matrix remains open, including complete before/after Compile previews, heading-owned prose destination choices, book/project deletion policy and human UAT. This automated test does not close #63.

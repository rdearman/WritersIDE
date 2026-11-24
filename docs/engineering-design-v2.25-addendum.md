# Engineering Design v2.25 Addendum — Selected Chapter Rehoming Acceptance

This addendum extends the recoverable-removal contracts in v2.21–v2.24. It records verified terminal coverage for choosing a destination chapter when removing a populated chapter.

## Contract

- The populated-chapter removal dialogue offers moving scenes to Unplaced, moving them to a selected compatible chapter, deleting the full hierarchy recoverably, and cancelling.
- Choosing another chapter previews the destination and the effect on Compile order before confirmation.
- Rehoming preserves scene identity and prose, uses the destination chapter's existing order, and remains recoverable to the original chapter.
- The operation does not modify the source manuscript beyond the requested structural change.

## Verification

A Linux PTY integration test drives the actual terminal menu and removal dialogue, selects a destination chapter, then reopens the project and checks the scene's destination, identity-linked text and compiled output. It restores the removal and verifies that the scene returns to its original chapter without changing its prose.

The test complements the default “Move scenes to Unplaced” workflow and its recovery checks documented in v2.24. Passing automated coverage does not replace human usability acceptance; issue #63 remains open for UAT and for the remaining lifecycle matrix, including populated subtree deletion and non-fiction heading cases.

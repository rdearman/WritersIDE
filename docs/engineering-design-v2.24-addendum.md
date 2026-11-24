# Engineering Design v2.24 Addendum — Populated Chapter Removal Acceptance

Date: 24 November 2026  
Status: Automated PTY workflow added; human verification remains open under #63.

## Approved removal behavior

This addendum records executable acceptance of the populated-heading policy in [Engineering Design v2.21](engineering-design-v2.21-addendum.md). A populated chapter is removed through the Outline's Delete action. Its confirmation presents the default `Move scenes to Unplaced` choice, alternatives for another chapter or recoverable subtree removal, descendant and word counts, and effects on Compile. Direct chapter prose is counted and explicitly identified as retained in Removed Documents for the default scene-rehoming action.

The default action preserves stable scene and prose document identities and their text. Unplaced scenes and removed prose are omitted from Compile. The operation can be restored after closing and reopening the project, returning the scene to its original chapter and making the prose active again.

## Verification

- A Linux PTY at 80×24 invokes the real Navigate → Outline → Delete route, inspects the populated-chapter confirmation, and confirms the default action.
- After reopening, the test verifies the scene is unplaced and retains its text; the direct chapter-prose document is recoverable and retains its text; both are omitted from Compile.
- The test restores the structural removal through the application service and verifies the original scene placement and active chapter prose return without content loss.
- Dialog output is matched by independently rendered fragments; flattening ANSI output is not used as evidence that cursor-positioned glyphs are contiguous.
- Full terminal checks, formatting, and workspace Clippy are recorded with the commit that adds this acceptance test.

## Remaining work

The operation-level tests do not establish human discoverability or usability. #63 remains open for UAT and its wider lifecycle matrix, including other removal choices, complete Compile-order previews, and broader object lifecycle handling.

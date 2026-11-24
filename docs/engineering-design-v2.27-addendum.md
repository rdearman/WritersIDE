# Engineering Design v2.27 Addendum — Non-fiction Subtree Removal Acceptance

This addendum extends the chapter-removal and recoverable-subtree contracts in v2.24–v2.26 to chapter-oriented non-fiction.

## Contract

- Removing a populated non-fiction section offers promotion, movement to another compatible section, or recoverable deletion of the whole subtree.
- The removal dialogue distinguishes the heading’s own prose and previews the Compile consequence.
- Recoverable deletion removes the heading subtree and its prose from active content without destroying their identities or files.
- Restoring the root after reopening restores the original child hierarchy and prose.

## Verification

A Linux PTY integration test drives Navigate → Outline → Delete for a non-fiction section that contains a nested subsection and section-owned prose. It selects subtree deletion, confirms the archived root and prose are omitted from Compile, then restores the root and verifies the child’s original parent and the prose text.

The test complements unit coverage for promotion and rehoming. The terminal PTY integration suite serialises its subprocesses to avoid input timing interference. Human UAT and the remaining lifecycle dependency cases are still required; issue #63 remains open.

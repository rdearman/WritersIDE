# Engineering Design v2.32 Addendum — Non-fiction Section Prose Rehoming

Issue #63's populated-heading removal policy now covers directly owned prose
under non-fiction sections. When a compatible active destination section exists,
the default action promotes child headings to the removed heading's parent and
moves direct prose into the selected active section. The dialog reports the
direct prose document count and word count, destination, and Compile effect.
The author can instead move the prose to Removed Prose Documents or remove the
whole subtree recoverably.

Migration 0051 removes the one-SectionBody-per-section index. Multiple prose
documents can therefore be retained in one destination section; their integer
positions define deterministic order. The transaction records each content
unit's original and applied parent/position, so restoration can return stable
IDs, text, metadata, and order. Restoration rejects a conflicting move rather
than overwriting it. Chapters and scenes retain their existing independent
placement model.

The application service validates the destination as an active section outside
the removed subtree. The terminal removal dialog defaults to a compatible parent
section where possible, allows another compatible destination to be selected,
and explicitly previews whether prose remains in Compile. Regression coverage
includes the service, direct schema migration from version 50, close/reopen,
ordering, Compile output, restoration, and a PTY workflow at the author-facing
dialog.

This is implementation coverage, not human acceptance. Issues #63 and #20 remain
open for human UAT of the heading-removal and non-fiction chapter workflows.

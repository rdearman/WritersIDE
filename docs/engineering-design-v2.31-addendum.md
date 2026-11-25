# Engineering Design v2.31 Addendum — Chapter-Owned Prose During Removal

Issue #63's approved populated-heading policy now applies an explicit retention
choice to direct fiction chapter prose. When another active chapter is
available, the default removes the heading while keeping its prose active in
that chapter. The author may press **P** to move the prose to Removed Documents
instead. Both choices preserve its stable content-unit ID and manuscript file.

Migration 0050 records original and applied prose parent/position on the
structural-removal record. It also permits multiple chapter-body units under a
chapter, with sibling positions remaining unique. Removal and scene placement
changes run in one SQLite transaction. Restore checks that rehomed prose is
still at its recorded applied parent and position, then returns it to its
original parent and order; a conflicting move fails without partial restoration.

Coverage verifies active prose remains in Compile after removal, stable identity
and text survive close/reopen, restoration returns the original placement, the
alternate recoverable-removal choice works through the dialog, and migration
from historical schemas still succeeds. The existing selected-chapter PTY
workflow remains covered. Non-fiction section-owned prose continues to remain
recoverable with its removed section; its separate rehoming destination is not
implemented by this change. Issue #63 remains open for lifecycle coverage and
human UAT.

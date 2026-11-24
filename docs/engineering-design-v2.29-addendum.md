# Engineering Design v2.29 Addendum — Scene Relationship Preservation on Removal

This addendum tightens the recovery evidence for populated chapter subtree deletion.

The terminal workflow fixture now links its scene to a character, plotline, and story event before removal. After recoverable deletion, project reopen, and restore, the test verifies all three relationships still resolve to the same entities, alongside scene identity, chapter placement, chapter-owned prose, and Compile output.

The removal operation archives the subtree and its scene participation without deleting relationship records. This test protects that lifecycle contract from regressions. It does not cover every dependency type (for example annotations, passage links, snapshots, and external asset references); those remain in the #63 lifecycle verification matrix. Human UAT is still required.

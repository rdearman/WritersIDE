# Engineering Design v2.30 Addendum — Cross-reference and Snapshot Retention

This addendum extends the chapter removal preservation tests to stable passage references and scene history.

The populated chapter subtree removal fixture now includes a scene snapshot, a stable passage anchor, and a story link from that anchored passage to a character. After recoverable deletion, project reopen, and restoration, the PTY integration test verifies the snapshot ID, anchor ID, and outgoing link are unchanged, in addition to the scene's character, plotline, and event associations.

These checks confirm that structural removal archives scene placement without deleting stable references or snapshot records. They do not cover every possible annotation or external asset dependency; #63 remains open for those lifecycle cases and human UAT.

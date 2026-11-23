# Engineering Design v2.01 Addendum — File browser focus and import feedback

Issues #52 and #53 reported that Scrivener import appeared to do nothing in
the shared file browser. The browser now starts with the directory tree
focused and shows which pane is active. Tab moves from the tree to folder
contents and then to the name field; Shift+Tab moves in reverse. The dialog
displays the selected source path and a dedicated, width-bounded status line
for import outcomes and actionable errors. Enter in the filename
field attempts preview of the selected Scrivener item, so a user who moved
focus one pane too far is not left with an inert dialog. F5 remains handled
by the file browser before workspace shortcuts.

Regression tests cover F5 while a conflicting F5 workspace binding exists,
Enter from the filename field, tab order, 80×24 visibility, and an invalid
selection whose error remains visible inside the dialog.

The supplied mounted project at
`/home/vmspace/vmsambashare/Scrivener_Archive/AmericanFlatbow.scriv` was
inspected read-only with the CLI. Its manifest parses as revision 2.0 and
reports creator `SCRWIN-3.1.5.1`, 45 binder items, zero proposed manuscript
items, 13 research items, and one skipped item. The preview reports unresolved
source IDs and missing folder-group `content.rtf`; therefore this inspection
is not evidence of a complete manuscript import. The source was not modified.
The provided project still needs human Alpha UAT through the mounted path;
automated handler tests cannot establish terminal key delivery or import
fidelity for that incomplete source.

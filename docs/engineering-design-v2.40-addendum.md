# Engineering Design v2.40 Addendum — New Project and Outline Navigation

## Approved workflow correction

The New Project browser now exposes project title, project folder, book type,
Create Project and Cancel as keyboard-focusable controls. Tab and Shift+Tab
navigate the controls; Enter advances from title/folder, selects the book type,
creates the project or cancels according to focus. F2 remains an optional type
selector. A title change updates the folder name until the author edits the
folder field, after which the folder name is independent. The selected parent,
folder name and resolved destination are rendered before creation. New project
creation has no redundant second confirmation because it refuses an existing
destination; it reports the created project and opens the appropriate fiction
or non-fiction workspace.

## Scene Cards hierarchy

Included scene cards now render structural nodes in manuscript outline order,
including section labels and indented chapter boundaries. Empty sections and
chapters remain visible in Included; Excluded only shows structural headings
with excluded scene content. Section labels are display context; chapter
boundaries remain the selectable card-group operation. Chapter placement,
scene identity and compilation ordering continue to use the existing relational
model.

## Verification contract

- Exercise project creation through Enter-controlled actions in unit and Linux
  PTY tests.
- Verify that fiction creation opens Scene Cards and non-fiction creation opens
  the chapter outline.
- Verify title/folder independence, destination collision rejection and Cancel.
- Verify nested section/chapter order and empty chapter visibility in the card
  model.
- Keep #39, #55 and #59 open for author UAT; automated coverage does not establish
  discoverability or usability.

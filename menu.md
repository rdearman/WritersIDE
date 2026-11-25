# Writer’s IDE menu options

Open a menu with **Ctrl+/** or **F10**. Use **Left/Right** to change top-level menus, **Up/Down** to choose an action, **Enter** to run it, and **Esc** to close. Menus are direct lists: Enter runs the selected action. The command palette remains available for less frequent commands.

Choose **Search All Commands…** from File, Edit, Insert, Navigate, View, Review, or Publish. Tools opens the command search directly. Help contains **Search Commands**. Registered command IDs, shortcuts, macros and keybindings remain available.

The complete command inventory and before/after placement audit is in [docs/menu-audit-v0.1.md](docs/menu-audit-v0.1.md). Command-use frequency is not measured; the audit’s visibility tiers are qualitative.

On a normal launch without a project argument, the start screen offers **New Project**, **Open Project**, **Recent Projects**, **Import Scrivener Project**, **Guided Tour**, and **Quick Start**. Choosing New Project opens the shared browser with Fiction/Non-fiction selection; it does not open a raw path prompt first. Passing a project path still opens that project directly.

## File

- New Project
- Open Project
- Recent Projects
- Close Project
- Import Scrivener as New Project…
- Import Scrivener Book into Series
- Open Text File…
- Import Document…
- Save Project
- Save Project As…
- Quit
- Search All Commands…

When the Series Books workspace is active, File also offers **Remove Selected Book from Series…** and **Show Removed Books**. The Removed Books view offers **Restore Selected Book to Series**. These contextual actions are hidden elsewhere so the general File menu stays compact.

## Edit

- Undo (`Ctrl+Z`)
- Redo (`Ctrl+Y` / `Ctrl+Shift+Z`)
- Cut Selection (`Ctrl+X`)
- Copy Selection (`Ctrl+C`)
- Paste (`Ctrl+V`)
- Delete Selection
- Select All (`Ctrl+A`)
- Select Word (at or immediately after the cursor)
- Select Paragraph (the non-empty paragraph containing the cursor)
- Find Next (`F3`)
- Find Previous (`Shift+F3`)
- Find in Manuscript (`Ctrl+F`)
- Find and Replace (`Ctrl+H`)
- Indent Paragraph
- Unindent Paragraph
- Reflow Paragraph
- UPPERCASE
- lowercase
- Title Case
- Bold Selection
- Italic Selection
- Rename Scene
- Set Scene Status
- Move Scene to Chapter…
- Move Scene Between Included and Excluded
- Comment on Selection
- Move Outline Item Up
- Move Outline Item Down
- Search All Commands…

## Insert

- New Section
- New Chapter
- New Scene
- Create Direct Prose Unit
- Write Selected Chapter
- Insert Date/Time
- Insert File…
- Create Story Link
- Remove Story Link
- Insert Snippet…
- Search All Commands…

## Navigate

- Manuscript
- Outline
- Scene Cards
- Reference
- Open Scene
- Return to Previous Position
- Search All Commands…

## View

- Switch View… (manuscript editor)
- Show/Hide Formatting Marks
- Open Outline Reference (Manuscript editor)
- Open Timeline Reference (Manuscript editor)
- Open Compile Diagnostics Reference (Manuscript editor)
- Split Side by Side (Manuscript editor)
- Split Top and Bottom (Manuscript editor)
- Grow Focused Window / Shrink Focused Window (when a divider is available)
- Next Window / Previous Window (when multiple windows are open)
- Close Window (when multiple windows are open)
- Single View (when multiple windows are open)
- Hide/Show Reference Panes (when reference panes are open)
- Follow Scene Card (when writing a scene)
- Pin Reference Pane (when a reference pane has focus)
- Next/Previous Window (when multiple reference panes are open)
- Move Reference Earlier/Later (when multiple reference panes are open)
- Close Window (when a reference pane is open)
- Return to Writing (when a reference pane has focus)
- Maximize Current Window / Restore Windows (one reversible toggle)
- Save Layout… / Load Layout… (Manuscript editor; names are stored in the current project)
- Swap Editor and Reference Sides (when a reference pane is open)
- Show Rendered Markdown (Reference workspace)
- Search All Commands…

View actions are contextual: editor/reference-pane commands appear only in the
manuscript editor and only when their required pane or scene exists. Split
windows may be nested; focus, close, Single View, maximize and restore operate
on the focused pane. Each pane retains a stable document source, and the pane
tree and focus restore with the project session. Formatting marks are available
while editing; rendered Markdown is available in the Reference workspace.
Named layouts preserve the recursive split tree and stable source keys in
`.writers-ide/window-layouts.json`. Load Layout remaps pane IDs to the matching
views currently open in the project; it reports a missing view without changing
the active layout.
F11 is currently assigned to Find Query History, so maximize/restore remains
available from View and its command can be remapped in Keybinding Manager.

## Review

- Resolve Save Conflict
- New Writing Issue
- List Reading Annotations
- List Reading Bookmarks
- Show Revision History
- Compare Snapshot
- Create Snapshot
- Search All Commands…

## Publish

- Compile
- Compile / Export…
- Compile Results
- Compile Configuration
- Compile Assembly
- Preview Latest Compile Output
- Preview Document Inclusions
- Search All Commands…

## Tools

- Search All Commands…
- New Research Note
- Keybinding Manager
- Editor Preferences
- Project Text Expansion Settings
- Personal Text Expansion Settings
- Manage Snippets…
- Quick Capture
- Writing Statistics

## Help

- Start / Resume / Restart Tutorial
- Context Help
- Search Commands

## Scene (Scene Cards only)

- New Scene
- Write Selected Scene
- Rename Selected Scene
- Edit Scene Synopsis
- Move Scene Up / Move Scene Down
- Move Scene to Chapter
- Include / Exclude Scene
- Search All Commands…

The Scene menu appears only while Scene Cards is active; Publish is available
from manuscript workspaces and command search. The Scene Cards footer shows the
effective Include / Exclude shortcut (Space by default).
Use Up/Down to select a card and Left/Right to reorder it, including across a
chapter boundary. Tab switches between Included and Excluded. Shift+Left and
Shift+Right move the selected scene's chapter block while preserving its scenes.
Insert creates a scene; Shift+Insert creates a chapter at the selected scene.
The Scene menu also exposes Move Scene Earlier and Move Scene Later. Move Scene
to Chapter opens the chapter chooser; Up/Down chooses the destination, Enter
moves, and Esc cancels. Unplaced scenes can be reordered before chapter
assignment and remain outside the compiled manuscript until placed.

On a temporary start project, **Save Project** opens the Save Project As
dialogue. The footer shows **[TEMPORARY]** until a permanent project folder has
been selected and validated. Save Project As creates a separate copy and never
overwrites an existing folder.

## Notes

- Disabled actions explain why they are unavailable. Availability depends on project and selection context.
- **Insert Snippet…** opens a chooser for project and personal snippets.
- Menu labels are concise; longer descriptions stay in command search and contextual help.
- All commands retain their registered IDs, keyboard shortcuts, and macro/keymap access.

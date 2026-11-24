# Writer’s IDE menu options

Open a menu with **Ctrl+/** or **F10**. Use **Left/Right** to change top-level menus, **Up/Down** to choose an action, **Enter** to run it, and **Esc** to close. Menus are direct lists: Enter runs the selected action. The command palette remains available for less frequent commands.

Choose **Search All Commands…** from File, Edit, Insert, Navigate, View, Review, or Publish. Tools opens the command search directly. Help contains **Search Commands**. Registered command IDs, shortcuts, macros and keybindings remain available.

The complete command inventory and before/after placement audit is in [docs/menu-audit-v0.1.md](docs/menu-audit-v0.1.md). Command-use frequency is not measured; the audit’s visibility tiers are qualitative.

On a normal launch without a project argument, the start screen offers **New Project**, **Open Project**, **Recent Projects**, **Import Scrivener Project**, **Guided Tour**, and **Quick Start**. Choosing New Project opens the shared browser with Fiction/Non-fiction selection; it does not open a raw path prompt first. Passing a project path still opens that project directly.

## File

- New Project
- Open Project
- Recent Projects
- Import Scrivener as New Project…
- Import Scrivener Book into Series
- Open Text File…
- Import Document…
- Save Project
- Save Project As…
- Quit
- Search All Commands…

## Edit

- Find in Manuscript
- Find and Replace
- Rename Scene
- Set Scene Status
- Move Scene
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

- Show/Hide Formatting Marks
- Search All Commands…

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

On a temporary start project, **Save Project** opens the Save Project As
dialogue. The footer shows **[TEMPORARY]** until a permanent project folder has
been selected and validated. Save Project As creates a separate copy and never
overwrites an existing folder.

## Notes

- Disabled actions explain why they are unavailable. Availability depends on project and selection context.
- **Insert Snippet…** opens a chooser for project and personal snippets.
- Menu labels are concise; longer descriptions stay in command search and contextual help.
- All commands retain their registered IDs, keyboard shortcuts, and macro/keymap access.

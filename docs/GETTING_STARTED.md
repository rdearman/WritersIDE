# Getting started with WIDE

WIDE is currently an alpha development build. Keep a separate backup of any
work you create and retain the original when trying an import.

## Start the application

From the repository root, run:

```sh
cargo run -p writers-ide-terminal
```

The start screen is temporary. Choose **File → New Project**, select Fiction or
Non-fiction, then choose a parent folder and enter a project name. WIDE creates
the project folder for you. An existing project can be opened with:

```sh
cargo run -p writers-ide-terminal -- /path/to/project
```

## Choose a starting point

- A fiction project opens in the scene-planning workspace. Create scenes, give
  them useful titles, and arrange or exclude them as your outline develops.
- A non-fiction project opens in the chapter outline. Chapters can contain
  prose directly; scenes are not required.

Select a manuscript item and open it to draft. The editor works with the
project’s manuscript text, and **Ctrl+S** saves the current document. Return to
the outline to choose another chapter or scene. The status area reports save
results and conflicts.

Use the **Help** menu for context help and the tutorial. The exact available
commands depend on the workspace and alpha build; the [menu reference](../menu.md)
lists the current menu entries.

## Importing an existing project

Scrivener import is experimental. Work on a copy of the source project, inspect
the import preview and warnings, and compare the result before continuing to
write in WIDE. The import workflow is not yet a general compatibility
guarantee.

## Compiling a draft

Use the **Publish** menu to choose a supported output format and destination.
Compilation and fidelity remain under development. Inspect the generated
document before sharing or publishing it.

## Report a problem

Include the WIDE commit or build you used, the terminal size, the steps you
took and what you expected to happen. Do not attach copyrighted manuscript
material unless you have permission to share it. [Open a GitHub issue](https://github.com/rdearman/WritersIDE/issues).

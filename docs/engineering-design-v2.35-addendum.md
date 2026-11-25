# Engineering Design v2.35 Addendum — Named Window Layouts

## Scope

The View workspace now supports named, project-local recursive window layouts.
This closes the save/load-layout implementation gap in GitHub issue #47 while
preserving the existing pane tree, session restoration, and shared view buffers.

## Storage and identity

Named layouts are stored in `.writers-ide/window-layouts.json` as version 1 JSON.
Each entry contains a user-provided name, the recursive `PaneLayout` tree, and
one stable source key for each pane. Manuscript editor panes use the dedicated
`manuscript-editor` source identity. Reference panes store their source keys and
an occurrence index, so multiple windows displaying the same source remain
distinct. Pane IDs are remapped to currently open matching views when a layout
is loaded.

Writes use the existing project-local atomic JSON writer. Layout names are
limited to 64 characters, reject controls and path separators, and replace an
existing name case-insensitively. Loading validates the file version, tree,
pane identities and source availability before changing the current layout.
Missing sources and malformed files produce an in-application error without
partially applying the layout.

## User interaction

`View > Save Layout…` asks for a name. `View > Load Layout…` lists saved names
in the prompt and asks which to load. Both commands require the manuscript
editor workspace. Layouts are project-local and travel with the project.

The View menu presents a single reversible maximize action: `Maximize Current
Window` changes to `Restore Windows` while maximized. It preserves and restores
the recursive split tree and current focus. The stable `window.restore-layout`
command remains in command search for keybindings and automation but is not a
second competing row in the primary View menu. F11 is not assigned by default
because it is already used for Find Query History.

## Current limitation

Loading reconnects source keys to views currently open in the project. It does
not reopen a closed reference document or create a character/research/notes view
that is not yet implemented as a pane source. The load operation identifies the
missing view and leaves the current pane tree unchanged. Full enumeration and
opening of every supported project document remains outstanding under #47.

## Verification

Automated coverage includes saved layout persistence, pane-ID remapping by
stable view source, reopen restoration, missing-source rejection, unsafe-name
rejection, recursive maximize/restore, and Linux PTY operation through the View
menu. These checks do not replace SSH/tmux validation or human usability review.

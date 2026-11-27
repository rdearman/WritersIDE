# Keybindings

Writer's IDE uses one command registry with three manuscript editing profiles: **Standard** (the default), **WordStar**, and **Custom** (the Standard editing behaviour with the author's configured overrides). Global commands and window management remain consistent across profiles and take precedence over profile-specific editing defaults. In particular, Ctrl+S always saves and Ctrl+X always cuts; use the ordinary arrow keys for left/down movement when those chords would conflict with WordStar navigation. Open **Tools → Keybinding Manager** and press `p` to cycle profiles; the current profile is shown in the manager. User and project overrides remain separate and project bindings take precedence.

The WordStar profile currently provides non-conflicting classic cursor movement (Ctrl+E up and Ctrl+D right), page movement (Ctrl+R/C), line/document movement (Ctrl+Q then S/D/R/C), and block selection/copy/delete (Ctrl+K then B/K/C/Y). The author-approved global Ctrl+S Save and Ctrl+X Cut commands take precedence over the legacy WordStar left/down chords; standard arrow keys remain available for those movements. Other commands remain available through menus and the command palette. Custom profiles use the existing user/project keymap configuration and contextual sequence support; they do not remove the Standard menu routes.

## Configuration files and precedence

The optional user file is `$XDG_CONFIG_HOME/writers-ide/keybindings.toml`, falling back to `~/.config/writers-ide/keybindings.toml`. A project may also contain `.writers-ide/keybindings.toml`.

The Standard defaults load first, then user overrides, then project overrides. A project binding wins when the same key and context are assigned at both levels. Other user bindings remain active. Invalid files are reported and ignored safely; they do not prevent project startup. The Keybinding Manager can edit either scope, select the manuscript profile, remove overrides, and restore defaults.

## Dialogues and file browsers

Enter activates the focused control or choice, Esc cancels or returns, Tab/Shift+Tab moves focus, arrows move through choices and lists, and Space toggles an appropriate control. Save As, backup, import-destination and compile dialogs open with their filename/action control focused, so Enter follows the visible primary-action hint. Open and insert browsers retain focus on their folder tree or file list; Enter opens a directory/project or selects a file as the dialog indicates. F-keys remain optional accelerators only.

The complete schema and examples are in [keybindings.example.toml](keybindings.example.toml). The example is also parsed and checked against the command registry by a regression test.

## Supported mapping forms

- `[bindings]` maps function keys to workspace destinations, such as `F4 = "Reading"`.
- `[commands]` maps a modified chord to a stable command ID, such as `"Alt+F" = "menu.file"`.
- `[contexts.editor]`, `[contexts.navigation]`, `[contexts.dialog]`, and `[contexts.menu]` map multi-stroke sequences to commands in that context.
- The same sequence may be used in separate contexts. Duplicate sequences and ambiguous prefix pairs within the effective context are rejected.
- Keybinding Manager supports additional bindings per command and reports conflicts before saving.

## Terminal limits and reserved input

Only keys that the terminal delivers can be bound. Conventional terminals encode Ctrl+/ as ASCII 0x1F, decoded by Crossterm as Ctrl+7; WIDE recognises that event as the registered Ctrl+/ menu command unless an explicit user binding claims it. Alt+M is the non-function-key menu alternative; F10 remains an optional accelerator. Alt/Meta reporting varies by terminal and SSH/tmux configuration. Bare printable keys are not accepted as global command bindings, preserving manuscript typing and IME input. A small set of application editing shortcuts is reserved to protect save, undo, redo and search navigation.

The example's `Ctrl+X Ctrl+S` is a context sequence: the first chord starts it, and the second completes the save command. Sequence timeout and cancellation are handled by the application. No keymap changes alter manuscript text, selection, or undo history.

These guarantees are covered by automated parser, precedence, conflict, sequence-dispatch and persistence tests. Actual key delivery should still be checked in the target terminal, SSH client and tmux configuration.

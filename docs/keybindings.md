# Keybindings

Writer's IDE uses one native keymap and stable command IDs. User keybindings are overrides; there are no WordPerfect, WordStar or Emacs compatibility profiles. To see the current effective bindings, open **Tools → Keybinding Manager** or run `keybinding-list` in the command palette.

## Configuration files and precedence

The optional user file is `$XDG_CONFIG_HOME/writers-ide/keybindings.toml`, falling back to `~/.config/writers-ide/keybindings.toml`. A project may also contain `.writers-ide/keybindings.toml`.

The built-in defaults load first, then user overrides, then project overrides. A project binding wins when the same key and context are assigned at both levels. Other user bindings remain active. Invalid files are reported and ignored safely; they do not prevent project startup. The Keybinding Manager can edit either scope, remove overrides, and restore defaults.

The complete schema and examples are in [keybindings.example.toml](keybindings.example.toml). The example is also parsed and checked against the command registry by a regression test.

## Supported mapping forms

- `[bindings]` maps function keys to workspace destinations, such as `F4 = "Reading"`.
- `[commands]` maps a modified chord to a stable command ID, such as `"Alt+F" = "menu.file"`.
- `[contexts.editor]`, `[contexts.navigation]`, `[contexts.dialog]`, and `[contexts.menu]` map multi-stroke sequences to commands in that context.
- The same sequence may be used in separate contexts. Duplicate sequences and ambiguous prefix pairs within the effective context are rejected.
- Keybinding Manager supports additional bindings per command and reports conflicts before saving.

## Terminal limits and reserved input

Only keys that the terminal delivers can be bound. Some terminals collapse Ctrl+/ into another event; F10 remains a fallback. Alt/Meta reporting also varies by terminal and SSH/tmux configuration. Bare printable keys are not accepted as global command bindings, preserving manuscript typing and IME input. A small set of application editing shortcuts is reserved to protect save, undo, redo and search navigation.

The example's `Ctrl+X Ctrl+S` is a context sequence: the first chord starts it, and the second completes the save command. Sequence timeout and cancellation are handled by the application. No keymap changes alter manuscript text, selection, or undo history.

These guarantees are covered by automated parser, precedence, conflict, sequence-dispatch and persistence tests. Actual key delivery should still be checked in the target terminal, SSH client and tmux configuration.

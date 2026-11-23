# Engineering Design v2.11 Addendum — Keymap Configuration Reference

**Date:** 23 November 2026  
**Related issues:** #8, #22, #31

## Configuration contract

The implementation uses one native default keymap and optional user/project TOML overrides. The documented example in [`keybindings.example.toml`](keybindings.example.toml) covers workspace function-key overrides, stable command IDs, alternate menu aliases, and context-scoped multi-stroke sequences. [`keybindings.md`](keybindings.md) documents file locations, precedence, conflicts, safe recovery, terminal input limitations, and the Keybinding Manager.

The implementation continues to use the existing typed command registry and dispatcher. This addendum does not introduce editor emulation modes or duplicate command handlers. Automated tests parse the example and verify every referenced command ID, chord and context against the current implementation.

## Verification boundary

Automated tests establish parsing, validation, scope precedence, sequence behavior, persistence and safe fallback. Terminals and SSH/tmux combinations vary in which modifier combinations they report, so real-terminal behavior remains an environment-specific verification item under #31.

# Writer's IDE — Engineering Design v0.9

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.8

## Terminal workspace bindings

The full-screen terminal application reads optional project-local configuration from `.writers-ide/keybindings.toml`:

```toml
[bindings]
F1 = "Analytics"
F4 = "Reading"
```

Supported keys are F1–F14 and workspace values are the user-facing workspace titles, matched without case sensitivity. Unspecified bindings retain their standard workspace mapping. Invalid keys or workspace names fail visibly during startup rather than silently changing key behavior. This first configuration supports F-key destination changes; arbitrary multi-key shortcuts remain open.

The configuration remains project-relative and travels with the novel. It does not change domain operations or persistent story data. The keymap parsing unit test checks fallback defaults, partial override behavior and target resolution.

## Remaining terminal gaps

Although the TUI is a genuine alternate-screen Ratatui/Crossterm application with split panes, palette, history, search, keyboard editing, reference reading and workspace navigation, several SRS workflows are still represented mainly as read-only summaries. These need focused implementation slices: richer outline restructuring and card actions; full character/story-bible/location/plotline forms; configurable non-function shortcuts; in-editor selection and clipboard operations; and explicit focus traversal for split panes. Do not mark TUI-001 complete until those are implemented and exercised in terminal behavior tests.

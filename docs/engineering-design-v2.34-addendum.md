# Engineering Design v2.34 Addendum — Searchable View Switching

The View menu now includes **Switch View…**. It opens the command registry in a view-scoped searchable chooser, showing recent view commands first and available supported destinations after them. The chooser can switch an editor-focused pane to Outline, Scene Cards or Timeline, or open the corresponding Outline, Timeline and Compile Diagnostics reference views. When a reference pane is focused, a newly selected view reuses that pane and preserves the recursive split tree. Existing views can also be reached through Next/Previous Window.

The chooser uses the shared command dispatcher and stable command IDs. It does not accept raw UUIDs or issue SQL. Its query and selected command are temporary UI state; pane identity, selected source and layout persist in the project session. Automated coverage checks searchable scoping, focused-pane reuse, tree preservation, session restore and view source identity.

The chooser currently exposes supported manuscript/reference destinations only; a comprehensive catalogue of every open/recent document, direct scene/chapter picker, named layout save/load, independent character/research/notes panes, and manual SSH/tmux validation remain open under #47. Human UAT remains required.

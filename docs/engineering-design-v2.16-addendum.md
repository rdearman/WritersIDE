# Engineering Design v2.16 Addendum — External-reference clipboard

**Status:** Implemented; terminal-specific behavior still requires human verification  
**Date:** 2026-11-24  
**Related issue:** #57

## Behavior

When the author selects a passage in a read-only external text reference and presses Ctrl+C, the terminal application keeps the passage in its existing app-local clipboard and kill ring, then emits an OSC 52 clipboard request to the terminal. This lets compatible local terminal emulators and SSH clients copy the selected passage to the user's system clipboard. Manuscript editing continues to use the app-local Ctrl+C/Ctrl+X/Ctrl+V behavior.

The OSC 52 payload uses standard Base64 and is capped at 100 KiB after encoding. Larger selections remain in the app-local clipboard and show a visible message that terminal clipboard transfer was skipped. The terminal may reject clipboard requests due to its own security policy; the application cannot infer acceptance, so the UI explicitly says “if supported” and retains the internal paste path.

The source reference is read-only and is never modified by copy. No clipboard contents are written to project files or logs. The TUI sends the control sequence through the existing Crossterm backend only after an explicit copy action.

## Verification

The reference-copy integration test verifies the copied Unicode passage, exact OSC 52 framing and Base64 payload, payload-size rejection, app-local paste into the manuscript, and byte-for-byte unchanged source. `cargo test --offline -p writers-ide-terminal external_text_file_opens_read_only_and_copies_visible_line_into_manuscript` passed.

## Limitations

OSC 52 support and permission vary by terminal emulator and SSH configuration. The app-local clipboard is the fallback. This does not enable system clipboard integration for ordinary manuscript selection/copy, which remains app-local.

# Engineering Design v2.38 Addendum — Special Character Insertion

The Insert menu's **Special Character…** action opens a focused Unicode picker.
The initial catalog covers typographic punctuation, non-breaking and
discretionary hyphens, common publishing symbols, and several currency signs.
Authors can filter by descriptive name, literal glyph, or `U+` code-point
notation, and can enter any valid printable Unicode scalar using `U+XXXX` or
`0xXXXX`.

Insertion goes through the manuscript editor's existing undoable insertion and
journalled save paths. The most recent 12 distinct inserted scalars are stored
in `.writers-ide/editor-preferences.toml`; older preference files receive an
empty list through the serde default. Invalid, control, and surrogate code
points are rejected. Cancel and unmatched searches do not alter manuscript
text. The picker does not rewrite source formatting or apply smart punctuation.

Smart punctuation remains deferred. It needs an explicit, configurable policy
for preserving literal input and language-specific quote/apostrophe behavior.
The picker is available through Insert and the command registry; it does not
assign a fixed shortcut.

Automated coverage verifies menu dispatch, named and code-point search,
surrogate rejection, insertion, undo, save/reopen, recent-character persistence,
cancellation, and the 80×24 Linux PTY path. This documents implementation
behavior; human discoverability review remains outstanding under #61 and #54.

# Engineering Design v2.37 Addendum — Rendered Editor Cursor Mapping

## Scope

The rendered Markdown editor now routes ordinary cursor movement and typing
through the existing semantic source/display projection. Canonical Markdown
remains the persisted representation; this increment does not introduce a
second editor buffer or rewrite source during display operations.

## Cursor and insertion contract

- Left/Right movement advances by visible grapheme in the rendered projection,
  skipping hidden Markdown delimiters. Up/Down move between source paragraphs
  using projected columns; Home/End target the visible line boundaries. These
  vertical commands do not yet treat terminal soft-wrapped rows as separate
  cursor lines.
- Typing, Tab, Enter, Backspace and Delete first resolve a lone editor cursor
  from a hidden-syntax boundary to the corresponding visible source boundary.
  The shared editor operation then mutates canonical Markdown and uses the
  existing Undo and journalled save paths.
- Unicode positions remain grapheme-aware through the projection's UTF-8 byte
  map. Source-fallback documents retain the existing source-editor behavior.
- A selected range continues to use canonical source offsets. Multi-cursor
  editing currently retains source-mode cursor movement because each cursor
  needs an independently mapped anchor; this is an explicit limitation.
- Rendering and cursor movement do not change manuscript bytes. Only an
  explicit edit marks the document dirty.

## Verification and remaining work

Regression coverage verifies that typing at the beginning of rendered bold
text inserts inside the semantic span, visible horizontal navigation skips
format delimiters, and typing after movement changes the expected source
boundary in a document containing accented and emoji graphemes. This does not
complete #61 coordinate mapping: selection extension, wrapping-aware vertical
movement, multi-cursor semantics, clipboard, find/replace and proofreading in
rendered coordinates, plus human writing-workflow acceptance, remain open.

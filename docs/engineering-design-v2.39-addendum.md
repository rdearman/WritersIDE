# Engineering Design v2.39 Addendum — Join Adjacent Paragraphs

`Edit → Join Adjacent Paragraphs` joins the paragraph under the cursor to the
following non-empty paragraph, consuming the intervening blank-line separator
and inserting one space. When there is no following paragraph, it joins to the
previous paragraph. Newlines inside a paragraph remain unchanged. The operation
uses the existing validated source-range replacement path, including narrowed
passage checks, one-step undo and journalled save. It does not change scene,
chapter or Markdown storage models.

The command does nothing when the current paragraph is empty or has no adjacent
paragraph. Paragraphs are separated by blank lines in the current Markdown
model; single line breaks inside a paragraph are preserved.

This is a partial implementation of the editing completeness audit in GitHub
issue #61. Split/merge document objects, deduplication preview, transposition,
whitespace cleanup and the remaining formatting requirements are outstanding.

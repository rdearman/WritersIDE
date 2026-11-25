# Engineering Design v2.33 Addendum — Recursive Terminal Windows

This addendum records the implementation of the recursive pane workflow requested by GitHub issue #47. The domain documents and editor buffers remain owned by the application; the layout tree stores only the view arrangement and stable pane identifiers.

## Layout and session invariants

- Pane IDs are monotonically allocated `u64` values; pane 1 is the primary manuscript editor.
- A split replaces only the focused leaf, preserving its ID as the first child and assigning the new second child a fresh ID.
- Vertical splits render side by side; horizontal splits render top and bottom.
- Split operations are rejected before mutation when the current terminal rectangle cannot provide the minimum pane dimensions.
- Closing a pane promotes its sibling subtree into its parent rectangle. Closing the only pane is rejected.
- Single View retains the focused leaf; documents and manuscript text remain available in their project-owned sources.
- The serialized tree is accepted on restore only if its pane IDs resolve to restored document views and its focus is a member of the tree. A maximized tree may temporarily contain a valid subset of open panes; a saved restore tree must match the complete set.
- Scene and direct-prose editor duplicates use stable scene/content-unit identifiers. Duplicate reference panes may share a source identifier while retaining separate pane IDs, focus and cursor state.

## UI and command contracts

View exposes **Split Side by Side**, **Split Top and Bottom**, **Grow Focused Window**, **Shrink Focused Window**, **Next Window**, **Previous Window**, **Close Window**, and **Single View** when applicable. Resizing adjusts the nearest parent divider by 500 basis points and persists the resulting tree. Existing reference commands remain available. `Return to Writing` focuses pane 1 when the editor remains in the layout, or restores the editor beside the focused pane if the author closed its view. The command registry is the dispatch surface for menu actions; the terminal UI does not issue SQL.

The renderer derives rectangles from the current frame on every draw, so resize does not rewrite the tree or document state. At compact terminal sizes, existing trees render within the available rectangle; new splits are rejected when the selected leaf cannot satisfy the minimum dimensions.

## Verification

Terminal tests cover recursive vertical/horizontal splitting, menu dispatch, closing and reducing a tree, save/reopen with stable document content, Return to Writing, and rendering at 80×24, 177×60, and 201×60. Automated rendering tests establish geometry and state persistence only; they do not establish human usability. Issue #47 remains open for human acceptance.

# Engineering Design v2.07 Addendum — Context-specific navigation labels

This addendum records the implementation response to GitHub Issue #45.

The shared details pane now names the kind of item it actually describes:

- Outline: **Selected outline item**.
- Story Bible: **Selected story record**.
- Issues: **Selected issue**.
- Research: **Selected research item**.
- Provisional knowledge review: **Proposed story information**.

Other workspaces retain **Selected scene** only when the pane is showing a scene. The generic workspace navigation hint now spells out the behavior as **Tab: next primary view · Shift+Tab: previous**. The manuscript editor retains its distinct **Esc: Outline** hint because Tab inserts indentation there.

These checks make the labels agree with the selected workspace and current keyboard behavior. They do not constitute the broader first-time-human terminology acceptance requested by Issue #45; that issue remains open for the author's UAT.

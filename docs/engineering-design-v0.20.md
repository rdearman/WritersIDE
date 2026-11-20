# Writer's IDE — Engineering Design v0.20

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.19

## Terminal outline operations

The command palette now offers structural scene-planning operations: `section <title>`, `chapter <title>`, `subchapter <section-id> <title>`, `scene <chapter-id> <title>`, `move <scene-id> <chapter-id> <position>`, `rename <scene-id> <title>`, and `scene-status <scene-id> <status>`. Creation chooses the next sibling position; movement delegates to the application service's transactional position algorithm. Scene renaming updates typed metadata without changing stable IDs or file paths. The Outline workspace displays chapter titles alongside their scenes.

The terminal regression test creates nested structure, drafts text, links a POV character, moves and renames the scene, then verifies placement, scene identity, relationship, and manuscript content. This makes the TUI useful for actual outlining and rearrangement, while full column/hoist/clone views and metadata forms remain future work.

## Practical manuscript editing and entity creation

The Editor workspace now supports character-indexed text selection, highlighted selection rendering, multiline copy/cut/paste in the application's internal clipboard, and Shift+Arrow selection. Uppercase/shifted text is accepted, and horizontal movement crosses line boundaries. Selection and clipboard state are transient UI state; only resulting manuscript text is persisted, through the existing optimistic durable save path. Conflicts continue to retain the unsaved editor buffer.

The terminal command palette can create a character (`character <name>`), plotline (`plotline <name>`), issue (`issue <title>`), and research note (`research <title> | <body>`), then navigate to the corresponding workspace. These operations use `story-app` services and do not issue domain SQL from the UI. They create initial records only; detailed entity editing forms and OS clipboard integration remain unresolved.

Regression tests cover multiline selection extraction/deletion, clipboard paste, selection extension/collapse, shifted uppercase entry, line-boundary movement, and command-palette creation for character, plotline, issue, and research entities. Offline workspace tests and Clippy pass for this revision.

## Plot Grid matrix

The Plot Grid workspace now derives a read-only matrix from `analytics_dataset`: scenes are rows in manuscript order, plotlines are columns, and a filled marker indicates scene membership. Chapter titles are resolved from structural nodes; unplaced scenes are labelled explicitly. The matrix reads canonical relationships and cannot change them. The app-level regression fixture verifies that an assigned plotline appears in the matching scene row.

The Characters workspace now derives a read-only scene-by-character matrix. Cells distinguish POV (`P`), major (`●`), minor (`○`), off-camera (`o`), mentioned (`m`), and no recorded participation (`·`). It reads application analytics and remains a projection; editing relationships continues through application services. A regression check verifies that a POV link appears with its role marker.

## Passage anchor revalidation

Scene saves revalidate passage anchors inside the same SQLite transaction that commits the manuscript hash and search index, including recovery after an atomic file replacement. A unique exact quote match is reattached automatically. Repeated matches are scored against saved prefix/suffix context and are reattached only if one candidate has a unique non-zero best score. Missing passages become `broken`; ambiguous repeated passages become `needs_review`. The stable anchor UUID is retained in all cases. `anchors` lists a scene's anchor states and `anchor-move <anchor-uuid> <start-byte> <end-byte>` explicitly reattaches an anchor after the author selects the correct passage. The operation validates UTF-8 boundaries and exact quote identity. The CLI provides `anchors` and `anchor-move` equivalents.

Tests cover unique relocation, context ambiguity, removed passages, explicit reattachment, invalid offsets and recovery after rename-before-database-commit. Anchor updates participate in the same transaction as recovered manuscript metadata so the durable journal cannot commit text while leaving anchor state silently current at stale offsets.

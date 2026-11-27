# WIDE User Experience and Interaction Design Specification

> **Superseded:** This unapproved proposal is retained as historical discussion only. The
> author-approved normative interaction contract is [interaction-design-v1.0.md](interaction-design-v1.0.md),
> tracked under GitHub issue #77. Where they differ, follow the approved specification.

**Version:** 1.0 proposal
**Status:** Draft for author review; not approved
**Date:** 26 November 2026
**Authority:** Discussion document only. It does not supersede the SRS, engineering design, approved GitHub requirements, or current application behaviour. It authorises no implementation or interface restructuring.

This proposal is intended to help the author decide how WIDE should feel and behave as one writing environment. Recommendations and alternatives below are design proposals, not accepted requirements. Existing capabilities should be preserved unless a later approved decision explicitly changes them.

## 1. Purpose

The Software Requirements Specification describes a broad set of authoring capabilities, and the engineering design describes their data and implementation contracts. The command audit inventories operations. Human testing has shown that these documents do not yet give the application one coherent interaction model: individually available operations can still be difficult to discover, can behave differently across contexts, or can fail to preserve the author's sense of where they are.

This document proposes that interaction model. It focuses on the author's working context, the relationship between manuscript structure and prose, reliable movement between planning and writing, and the feedback that makes project state understandable. It is not a proposal to replace the Rust core, storage model, terminal application, or existing features.

### Author's stated design direction (proposal, not approval)

The author describes WIDE as **a hierarchical document editor with integrated novel-development tools**: the manuscript is central, and other capabilities support it. The proposed influences are GrandView's outlining philosophy, Scrivener's document management, WordPerfect's editing conventions, and Emacs's flexible workspaces. The author explicitly does not propose reproducing any of those applications wholesale. This guidance is incorporated below as a design direction for review; it does not approve a new interface, bindings, or implementation.

## 2. Evidence, limits and terminology

### 2.1 Material reviewed

This draft is based on the repository material available on 26 November 2026:

- `docs/software-requirements-v0.2-addendum.md` and the SRS v0.1 supplied in the project brief. The original SRS source file is not present in the repository.
- `docs/engineering-design-v1.92.md`, which contains the active design material available for this review, and later implementation addenda referenced by `docs/DEVELOPMENT_STATUS.md`.
- `docs/DEVELOPMENT_STATUS.md`, including the Alpha UAT 35/36 records, project lifecycle findings, outliner changes and open human acceptance gates.
- `docs/COMMAND_WORKFLOW_AUDIT.md`, `menu.md` and `menu-audit.md`.
- The current application and tests as represented by the most recent development handover.

The GitHub open-issue list and latest comments on #76, #44, #54, #55, #37, #38, #46, #53, #42, #52, #68, #20 and #7 were retrieved through the authorised GitHub CLI. They confirm that #76 remains the production terminal audit/release gate; #44 is still awaiting human acceptance; the latest comments record the Alpha UAT 36 checkpoint; #53 and #7 have later Scrivener profile work whose publication fidelity remains incomplete; and #55 requires a unified outliner while preserving Scene Cards behavior. Issue #75 is explicitly marked post-freeze/vNext and is excluded from this v1.0 proposal and implementation goal. Other open issues have not all been read comment-by-comment as part of this design review and remain in the continuing backlog review.

### 2.2 Terms used in this proposal

- **Project:** One book and its associated research, planning records, settings and outputs.
- **Manuscript item:** An author-visible structural or prose item such as a section, chapter, scene, or direct chapter prose.
- **Workspace:** A task-oriented application area, such as Write, Research or Publish.
- **View:** A presentation of project information within a workspace, such as Outline, Cards or Timeline.
- **Focus:** The part of the interface that receives the next keyboard action.
- **Selection:** The item or text currently targeted by an operation.
- **Active document:** The manuscript or reference document currently open for reading or editing.

The interface should use author-facing language. These definitions clarify this proposal; they are not proposed labels that must be shown in the application.

## 3. Design goals

The proposed experience should:

1. Let an author tell at a glance which project, manuscript item, workspace and pane are active.
2. Make the common writing journey direct: open a book, find a place, write, save, return to structure and continue.
3. Keep manuscript hierarchy and prose connected while retaining their distinct purposes.
4. Preserve scene-card features, including inclusion state, synopsis and word count, as the outliner grows to include chapter prose and nested structure.
5. Make structural edits understandable and recoverable.
6. Keep private planning and research information available without confusing it with publication content.
7. Present feedback in the context of the action that caused it.
8. Keep ordinary authoring independent of UUIDs, SQL, command identifiers and internal syntax.
9. Support keyboard-only terminal use, SSH, tmux, resizing and monochrome display.
10. Keep advanced functions available without making their full inventory the ordinary writing interface.

## 4. Existing approved requirements versus proposals

| Topic | Already approved or established in the project | Proposal in this document |
|---|---|---|
| Local-first project and manuscript safety | Portable projects, journalled saves, conflict detection, recovery, stable identities | Make saved/staged/unsaved state visible in one consistent place |
| Manuscript structure | Sections, chapters, scenes and direct prose; fiction and non-fiction; one unified outliner is required by current #55 direction | Use one outline tree as the primary structure view, with Cards as a projection of that same hierarchy |
| Scene identity and placement | Scenes have stable identity; placement/inclusion/chronology are separate concepts | Show those concepts with distinct visual indicators and explanatory context |
| Project types | Fiction and non-fiction project types exist; non-fiction may use chapter prose without scenes | Start a new project by asking Fiction or Non-fiction in a short, clear dialogue |
| Menus and shortcuts | Keyboard-first interaction, Ctrl+/ preferred and F10 fallback, command registry, ordinary `/` remains text; current menu conventions are documented in `menu.md` | Rationalise the menu taxonomy and define contextual availability consistently |
| Import | Scrivener import, automatic structural preservation and staging workflows are approved | Make normal import end in the opened manuscript, with advanced mapping/report review optional |
| Publishing | Shared Compile IR, named profiles, diagnostics, output formats and selection rules exist or are in approved scope | Present compile as a guided edition workflow with a clear content summary before output |
| Features and visual presentation | Preserve existing functionality and established visual behavior; human UAT remains required | The exact proposed default layout and visual conventions require author approval |

Nothing in the proposal downgrades an approved requirement. When this proposal differs from the current implementation, it indicates a design question or potential gap, not permission to change code.

## 5. Reference interaction patterns

These are pattern observations, not a request to clone another application.

### 5.1 Scrivener

Scrivener's Binder makes a book's hierarchy persistent and directly navigable; its editor keeps the selected document central; synopsis/card and inspector presentations expose planning information without making every fact part of the prose. Its compile configuration also demonstrates that manuscript organisation and publication assembly are related but not identical.

**Useful lesson for WIDE:** keep a visible structural navigator, selected-document identity, and focused document canvas; treat publication assembly as a separate, explicit choice.

**Do not copy:** Scrivener's platform-specific panels, terminology where it does not fit WIDE, or hidden assumptions that every source item belongs in a publication.

### 5.2 GrandView

GrandView's outline model supports deep hierarchy, hoisting a branch, alternate views and stable references to information from more than one organisational context.

**Useful lesson for WIDE:** structure should be navigable at multiple scales; an author should be able to narrow attention to a section without changing the book itself; alternate views should be projections over the same underlying items.

**Terminal translation:** predictable tree movement, collapse/expand, branch focus, clear parent context and stable selection after an operation. Clones or references must remain distinguishable from duplicate manuscript documents.

### 5.3 Traditional word processors

Word processors establish familiar expectations: a persistent menu or ribbon, a clearly focused document, visible selection and cursor, undo/redo, conventional text movement, a dependable save state, and commands available in context.

**Useful lesson for WIDE:** novel-specific planning should augment a legible manuscript editor rather than make ordinary typing feel like operating a database. Structural and publication controls should be discoverable while prose remains the visual centre of writing.

WordPerfect's Reveal Codes tradition suggests that semantic formatting should be inspectable and repairable without forcing authors to draft in markup syntax. Emacs demonstrates that keyboard-driven editing, buffers, panes and task-specific views can coexist without requiring a fixed sequence of full-screen modes. The author's proposal draws on these patterns but does not ask WIDE to clone either product.

## 6. Proposed application model

### 6.1 Shared shell

The following concepts should be shared by every workspace:

```text
Application
└── Project
    ├── Workspace (current task area)
    ├── View (presentation within that area)
    ├── Focus (where input goes)
    ├── Selection (target of an operation)
    └── Active document (stable project item, if any)
```

The current workspace, focused pane, selected item and active document are related but not interchangeable. The proposed design keeps them explicit so that a command cannot silently act on a stale scene or an unrelated selection.

The author's architectural question is whether the current application already has a coherent owner for these shared interaction facts. A conceptual boundary for review is:

```text
Terminal views: menus · editor · outliner · inspectors · boards
                         │
Proposed shared interaction responsibilities:
focus · selection · active identity · navigation · commands · workspace/panes
                         │
Existing application services:
manuscript · story data · import · compile · persistence · recovery
```

This is a hypothesis to investigate, not a requirement to create a new controller crate or subsystem. If approval follows, trace current state ownership first and prefer the smallest correction that removes contradictory state.

### 6.2 Candidate workspace organisation

Two plausible structures are offered for review.

**Option A — Task workspaces:** Write, Plan, Research, Review, Publish, with project-wide File, Edit, Navigate, View and Help commands.

Advantages: names reflect author tasks; related operations can be grouped; the application has a memorable centre of gravity.
Disadvantages: many current views do not fit cleanly into a single task; workspace switching may hide useful information; introduces a larger information-architecture change.

**Option B — Current view/workspace model:** retain the existing named workspaces such as Outline, Scene Cards, Timeline, Analytics, Characters, Story Bible, Research, Issues and Search.

Advantages: least disruption; preserves established routes and individually named work areas.
Disadvantages: a long list of peer destinations treats analysis views as equal to everyday writing; an author may need to know the implementation's workspace vocabulary.

**Option C — Hybrid (proposal for consideration):** make **Write** the primary destination, with Outline and Cards as views of the same manuscript; group other current destinations under task labels or a searchable workspace switcher without removing them. Keep Research, Review and Publish as clear destinations, and make specialised analysis available under Plan/Review or Search All Commands.

Advantages: establishes an author-centred default while retaining capabilities; supports a single outliner; progressive disclosure without deleting functions.
Disadvantages: requires careful migration of commands and help; grouping can make a rarely used view harder to find; requires agreement on exact menu and workspace labels.

No option is selected by this draft.

#### Candidate first screen

The author's proposed default is a manuscript-first Writing workspace with hierarchy and editor panes. The project should be open for writing within seconds; a mandatory dashboard, statistics view or planning board should not stand between launch and the manuscript. This can be balanced with first-run guidance through a small non-blocking welcome/tutorial prompt or a blank outline with obvious actions.

```text
┌ File  Edit  Insert  Format  Navigate  View  Tools  Help ──────┐
│ The Glass Harbour · Fiction · Saved                           │
├ Outline ─────────────────┬ Manuscript ────────────────────────┤
│ ▾ Part One                │ Chapter One                        │
│   ▾ The Arrival            │                                    │
│     ◉ The letter           │ Thomas was certain the letter had  │
│     ○ The station          │ arrived before the storm.          │
│   + New chapter             │                                    │
└────────────────────────────┴───────────────────────────────────┘
```

Alternative: open at the last active workspace and let the author choose an initial layout in preferences. This better respects session restoration but can reopen into an unfamiliar analysis pane. A dashboard remains available as an optional workspace, not a required landing screen. This is a design proposal and does not change current startup behavior.

### 6.3 A common manuscript-node presentation

The author proposes a common author-facing document-node abstraction. Each node has applicable properties: title, type, optional text, optional children, synopsis, private notes, status, publication inclusion, own and descendant word counts, and relevant metadata such as characters, locations, chronology and custom fields. A chapter may contain prose, child sections/scenes, or both. Fiction and non-fiction should share editing and structural operations; templates and optional metadata can differ.

This is a presentation model, not a proposed database rewrite. The existing structural-node, scene, content-unit and placement data should be reused wherever possible. Decisions for review include how a scene appears relative to other document types, how direct chapter prose is ordered with child content, and which properties are visible by default.

The author also proposes four flexible workspace presets—Writing, Planning, Research and Focus—built from panes that can display supported views. Splitting a pane would create another pane in the same project context, not a separate application mode. This can coexist with the existing workspace model, but its focus, selection, resizing, persistence and close behavior need a design decision. A lower-risk alternative is to preserve fixed workspaces and the existing split/reference layouts while allowing a small set of named presets.

## 7. Navigation and focus model

### 7.1 Proposed rules

These rules are proposed cross-application invariants for author review:

- The interface always shows the current project and workspace.
- Focus is visible and distinct from selection. Moving focus does not silently change the selected manuscript item.
- Enter opens or acts on the selected item according to the focused view; the action is indicated by a visible hint or contextual command.
- Escape returns to the immediately preceding meaningful context, preserving the prior selection and editor location when possible.
- A command targets the active document or selected item visibly named in the command context. If no valid target exists, it is disabled or explains why.
- A successful action changes visible state immediately. Success feedback names the result, not an internal operation.
- Navigation history can return to a prior document and location without changing manuscript order.
- A workspace change does not imply a manuscript move.

### 7.2 Key conventions for discussion

The existing menu access decision remains Ctrl+/ with F10 fallback. The proposal is to display the effective shortcut in the menu/help/footer, and to keep ordinary `/` printable in the editor. Enter, Esc, arrows, Tab and function keys should have stable roles, but exact conflicts with current bindings must be tested and approved before changes.

| Key | Proposed default role | Question for review |
|---|---|---|
| F10 / Ctrl+/ | Open menu bar | Keep both as fixed safety routes even if one is remapped? |
| Enter | Open/confirm/expand in the current context | Should tree-node Enter always open content, with a separate key for expand? |
| Esc | Close current layer or return one context | How many nested layers should one Esc unwind? |
| Tab / Shift+Tab | Move between major panes or dialogue controls | Should Tab be reserved for focus rather than menu modes? |
| Ctrl+S | Save | Preserve the current configurable binding and show it in status/help |
| F1 | Context help or existing assigned action | Current assignment conflicts must be resolved explicitly, not silently |

## 8. Project lifecycle proposal

The approved temporary-project architecture should be presented as an ordinary project lifecycle, not as a technical staging feature. The author's direction is the traditional word-processor flow: no destination browser until a permanent location is needed, no compulsory import mapping for ordinary use, and no reduced functionality while the project is temporary.

### 8.1 New Project

Proposed short dialogue:

```text
┌ New Book ───────────────────────────────┐
│ Title: [The Glass Harbour____________]  │
│                                         │
│ Book type:  (•) Fiction  ( ) Non-fiction│
│                                         │
│ [Create and Start Writing]   [Cancel]    │
└─────────────────────────────────────────┘
```

The project should open into a useful writing/structure context. Fiction may show an empty outline ready for chapters/scenes; non-fiction may show an empty chapter outline without fiction-specific scene controls. The exact landing view is a design decision below.

### 8.2 Open and Import

Proposed Open Project treats a recognised WIDE project as one selectable object. Enter opens it; it does not show internal files. The shared file browser can still be used to choose a project directory.

Proposed normal Scrivener import: choose a source, let WIDE analyse and import into application-managed temporary working storage, then open the resulting project. An import summary and diagnostics remain available, but routine authors need not map binder items. Advanced review can remain an explicit action. A source folder is never a writable destination.

### 8.3 Save, Save As and close

Proposed visible state model:

```text
New/Imported working book ── autosave ──> Working copy
Working copy ── first Save ──> Choose permanent location ──> Saved project
Saved project ── Save As ──> Prepare new copy ── validate ── promote ──> New saved project
Any open project ── Close ──> Save / Discard / Cancel (when needed)
Unexpected exit ──> Detect working copy and offer Recover / Discard
```

The stored project ID and manuscript file integrity rules remain authoritative. The UI should show “Working copy”, “Saved”, “Saving…” or a clear conflict state and identify the project/document. It should not say “Saved” until durable persistence succeeds.

Open design question: should the author be able to create multiple temporary books in one session, and how should the project switcher distinguish them? This proposal does not change the existing lifecycle implementation.

## 9. Manuscript outliner proposal

The current requirement calls for one unified hierarchical outliner, while prior Scene Cards work contains useful visual features. The proposal is to make Cards a presentation of the same structure, not a competing outliner.

### 9.1 Candidate A — Tree-first outline with inspector

```text
┌ Write · Outline ─────────────────────────────┐
│ The Glass Harbour · Fiction · Working copy   │
├ Structure ─────────────────┬ Selected item ──┤
│ ▾ Part One                  │ Chapter 1       │
│   ▾ The Arrival              │ Synopsis        │
│     ◉ Opening at the quay    │ Status: Draft   │
│     ○ The locked room        │ Words: 824      │
│   ▸ Departure                │ Included: Yes   │
│ ▸ Part Two                   │ [Edit details]  │
│                              │                 │
│ Enter Open · Space Collapse │                 │
│ Insert Scene · Move · Include│                 │
└──────────────────────────────┴─────────────────┘
```

Advantages: compact; handles deep hierarchies and empty chapters; fast keyboard navigation; metadata can be inspected beside the tree.
Disadvantages: long titles and synopsis are constrained; many metadata columns can overwhelm terminal width; card-like planning is less visual.

### 9.2 Candidate B — Cards-first outline

```text
┌ Write · Cards ─────────────────────────────────────────┐
│ Included (8)                 Excluded (2)              │
│ Chapter 1: The Arrival                                  │
│ ┌ Scene 1 ────────────┐ ┌ Scene 2 ────────────┐         │
│ │ Opening at the quay │ │ The locked room     │         │
│ │ POV: Mara           │ │ Status: Outlined    │         │
│ │ 824 / 1,100 words   │ │ Synopsis: ...       │         │
│ └────────────────────┘ └──────────────────────┘         │
│ Chapter 2: Departure                                     │
│ ...                                                      │
└──────────────────────────────────────────────────────────┘
```

Advantages: makes scene purpose, synopsis, word target and inclusion visible; matches established scene-planning use.
Disadvantages: poorer fit for deeply nested non-fiction or direct chapter prose; a separate Included/Excluded collection can obscure original placement and hierarchy; terminal width limits simultaneous cards.

### 9.3 Candidate C — Unified tree with a card detail pane (proposal)

Keep the stable hierarchy as the navigation authority; selecting a row populates a detail/card pane with synopsis, status, word counts, POV, characters, plots, chronology and inclusion. A Cards view uses the same IDs, ordering operations and structural parents. This retains card capabilities and makes the tree the single structural control centre.

Advantages: preserves hierarchy and planning richness; one selection and command model; adaptable to fiction/non-fiction.
Disadvantages: demands more screen space; implementation must preserve current cards visual affordances; requires clear narrow-terminal behavior.

This is closest to the author's proposed universal hierarchical editor. The tree is the structural editor; the adjacent pane can show manuscript text, synopsis or metadata for the selected node. A node may have body text and children at the same time. Scene-specific planning remains optional and must not be forced on non-fiction authors. The scene/card view remains a projection over the same identities and structure.

#### Candidate structural-editing bindings

The author suggests these as possible outline-context defaults, subject to the existing keymap requirements and terminal portability:

| Operation | Candidate binding |
|---|---|
| Insert sibling/child | Insert, with the requested relationship made explicit |
| Open selected item | Enter |
| Toggle publication inclusion | Space |
| Demote/promote | Tab / Shift+Tab |
| Move within hierarchy | Alt+Up / Alt+Down |
| Collapse/expand | Left / Right |
| Rename | F2 |
| Delete | Delete with safeguards based on subtree contents |
| Return to originating view | Esc |
| Multi-selection | Operate on a selected set where the action supports it |

These are proposals, not approved bindings. A decision should check conflicts with current menu conventions and ensure every action also has a discoverable menu route.

### 9.4 State distinctions

The UI should keep these concepts distinct and explain them in the inspector:

- **Included / Excluded:** whether an item contributes to a chosen publication assembly/profile.
- **Unplaced:** a manuscript scene that has no current chapter placement.
- **Private:** research/planning content not included in publication by default.
- **Heading suppressed:** body may publish while its structural heading does not.
- **Archived/deleted:** recoverable lifecycle state, not a publication selection.

Per-profile overrides, inherited inclusion and the display of alternative back matter need a single understandable effective-state indicator. The precedence rules should be stated in the approved design after reviewing the latest issue contract.

The author explicitly prefers one structural editing model over separate chapter and scene navigation systems. Common actions should apply across node types where validation permits, while preserving node-specific behaviors. Fiction/non-fiction differences should primarily affect templates, terminology and optional metadata—not require separate editors.

## 10. Manuscript editor proposal

The editor should be treated as a word-processing canvas for canonical Markdown-backed content, not as an internal command console.

```text
┌ Write · Editor ──────────────────────────────────────────┐
│ The Glass Harbour  › Part One › Chapter 1 › Opening...  │
├──────────────────────────────────────────────────────────┤
│                                                          │
│ The tide turned beneath the old stone steps. Mara        │
│ tightened her coat and watched the harbour lamps...      │
│                                                          │
│                                                          │
├──────────────────────────────────────────────────────────┤
│ Saved · Scene · 824 words · Ctrl+S Save · Esc Outline    │
└──────────────────────────────────────────────────────────┘
```

Proposed display obligations:

- Breadcrumb, title and saved document identity all refer to the same item.
- Cursor, selection and active pane are visually distinct.
- Save state is persistent and never inferred from a transient message alone.
- Formatting marks, semantic formatting and literal source syntax have an explicit mode; toggling marks does not alter content.
- Undo/redo names the operation when available and does not cross silently into unrelated document state.
- Navigation from a search result or diagnostic preserves a route back to the originating location.
- Switching to an outline does not discard unsaved text or move the cursor to an unrelated document.

Whether the editor should default to rendered/WYSIWYM text, visible Markdown source, or a user-selectable hybrid is a significant design decision. The SRS and semantic authoring issues govern canonical representation and export, but the final editing presentation needs author review.

The author's stated preference is traditional word-processor editing with semantic formatting displayed in prose, and an explicit Reveal Codes/source view for inspecting canonical Markdown. The author also proposes individual-document editing, continuous chapter editing and eventually continuous manuscript editing. Continuous views would need to map cursor positions, saves, undo, diagnostics and inclusion back to stable source documents; they must be projections over canonical items rather than a concatenated replacement manuscript file. This is a substantial design choice and is not authorised for implementation yet.

Candidate editing choices for review:

- **Rendered semantic editor with Reveal Codes:** closest to the author's preference; has the clearest writing surface, but needs reliable source mapping and formatting controls.
- **Markdown source editor with formatting shortcuts:** easiest to preserve source text and map current files; risks exposing markup as ordinary prose-editing friction.
- **Hybrid/editor mode toggle:** balances prose and source inspection; risks mode confusion and differing cursor behavior.

The proposal favours evaluating the rendered editor and explicit source inspection against actual author workflows, but no mode is selected here.

### 10.1 Pane flexibility

The author proposes that any pane can display any supported view and that splitting the focused pane adds a pane to the same project context. The shared pane model would include editor, outline, cards, synopsis, inspector, research, timeline, characters, diagnostics, search and project management. The alternative is retaining established fixed layouts and augmenting them only with named presets. Flexible panes offer the Emacs/GNU Screen model the author prefers, but increase focus and layout-state complexity. Both options must preserve current split-pane functionality and stable document selection.

## 11. Menu and command organization proposal

The command registry is an implementation and reuse mechanism, not a menu specification. The author-facing menu should expose task language, avoid raw identifiers, and keep visible lists short enough to scan. A search/command palette can remain for infrequent actions without becoming a prerequisite for common writing.

### 11.1 Candidate menu layout

**Option A — Keep the current root menu:** File, Edit, Insert, Navigate, View, Review, Publish, Tools, Help.
Advantage: continuity and recognizable word-processor roots.
Cost: commands overlap by context and several roots remain broad.

**Option B — Task-first roots:** File, Write, Plan, Research, Review, Publish, Help.
Advantage: author tasks are visible; fewer peer categories.
Cost: departs from conventional Edit/Insert/Navigate/View organization; reclassification affects shortcuts/help/testing.

**Option C — Conventional hybrid (proposal):** File, Edit, Insert, Navigate, View, Review, Publish, Help; place project-wide specialist setup under Tools or a searchable “More commands” destination. Contextual menus expose only actions relevant to the focused item.

Advantage: familiar to word-processor users and broadly consistent with current roots; reduces routine clutter without removing commands.
Cost: still has eight roots; the boundary between Review, Tools and Publish needs a command-by-command audit.

The author's proposed roots are **File, Edit, Insert, Format, Navigate, View, Tools, Help**, with Publish either a prominent operation or a distinct root. Proposed File actions include New Fiction Project, New Non-fiction Project, Open Project, Open Recent, Import, Save, Save As, Close Project, Publish and Exit. The established `Publish` root and current issue contracts must be reconciled before selecting a final model; this draft does not remove it. A compact menu can keep common author actions visible while leaving the full advanced registry searchable in the command palette and exposing context-relevant actions in local menus.

### 11.2 Illustrative concise hierarchy

```text
File       Edit       Insert       Format       Navigate       View       Tools       Help
 ├ New Book  ├ Undo     ├ Chapter    ├ Bold         ├ Find          ├ Outline  ├ Preferences ├ Resume Tour
 ├ Open      ├ Redo     ├ Scene      ├ Italic       ├ Next Match   ├ Cards    ├ Keybindings ├ Context Help
 ├ Import    ├ Find     ├ Section    ├ Paragraph    ├ Previous Doc  ├ Editor   ├ Layouts     ├ Help Topics
 ├ Save      ├ Replace  ├ Note       └ Reveal Codes └ History       └ Panes    └ More…       └ Shortcuts
 ├ Save As   └ Prefs
 ├ Publish…
 └ Close
```

This is a proposal, not a final menu inventory. The exact set must be reconciled with `menu.md`, `menu-audit.md`, the latest #54/#49 amendments and user-approved command visibility. Existing menu commands must not disappear merely because their placement changes.

## 12. Dialogue standards

Dialogs should be short, task-specific and explain what changes. A dialogue should not ask for a UUID or require command grammar in an ordinary workflow.

```text
┌ Move “The locked room” ───────────────────────────┐
│ Move this scene to:                               │
│   Part One / Chapter 2 — The Departure            │
│                                                   │
│ Its text, notes and relationships will stay with it.│
│                                                   │
│ [Choose Chapter]  [Move]  [Cancel]                │
└───────────────────────────────────────────────────┘
```

Proposed conventions:

- Enter activates the focused button/choice; Esc cancels unless cancellation would lose data, in which case the unsaved-state contract is shown.
- Errors remain visible in the dialog and identify how to recover.
- Non-blocking warnings do not masquerade as errors or prevent an otherwise valid operation.
- Destructive or structural actions state their scope, affected children and recovery path.
- Defaults retain content and cancel safely.
- A completed action returns to the affected item and shows the outcome.

## 13. Visual and status language proposal

Use a small, consistent set of indicators throughout Outline, Cards, editor and compile review.

| Meaning | Proposed representation | Accessibility requirement |
|---|---|---|
| Current selection | Inverse/highlighted row | Must remain clear in monochrome |
| Keyboard focus | Distinct border or focus marker | Never depend on colour alone |
| Included | Explicit `Included` or compact check state | Distinguish from selection |
| Excluded | Explicit `Excluded` label or stable symbol | Do not hide item or imply deletion |
| Unplaced | `Unplaced` label | Explain that it remains in the project |
| Dirty/unsaved | `Unsaved` / `Saving…` | Persistent until durable save completes |
| Conflict | `Conflict` with visible resolution route | Must not be confused with ordinary dirty state |
| Background work | Named operation plus progress/activity | Provide cancel where safe |
| Warning/error | Severity label, count and navigation | Distinguish editorial suggestion from compile failure |

The author suggests these compact monochrome-safe glyphs: `✓` included, `×` excluded, `▸` collapsed, `▾` expanded, `*` unsaved, `!` attention/error and `~` work in progress. These are candidate symbols, not approved glyphs. Terminal encoding, narrow layouts and assistive use must be checked. Focus, selection, inclusion and document status remain independent states even when represented compactly.

Proposed status hierarchy: project state, current manuscript location, active task/workspace, and the most relevant keyboard actions. The footer should not repeat command inventories, long documentation or internal registry output.

## 14. Background operations and feedback

Long imports, compile, search indexing, EPUB generation and analysis should acknowledge the command immediately. The author should see what operation is running, its current stage and whether cancellation is safe. Cancellation should leave the source project intact and either remove an incomplete destination or report its recoverable state.

Proposed feedback sequence:

```text
Importing “Arrow Making” · Reading binder (12 of 39 items)
Destination: Temporary working copy
[Esc Cancel]
```

On success, open the resulting book and show a concise summary with a route to detailed import diagnostics. On failure, restore the prior workspace and explain whether a temporary copy was retained for recovery.

## 15. Author workflow proposals

These are acceptance journeys for a future approved design; they are not current claims that every journey passes.

### 15.1 New fiction

New Book → Fiction → project opens → add chapters/scenes from the outline → open a scene → write → autosave → Save/Save As → return to outline at the same selection → close/reopen → continue.

### 15.2 New non-fiction

New Book → Non-fiction → chapter outline → add/reorder chapters and subsections → edit chapter prose directly → include illustrations/structured material → save/reopen → compile without creating scenes.

### 15.3 Existing Scrivener manuscript

Import → choose `.scriv` → automatic analysis/import into a temporary working copy → open usable outline → inspect optional warnings → edit → save/reopen → select publication profile → compile and inspect output.

### 15.4 Restructure

Select item → move/reorder/promote/demote or split/merge where supported → see the result in the same outline → undo or restore if mistaken → verify order and identity after reopen and compile.

### 15.5 Writing session

Open last document → confirm breadcrumb and save state → navigate by document/word/paragraph or search → edit → save/autosave feedback → switch to reference → return to exact editor location.

### 15.6 Publication

Publish → choose profile/edition → inspect selected front matter, manuscript items and back matter → review warnings/errors → choose output format and destination → compile → open/preview artifact → navigate any diagnostics back to source.

### 15.7 Research and story development

Create or import a research note → link it to an entity/passage by selection and name → navigate back from the manuscript → maintain privacy/inclusion distinction. Characters, locations and plotlines should be authorable from named records and pickers rather than identifiers.

### 15.8 Editorial review

Find or run proofread → inspect a source-linked diagnostic → accept, edit, ignore or dismiss explicitly → undo if changed → rerun checks without altering unrelated prose.

### 15.9 Recovery

Unexpected exit → identify recoverable project and last saved state → preview available version/conflict → recover or discard deliberately → reopen at a useful location → verify content and structure.

### 15.10 Project management

Create/open/import → switch among books without exposing project internals → save as a copy → close with Save/Discard/Cancel where needed → reopen exact project.

## 16. Proposed cross-feature invariants

The following candidate invariants are central to the review. They are recommendations until approved:

1. **Identity agreement:** title, breadcrumb, editor buffer, save target and diagnostic target refer to the same active item.
2. **No silent success:** a command reports success only after the requested state change is visible and persisted as required.
3. **Selection safety:** an operation never acts on a hidden or stale selection without showing its target.
4. **State preservation:** returning from a modal or reference view restores the previous selection, cursor and scroll where the source still exists.
5. **One structure:** alternate outline/card/timeline views project the same stable manuscript items; none silently changes manuscript order.
6. **Content retention:** movement, exclusion, deletion and profile changes preserve source content unless an explicit recoverable deletion is confirmed.
7. **Publication privacy:** planning and research material is excluded by default; effective publication inclusion is inspectable for the selected profile.
8. **Visible persistence:** “Saved” means durable save succeeded; unsaved/conflicted data stays visibly distinct.
9. **Progress feedback:** operations that take noticeable time acknowledge immediately and expose errors without blocking the terminal silently.
10. **Author vocabulary:** normal workflow avoids UUIDs, SQL terms, object-store terminology and command syntax.
11. **Feature preservation:** extending a view or workflow retains its established capabilities unless an approved design decision explicitly supersedes them.
12. **Keyboard parity:** any operation required for an author journey has a discoverable keyboard path; mouse support never becomes mandatory.
13. **Terminal resilience:** resizing, narrow widths, monochrome, SSH and tmux do not turn status or content into a wall of diagnostics.
14. **Undo/recovery distinction:** immediate undo, persisted revision history and crash recovery are presented as related but distinct protections.

### Author-stated design exclusions and architecture question

The author rejects the following as outcomes for the proposed interface: a mandatory obstructive dashboard; separate fiction and non-fiction editors; separate inconsistent scene and chapter navigation systems; a primary menu listing every backend operation; UUIDs in ordinary dialogues; compulsory import mapping; independent selection state in each manuscript view; feature-specific keyboard conventions; and replacing established views whenever requirements expand.

The author identifies a possible **shared interaction controller** as an architectural question: focus, selection, active document identity, navigation history, command dispatch and workspace state may be inconsistently owned by individual views. The proposal does not claim this is a confirmed defect or prescribe a new subsystem. If the design is approved, implementation review should trace these responsibilities through existing app services and UI state first, then make the smallest correction supported by evidence.

## 17. Important decisions for author review

No decision below has been made. The author can accept, reject, combine or replace any option.

| Decision | Alternatives | Provisional design view for discussion | What approval would affect |
|---|---|---|---|
| First view when opening a book | Outline; editor at last location; dashboard/Write landing | Write workspace showing outline and last/selected document | Workspace transitions and session restoration |
| Default writing layout | Editor only; outline + editor split; editor with toggleable outline | Outline + editor at standard widths, editor-only at narrow widths | Screen layout, focus and resizing |
| Primary outline presentation | Tree; cards; unified tree with details/cards projection | Unified tree, Cards as alternate view | Visual design and preservation of card behavior |
| Scene Cards inclusion display | Separate Included/Excluded collections; same hierarchy with state; both filtered projections | Preserve hierarchy and offer filters for included/excluded | #55 interaction and collection semantics |
| Menu model | Current conventional roots; task-first roots; conventional hybrid | Conventional hybrid pending command audit | Menu taxonomy and shortcut/help labeling |
| Command search | Expert fallback; universal primary surface; both | Secondary route, never required for common workflow | Discoverability and command visibility |
| Editor presentation | Markdown source; rendered semantic prose; hybrid/toggle | Hybrid needs prototype; no choice recommended without author preference | WYSIWYM, formatting marks, editing model |
| Focus movement | Tab between panes; explicit pane keys; both | Both, with consistent visible focus | Keyboard map and pane behavior |
| Project lifecycle | Ask permanent location first; temporary then first Save; current approved temporary model | Keep temporary-first lifecycle | New/Open/Import/Save workflows |
| Save feedback | Footer only; persistent label plus footer; save indicator in title and footer | Persistent label plus concise footer cue | Status display and recovery prompts |
| Inclusion precedence | Explicit profile value; inherited default with override; assembly selection | One visible effective value with source of override explained | Compile model presentation |
| Empty-project start | Blank outline; guided first-action landing; auto-create first chapter | Blank but actionable outline without automatic fictional content | Onboarding and project creation |
| Help/tutorial | Separate searchable help and resumable lessons; integrated contextual help | Keep help and lesson progress separate but mutually linked | Help navigation and keybinding conflicts |

## 18. Screen-state mock-ups

These mock-ups illustrate possible interaction states only; they do not define pixel-perfect UI or approved labels.

### 18.1 Fiction project in writing context

```text
 File  Edit  Insert  Navigate  View  Review  Publish  Help
 The Glass Harbour · Fiction · Working copy
┌ Outline · Included ─────────────┬ Editor ─────────────────────────────┐
│ ▾ Part One                      │ Part One › Chapter 1 › Opening       │
│   ▾ Chapter 1: The Arrival      │                                      │
│     ◉ Opening at the quay       │ The tide turned beneath the old     │
│     ○ The locked room           │ stone steps. Mara watched the lamps…│
│   ▸ Chapter 2: Departure        │                                      │
│ ▸ Part Two                      │                                      │
├────────────────────────────────┴──────────────────────────────────────┤
│ Focus: Outline · Selected: Opening at the quay · Included · Saved     │
│ Enter Open · Space Collapse · Insert Scene · F10 Menu · Ctrl+S Save   │
└───────────────────────────────────────────────────────────────────────┘
```

### 18.2 Non-fiction chapter prose

```text
 File  Edit  Insert  Navigate  View  Review  Publish  Help
 Field Notes on Urban Gardens · Non-fiction · Saved
┌ Outline ────────────────────────┬ Editor ─────────────────────────────┐
│ ▾ Introduction                  │ Chapter 1 › Choosing a Site          │
│ ▾ Part I: Foundations           │                                      │
│   ▾ Chapter 1: Choosing a Site   │ Before choosing plants, observe the │
│     § Chapter text               │ light across the site...             │
│     ▸ 1.1 Soil and drainage      │                                      │
│   Chapter 2: Water              │                                      │
└─────────────────────────────────┴──────────────────────────────────────┘
```

### 18.3 Import summary

```text
┌ Scrivener import complete ─────────────────────────────┐
│ The Arrow Maker · Working copy                          │
│ 8 folders · 27 manuscript documents · 4 research items  │
│ Source was not changed.                                 │
│ 3 items need review: two unknown labels, one missing asset│
│                                                        │
│ [Open Book]  [Review Import Notes]  [Discard Copy]       │
└────────────────────────────────────────────────────────┘
```

### 18.4 Compile profile

```text
┌ Compile Book ────────────────────────────────────────────┐
│ Edition: [Kobo eBook ▼]    Format: [EPUB 3 ▼]            │
│ Front matter: [Kobo front matter              Change]    │
│ Manuscript:   18 chapters · 54 scenes · 2 excluded       │
│ Back matter:  [Kobo back matter                Change]    │
│ Output:       [The Glass Harbour.epub          Browse]    │
│                                                         │
│ [Review Selection]  [Compile]  [Cancel]                  │
└─────────────────────────────────────────────────────────┘
```

## 19. Adoption and review process proposed

If the author approves a direction, the next step should be a gap analysis against the current application, not an immediate rewrite. That analysis should:

1. Map each approved interaction invariant to existing code and tests.
2. Identify visual behavior that must be preserved (especially current outline/card indicators, command registry and menu keyboard model).
3. Separate defects from design changes.
4. Break any approved changes into small, reversible milestones.
5. Define PTY acceptance paths before implementation.
6. Retain existing author UAT issues until their human acceptance criteria pass.

Until that approval, this proposal should remain a standalone review artifact. Existing interface and implementation remain the baseline.

## 20. Review checklist

The author may use this checklist during review:

- [ ] Choose or revise the primary workspace organisation.
- [ ] Choose the landing view for a newly created and reopened book.
- [ ] Confirm whether Outline and Cards are two presentations of one unified outliner.
- [ ] Confirm how Included, Excluded, Unplaced, Private and Heading Suppressed should appear.
- [ ] Choose the ordinary menu model and command-search role.
- [ ] Decide whether prose should default to rendered/hybrid or source-style editing.
- [ ] Confirm focus and keyboard conventions, including Tab, Enter and Esc.
- [ ] Confirm project lifecycle presentation for temporary work, Save and Save As.
- [ ] Confirm the proposed cross-feature invariants or supply replacements.
- [ ] Identify any existing visual or interaction behavior that must be preserved exactly.
- [ ] Review latest GitHub amendments not available during this draft.

---

**End of proposed WIDE User Experience and Interaction Design Specification v1.0.**

# WIDE — User Experience and Interaction Design Specification v1.0

Status: Author-approved, 27 November 2026. Normative for interaction behaviour. Implementation and human acceptance remain outstanding.

## 1. Authority and scope

WIDE is a hierarchical document editor with integrated novel-development tools, local-first, keyboard-first and terminal-first. Fiction and non-fiction share the same hierarchy and editor, with different templates and relevant planning metadata. The default workspace is outliner plus manuscript editor; Dashboard is optional. Refactor incrementally, not by rewriting the application. Preserve all previously approved functionality and visual information unless explicitly superseded here. No unrelated feature expansion; deferred #75 remains deferred.

This specification supersedes conflicting interaction proposals, including the rejected cascading-menu design. The SRS and engineering specifications continue to govern functional scope and technical invariants. Preserve stable IDs, one SQLite database per book, portable canonical UTF-8 Markdown, application-service mutation boundaries, transactional ordering, durable write journal, conflict recovery, provenance, separate optional chronology and renderer-neutral Compile IR. Do not infer permission to weaken these invariants.

## 2. Manuscript hierarchy (HIER)

HIER-01: Flexible typed tree, with Book root, Part, Section, Chapter, Scene, Document and Folder. Parts/sections/chapters can contain prose and children; Documents can contain prose and children; Folders organise children without compulsory headings. Scenes are leaves with narrative metadata. Arbitrary applicable structural depth, no cycles. Front matter, back matter, research and unplaced scenes are logical collections with distinct publication policies.

HIER-02: A container's own prose precedes its children. Independently ordered prose documents can be interleaved with scenes and structural children. No fabricated scenes for non-fiction. Preserve stable identity, metadata, links and content when moving or renaming. Type conversion must preserve data; reject incompatible conversions safely or request explicit resolution. Scene placement is singular; unplaced scenes remain accessible. Heading visibility and generated numbering are independent of title and structural identity.

HIER-03: Common structural operations: insert before/after/as child, rename, move, duplicate, promote/demote, include/exclude, recoverable delete. Validate destinations, preserve descendants, provide explicit handling of subtree removal and undo/recovery. Newly created items must be visible and selected; reveal their location even when filters would hide them. Use author terms, not internal content-unit or UUID terminology.

## 3. Outliner (OUT)

OUT-01: One unified hierarchical outliner, with compact/detailed/progress presentations as configurations, not separate implementations. Configurable, reorderable, resizable columns and named presets. Title/hierarchy and inclusion always visible; status, word count and target default visible where space permits. Optional progress, synopsis, POV, characters, location, story time, plotline, labels, custom metadata, diagnostics and modification date. Preserve essential identity and inclusion at narrow widths.

OUT-02: Maintain total written and effective publication counts separately, including aggregate counts on collapsed containers. Derive counts consistently from canonical content and active profile; do not double-count. Excluded items visible by default, visually distinguished.

OUT-03: Independent indicators for expansion (▾/▸), inclusion (✓/×), mixed descendant inclusion (◐), not applicable (—), unplaced (?), unsaved (*) and diagnostic (!); selection, focus and writing status remain distinct. Provide monochrome and ASCII equivalents. Container inclusion and expansion must not share a single indicator.

OUT-04: Hoisting temporarily isolates a branch. Filters include status, inclusion, characters, plotline, location, labels and custom metadata. Metadata sorting is presentation only, never manuscript reordering. Explicit structural commands alone change canonical order.

OUT-05: Scene Cards are an alternative presentation of the same manuscript identities, hierarchy, order and inclusion, with synopsis, metadata and progress. Preserve existing rich cards and optional Included/Excluded grouping and filters. No separate conflicting state.

## 4. Workspace (WS)

WS-01: Project owns content; workspace is an arrangement of panes; pane is independently focused; view is a presentation. No task-specific application modes. Every registered author-facing view must provide the same functional capabilities whether displayed full-screen or within a split pane. Pane size may affect presentation, but not functionality. Four visible panes maximum for v1. Flexible recursive horizontal/vertical splits, resize, swap, balance, close, change view, cycle focus, maximise any pane and restore exact previous layout. Closing last pane restores a default view, not project closure. A static or read-only projection is supplementary and never substitutes for the selected view's functional implementation.

WS-02: Built-in Writing (outline/editor), Planning, Research, Review and Focus presets, plus user-defined named presets. Views include all existing approved destinations (outline, editor, cards, inspector, timeline, research, characters, diagnostics, search, plotlines, story bible, statistics, etc.). Presets do not restrict commands.

WS-03: Panes can follow selection, be pinned to a stable document/entity, or maintain independent navigation. Inspector normally follows; reference editor may pin; research browser may be independent. Multiple editors of one document share a single editable buffer, revision, undo and dirty state; each has independent cursor, selection and scroll. Rebase positions safely after edits. One persistence/conflict mechanism per document.

WS-04: On resize, temporarily collapse panes that cannot fit, retaining their state and making them accessible; restore arrangement automatically when space permits. Define per-view minimum dimensions in engineering design.

WS-05: Persist application preferences/presets, project workspace/outline expansion/profile, and session document/cursor/focus/scroll/pinned references. Restore last useful context when reopening. New/imported projects open Writing. Multiple open projects have isolated workspaces, buffers, history and save state. Only active project editable; cross-project references read-only for v1. Switching never discards dirty work.

## 5. Keyboard and dispatch (KEY)

KEY-01: WIDE Standard, dedicated WordStar profile and user-defined profiles over one command system. Full contextual rebinding, multiple bindings, conflict detection, reset, import/export. Display effective bindings in menus/help/status. The WordStar profile provides its non-conflicting classic navigation, block marking/copy/delete and multi-key commands, including Ctrl+K and Ctrl+Q sequences. Where a legacy WordStar chord conflicts with an author-approved application command (including Ctrl+S Save or Ctrl+X Cut), the application command takes precedence and the equivalent standard arrow/menu action remains available. Pending prefixes visibly show available continuations; Escape cancels.

KEY-02 (author-approved keyboard standard, comment #7227291073): No essential author workflow may require a function key. Standard global operations use Ctrl+N/O/S/W/Q for new/open/save/close/quit; Save As remains available through File. Editing uses Ctrl+Z/Y for undo/redo, Ctrl+X/C/V/A for cut/copy/paste/select all, Ctrl+F/H for find/replace, Ctrl+B/I/U for formatting, Ctrl+G for go to, and Ctrl+/ for the menu. The File/View/Help menus and ordinary dialogues provide direct Enter/Esc/Tab/arrow/Space routes wherever terminal encoding or a conflicting editor context makes a Ctrl shortcut unsuitable. F-keys may remain optional accelerators and must never be the only way to confirm, create, import, publish, retry, skip, select a view, or complete another ordinary operation. Resolve Ctrl+H versus Backspace according to the terminal event and active context; never consume ordinary editing keys as menu syntax. Function-key alternatives remain discoverable but secondary. Global commands take precedence over Standard, WordStar and Custom editor-profile defaults.

KEY-03 (U37 approved refinement, supersedes prior binding proposal): In Outline, Up/Down select and Left/Right collapse/expand or move through hierarchy; Enter opens the selected item; Space toggles inclusion where applicable. Insert creates a scene, Shift+Insert creates a chapter, and Ctrl+Shift+Insert creates a section at the selected structural context. Shift+Up/Down moves the selected item earlier/later; Shift+Left/Right promotes/demotes structural items. Delete performs recoverable removal. Ctrl+R renames the selected applicable document or structural item. Tab retains ordinary editor indentation and dialogue-field navigation. Pane navigation is provided by F6/Shift+F6 or by activating the window prefix described in the UAT 40 refinement below. The editor retains ordinary text entry and its independent cursor/scroll. Effective inherited bindings must appear in menus and contextual help; user overrides remain authoritative.

KEY-06 (author-approved UAT 40 refinement): The default window-management prefix is Ctrl+Backslash, followed by a separate keypress. In conventional terminal input this is ASCII 0x1C; Crossterm decodes it as Ctrl+4, so the two physical chords are indistinguishable on the baseline protocol. WIDE displays the canonical form `Ctrl+\`; `Ctrl+4` is accepted as an alias when configuring it. A user-defined binding for the same decoded event takes precedence over the default prefix and is never silently replaced. A user who needs Ctrl+4 for another command must rebind the window prefix. The prefix is configurable. After activation, Up/Down/Left/Right focus the nearest pane in that direction; 1–4 focus stable visible pane numbers; Tab/Shift+Tab focus next/previous pane; H/V split; X closes the focused pane; M maximises/restores; P selects a saved workspace layout; ? shows help; Esc cancels. Available actions are shown while the prefix is pending. Unsupported sequences are consumed without changing manuscript content. The View menu and F6/Shift+F6 remain alternatives. Direct Alt+Arrow navigation is not the default because host terminal emulators may intercept it.

KEY-07 (shortcut notation invariant): In labels and configuration, plus signs separate keys pressed simultaneously; `Ctrl+Shift+O` means the three physical keys Control, Shift and O. A comma separates sequential chords: `Ctrl+\, 1` means press Control+Backslash, release both, then press 1. A literal plus key is named `Plus` (for example, `Shift+Plus`). Shortcut parsers, dispatch, formatting, menus, help and prefix handling must use this same interpretation. The prefix workflow must work from the single input byte 0x1C followed by an independent key event; it must never wait for or consume a literal plus as syntax.

KEY-08 (author-approved universal control behaviour): Enter confirms or activates the focused choice; Esc cancels or returns to the originating view; Tab/Shift+Tab advances or reverses control focus; arrows navigate choices and lists; Space toggles or activates an appropriate control; Home/End move to the contextual start/end. Apply this to pages, panes, pop-ups, dialogues, file browsers, selection lists and confirmations. Editor focus keeps normal manuscript Enter, Tab and Space behaviour. No ordinary author operation may be available only through an F-key.

KEY-04: One typed command registry and dispatch path for menu, palette, shortcut and contextual action. Precedence: modal dialogue/menu; pending prefix; reserved global commands; focused view; remaining application keymap; printable input. Exactly one handler. Resolve project, pane, selected object and active document explicitly, validate availability before mutation. No dispatch by display label. Opening menus preserves originating context and cursor.

KEY-05: Traditional terminal baseline, optional enhanced keyboard protocols. Audit Ctrl+I/Tab, Ctrl+M/Enter, Ctrl+H/Backspace, Alt/Escape sequences, modifier/function-key limitations, Ctrl+S flow control, SSH/tmux interception. Every essential action has a reliable fallback. Mouse optional with full keyboard parity.

KEY-09 (issue #62 navigation-history contract): Navigate exposes Return to Previous Position and Return to Next Position as paired history actions. They restore the manuscript document and cursor position left by the opposite action. An empty history boundary reports a concise, non-fatal message and preserves focus, cursor and unsaved text. History movement requires a clean editor buffer; the author must save before switching documents, and a missing or ambiguous passage leaves the current document unchanged.

## 6. Menus (MENU) — supersedes rejected cascading proposal

MENU-01: Persistent conventional root menus File, Edit, Insert, Format, Navigate, View, Tools, Help. Add dynamic context-specific root menus based on focused pane: Outline, Scenes, Timeline, Characters, Research, Diagnostics and equivalent relevant views. Focus, not mere pane visibility, determines context. Preserve originating context while menu is open.

MENU-02: All drop-downs flat, readable, no cascading submenus. Complex choices use dedicated compact/searchable dialogues. No 44-entry catch-all Edit menu. Persistent menus retain general responsibilities and may adapt relevant entries to context. File project lifecycle/persistence/publication; Edit existing content; Insert prose material only; Format semantic formatting; Navigate movement; View panes/workspaces/views; Tools general utilities; Help assistance.

MENU-03: Outline menu owns structural and manuscript-document creation, rename, move, promote/demote, inclusion, hoist and recoverable deletion. A single context-aware New Chapter action replaces duplicate chapter-in-section commands. These actions must not be in prose Insert. Insert contains date/time, picture, table, footnote, special character, scene/page break, snippet and other approved prose insertion. A chapter's direct prose opens through the chapter itself; it is not displayed as a duplicate child row. Excluded documents remain in place and display an `[excluded]` suffix; the optional Excluded view remains available. Other context menus own their object-specific operations. Example labels are illustrative; reconcile actual registry before restructuring.

MENU-04: Advanced author commands remain discoverable through command palette. No capability removed merely because absent from a menu. Reconcile every existing registry entry to a visible contextual route, palette, alias, internal-only classification or explicit consolidation; record rationale and tests. Do not expose implementation internals. Use unified dispatch, meaningful availability and recovery explanations. Keyboard shortcuts supplement menu commands; they do not replace them. Ordinary authoring operations must remain accessible through named menu actions. Demonstrate flat dynamic menu rendering and focus transitions in production terminal before broad menu migration. Rejected submenu hierarchy, menu density quotas and earlier fixed menu-content proposal are not normative.

## 7. Manuscript editor (ED)

ED-01: Semantic word-processor presentation by default; canonical Markdown is implementation detail. Optional Reveal Codes/source inspection and formatting marks. Support approved inline styles, paragraphs, headings, lists, tables, images and annotations; formatting commands affect selection or subsequent typing. Preserve exact canonical source where no edit is made; unsupported constructs diagnosed, not silently lost.

ED-02: Edit individual document, chapter, section or entire manuscript. Continuous editing displays ordered units with visible boundaries, preserving stable identities and independent persistence; no accidental merges. Document-level shared undo/redo. Structural undo remains available in appropriate context; no regression of existing recovery.

ED-03: Autosave in temporary and permanent projects. Restore cursor, selection and scroll on return. Title, breadcrumb, buffer, save target and status refer to same identity. Escape returns to originating selection. Date/time and other insertion target active editor cursor or are unavailable; never stale buffer. Find/replace document, branch or manuscript with optional regex and safe scoped operations.

ED-04: Inline proofreading indicators plus navigable diagnostics, including common capitalisation/typing errors. Document-specific proofreading language independent of UI language. Configurable focus-mode text width. Internal clipboard and system clipboard where supported. Preserve Unicode and semantic formatting across edit/save/reopen/compile.

## 8. Publication (PUB)

PUB-01: File > Publish or F9 opens named reusable profiles for existing approved formats. Default manuscript inclusion plus per-profile overrides; container inclusion inherited unless explicitly overridden. Shared effective inclusion resolver for outline, cards, counts, preview and compiler. Explicit descendant override must not bypass private/research or Trash safety boundaries.

PUB-02: Select front/back matter independently per profile. Configurable heading visibility, numbering, formatting and scene separators. Preview effective order, headings and inclusion before publishing. Resolve project/manuscript variables; unresolved values diagnosed. Compiler-style errors/warnings/information; truthful completion and output location.

PUB-03: Research and private metadata excluded unless explicitly selected through an appropriate publication mechanism; never silently publish private notes, character sheets or Trash. Publishing is non-mutating, including profile-specific suppression of bodies/headings. Preserve supported Scrivener settings and report unsupported settings. Use common Compile IR, semantic structural comparisons and independent format validation; EPUBCheck alone is insufficient. Retain existing approved formats and publication compatibility requirements.

## 9. Application shell and lifecycle (SHELL)

SHELL-01: Startup restores previous session, optional recent-project selector. New project compact title/type choice (or separate fiction/non-fiction actions), no filesystem selection. Import selects source, analyses/maps/validates automatically and opens manuscript directly in WIDE-managed temporary working storage. No mandatory review, destination or Open Book confirmation. Advanced mapping/report optional; genuine blocking ambiguity requires clear author decision. Source read-only, Trash ancestry excluded, private notes nonpublishing. Imported book type visible/editable without structural rewrite.

SHELL-02: Temporary project is a real working project with normal SQLite/files/assets/journal, autosave and recovery, not a second in-memory model. Use durable application-managed staging, not volatile OS temp. First explicit Save requests permanent location; Save As copies complete project safely. Publish from temporary projects. Close unsaved project or exit offers Save/Discard/Cancel. Crash/SSH disconnect recovery preserves work. Open Project recognises project root and opens it without exposing internal files as normal navigation. Never overwrite source or existing project silently.

SHELL-03: Status bar shows active document, focus, word count, save state, relevant effective shortcuts and latest outcome; narrow width prioritises critical outcome/identity/save state. Compact consistent keyboard-accessible dialogues. Routine notifications non-blocking; errors explain failure, cause and recovery actions. No raw UUID dumps as ordinary author feedback.

SHELL-04: Long operations show progress/stage and elapsed time, remain responsive and cancellable where safe; no false success or frozen appearance. Themes configurable, monochrome and ASCII fallback, responsive terminal layouts, SSH and optional mouse. Context-sensitive help and effective bindings.

## 10. Mandatory cross-feature acceptance (ACC)

- Preserve existing capabilities and visual information; explicitly identify approved supersessions.
- New items visible and selected; correct stable target for every operation; truthful success only after actual success.
- Return from editor restores originating selection and scroll. No stale buffer writes or cross-project mutation.
- Temporary and saved projects have equivalent authoring capabilities. Dirty work and external edits preserved; recovery is explicit.
- Long-running operations visibly progress; no silent data loss, publication leakage or source modification.
- Every author-facing command tested through production terminal interface in applicable contexts; registry inventory alone is not workflow acceptance.
- Test complete author journeys: new fiction/non-fiction, create/restructure/edit/include/save/reopen, import protected source/edit/publish, continuous editing and shared buffers, pane focus/resize/pin/restore, project switching, research/story links, diagnostics and proofreading, revision/backup/recovery, publication profiles and privacy. Verify rendered state, actual persisted data and output; include narrow/wide terminals, empty/populated and temporary/saved projects.
- Maintain synthetic public tests and protected local real-manuscript regressions. Never commit private source, generated publications or content-bearing reports. Preserve source checksums.
- Run fmt, warnings-denied Clippy, workspace tests and production PTY journeys; package isolated frozen UAT builds with checksums and checklists. Automated evidence and human acceptance are distinct; #44/#76 remain open until genuinely accepted.

## 11. Reconciliation and implementation

Reviewed repository sources: engineering-design-v0.1.md, engineering-design-v0.2.md, software-requirements-v0.2-addendum.md, engineering-design-v2.43-addendum.md, wysiwym-contract-v0.3.md and DEVELOPMENT.md. Original SRS v0.1 is explicitly reported absent from repository by its addendum; direct full reconciliation against that original and the entire engineering addendum series remains required. This document does not claim that audit has been completed.

| Existing contract / issue | Resolution |
|---|---|
| Engineering v0.1/v0.2 strict section/chapter/scene placement and chapter-only triggers | HIER defines author-facing flexible typed hierarchy. Audit current schema/services and propose safe migrations/adapters where required; preserve stable identity, acyclicity, transactional order, singular placement and recovery. Do not disable constraints blindly. |
| SRS addendum first-run destination, immutable BookType, fiction Cards/non-fiction Outline | SHELL supersedes these interaction rules: temporary storage, editable type, common Writing default. |
| Engineering v0.1 §8 global scene selection and separate workspaces | WS replaces with explicit per-pane contexts and shared buffers. Read-only projections may supplement a view but cannot replace its full functionality in a pane. |
| SRS TUI-002/003, #8/#49 hierarchical menus | MENU supersedes cascading/hierarchical menu presentation only; preserve typed registry, Ctrl+/ and F10. |
| #25 one to three panes, #13/#47 split views | WS authorises up to four visible panes and flexible splits. |
| #31 no fixed editor profiles, #22/#32 | KEY provides optional predefined profiles, never fixed or mandatory; custom bindings remain authoritative. |
| WYSIWYM v0.3, #21/#29/#61 | ED aligns with canonical Markdown and semantic projection; normal editor is semantic, source optional. Preserve source mapping and journal. |
| Engineering v2.43 structural undo, #55/#63 | Retain recoverable structural undo/redo and visible feedback; ED context determines text versus structural undo. |
| #7/#16/#53/#67/#68/#73 | PUB and SHELL govern publication UI, import lifecycle, inclusion and fidelity; preserve backend scope. |
| #44/#76 | Existing release gates remain; this specification supplies normative interaction requirements and author journeys. |

Implementation order: (1) reconcile all specifications and registry; record gap matrix and migration risks; (2) stabilise shared state/dispatch and project lifecycle; (3) prove dynamic flat menus and unified outline; (4) workspace/pane/buffer management and keyboard profiles; (5) semantic editor and continuous editing; (6) publication and shell integration; (7) full regression and human UAT. Dependencies and already implemented work may justify smaller reordered milestones; document rationale. Fix data-integrity blockers immediately. Do not rewrite or duplicate working services. Exact key conflicts, pane minima, menu-entry classification and schema migration mechanics are engineering deliverables, not permission to invent new UX. If a material design ambiguity remains, record it and continue independent work. Commit verified milestones, update issues and DEVELOPMENT_STATUS.md, continue persistent v1 goal until actionable work exhausted or hard blocker/limit. No voluntary stop at a routine milestone.

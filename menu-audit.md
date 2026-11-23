# Terminal menu audit

## Design direction

Issue #54 revises #49: each top-level menu is now a single flat, keyboard-navigable list. The former collapsed groups are removed. Frequent actions remain direct; less frequent commands stay registered and searchable through **Search All Commands** / **Help > Search Commands**. This audit does not remove command IDs, shortcuts, macros, or keymap support.

Frequency is an engineering estimate from command purpose and existing “advanced” metadata; the application does not collect command-usage telemetry.

## Before and proposed menu structure

| Menu | Before | Proposed |
|---|---|---|
| File | File > Direct actions; File > Projects & Files; File > Files & Imports | Flat list: New Project, Open Project, Recent Projects, Import Scrivener as New Project…, Import Scrivener Book into Series, Save Manuscript Buffer, Quit, Search All Commands… |
| Edit | Edit > Editing Preferences; Edit > Direct actions; Edit > Text & Editing Tools | Flat list: Find in Manuscript, Find and Replace, Rename Scene, Set Scene Status, Edit Scene Planning Field, Comment on Selection, Search All Commands… |
| Insert | Insert > Direct actions; Insert > Manuscript Structure; Insert > Links & Notes; Insert > Writing Tools | Flat list: New Section, New Chapter, New Scene, Create Direct Prose Unit, Create Story Link, Remove Story Link, Insert Snippet…, Search All Commands… |
| Navigate | Navigate > Workspaces & Outline; Navigate > Direct actions; Navigate > Search & Navigation; Navigate > Scenes & Documents | Flat list: Manuscript, Outline, Reference, Open Scene, Return to Previous Position, Search All Commands… |
| View | View > Direct actions | Flat list: Show Formatting Marks, Search All Commands… |
| Review | Review > Direct actions; Review > Revisions & Recovery; Review > Issues & Review; Review > Reading & Annotations | Flat list: Resolve Save Conflict, New Writing Issue, List Reading Annotations, List Reading Bookmarks, Show Revision History, Compare Snapshot, Create Snapshot, Search All Commands… |
| Publish | Publish > Direct actions; Publish > Compile & Export; Publish > Manuscript Assemblies | Flat list: Compile, Compile / Export…, Compile Results, Compile Configuration, Compile Assembly, Preview Latest Compile Output, Preview Document Inclusions, Search All Commands… |
| Tools | Tools > Direct actions; Tools > Advanced Tools; Tools > Research & Reading; Tools > Backup & Recovery; Tools > Writing Tools | Flat list: Search All Commands…, New Research Note, Keybinding Manager, Editor Preferences, Project Text Expansion Settings, Personal Text Expansion Settings, Manage Snippets…, Quick Capture, Writing Statistics |
| Help | Help > Direct actions; Help > Help & Documentation | Flat list: Resume Tutorial, Context Help, Search Commands |

## Command-by-command audit

Current location records the pre-change menu from the previous inventory. Proposed locations reflect the new flat-menu direction. “Search All Commands” keeps less-used functions available without crowding the author’s primary actions.

| Command ID | Current action | Current location | Proposed location | Estimated frequency | Disposition |
|---|---|---|---|---|---|
| `view.dashboard` | Dashboard | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.outline` | Outline | Navigate > Direct actions | Navigate > direct action | Frequent | Retain as direct action |
| `view.cards` | Scene Cards | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.editor` | Manuscript | Navigate > Direct actions | Navigate > direct action | Frequent | Retain as direct action |
| `view.plot-grid` | Plot Grid | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.characters` | Characters | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.story-bible` | Story Bible | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.timeline` | Timeline | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.analytics` | Analytics | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.research` | Research | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.issues` | Issues | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.search` | Search | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.reading` | Reference | Navigate > Direct actions | Navigate > direct action | Frequent | Retain as direct action |
| `view.revisions` | Revisions | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.planning-boards` | Planning Boards | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.assemblies` | Manuscript Assemblies | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.series` | Series Books | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `command.palette-open` | Command Palette | Tools > Direct actions | Tools > Search All Commands…; also listed in other menus as a universal action | Frequent | Retain as direct action; user-facing label is Search All Commands… |
| `file.new-project` | New Project | File > Direct actions | File > direct action | Frequent | Retain as direct action |
| `file.open-project` | Open Project | File > Direct actions | File > direct action | Frequent | Retain as direct action |
| `file.import-scrivener` | Import Scrivener as New Project… | File > Direct actions | File > direct action | Frequent | Retain as direct action |
| `file.import-scrivener-current` | Import Scrivener into Current Project… | File > Projects & Files | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `file.recent-projects` | Recent Projects | File > Direct actions | File > direct action | Frequent | Retain as direct action |
| `file.save` | Save Manuscript Buffer | File > Direct actions | File > direct action | Frequent | Retain as direct action |
| `file.quit` | Quit | File > Direct actions | File > direct action | Frequent | Retain as direct action |
| `edit.resolve-save-conflict` | Resolve Save Conflict | Review > Direct actions | Review > direct action | Frequent | Retain as direct action |
| `help.context` | Context Help | Help > Direct actions | Help > direct action | Frequent | Retain as direct action |
| `help.show-welcome` | Show Welcome | Help > Help & Documentation | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `help.resume-tutorial` | Resume Tutorial | Help > Direct actions | Help > direct action | Frequent | Retain as direct action |
| `help.restart-tutorial` | Restart Tutorial | Help > Help & Documentation | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `help.search` | Search Commands | Help > Help & Documentation | Help > Search Commands | Frequent | Promote to visible Help action |
| `revision.restore-hunk` | Restore Selected Revision Hunk | Review > Revisions & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.run` | Compile | Publish > Direct actions | Publish > direct action | Frequent | Retain as direct action |
| `compile.expand-transclusions` | Compile with Transclusions | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.selected` | Compile Selected Stages | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.selected-expand-transclusions` | Compile Selected Stages with Transclusions | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `view.formatting-marks` | Show Formatting Marks | View > Direct actions | View > direct action | Frequent | Retain as direct action |
| `compile.format` | Compile / Export… | Publish > Direct actions | Publish > direct action | Frequent | Retain as direct action |
| `compile.configuration` | Compile Configuration | Publish > Direct actions | Publish > direct action | Frequent | Retain as direct action |
| `compile.results` | Compile Results | Publish > Direct actions | Publish > direct action | Frequent | Retain as direct action |
| `compile.suppressions` | Diagnostic Suppressions | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.suppress` | Suppress Compile Finding | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.unsuppress` | Revoke Diagnostic Suppression | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.preview-output` | Preview Latest Compile Output | Publish > Direct actions | Publish > direct action | Frequent | Retain as direct action |
| `compile.open-output-directory` | Open Compile Output Directory | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.next-diagnostic` | Next Compile Diagnostic | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.previous-diagnostic` | Previous Compile Diagnostic | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `review.merge` | Merge Editorial Revision | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `review.resolve` | Resolve Editorial Conflicts | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `review.docx` | Accept or Reject DOCX Revisions | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `review.inspect` | Inspect DOCX Review Markup | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scene.create` | New Scene | Insert > Direct actions | Insert > direct action | Frequent | Retain as direct action |
| `scene.move` | Move Scene | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scene.rename` | Rename Scene | Edit > Direct actions | Edit > direct action | Frequent | Retain as direct action |
| `scene.status` | Set Scene Status | Edit > Direct actions | Edit > direct action | Frequent | Retain as direct action |
| `scene.field` | Edit Scene Planning Field | Edit > Direct actions | Edit > direct action | Frequent | Retain as direct action |
| `search.find-panel` | Find in Manuscript | Edit > Direct actions | Edit > direct action | Frequent | Retain as direct action |
| `search.find-open` | Find in All Open Files | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.find-project` | Find in Project | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.occur` | List Current Scene Occurrences | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.grep` | Search Manuscript Text | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.tags` | Go to Definition | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.refs` | Find References | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.return-position` | Return to Previous Position | Navigate > Direct actions | Navigate > direct action | Frequent | Retain as direct action |
| `search.tags-export` | Export Emacs TAGS Index | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.history-older` | Previous Search Query | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.history-newer` | Next Search Query | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.replace-panel` | Find and Replace | Edit > Direct actions | Edit > direct action | Frequent | Retain as direct action |
| `search.replace-next` | Replace Next in Current Scene | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.replace-undo` | Undo Last Scoped Replacement | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.replace-preview` | Preview Find and Replace | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.replace-apply` | Apply Replacement Preview | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.replace-cancel` | Cancel Replacement Preview | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `scene.subchapter` | New Chapter in Section | Insert > Manuscript Structure | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `structure.section` | New Section | Insert > Direct actions | Insert > direct action | Frequent | Retain as direct action |
| `structure.chapter` | New Chapter | Insert > Direct actions | Insert > direct action | Frequent | Retain as direct action |
| `character.create` | New Character | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `character.profile` | Show Character Profile | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `character.set` | Edit Character Field | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `character.alias` | Add Character Alias | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `character.un-alias` | Remove Character Alias | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `character.link` | Link Character to Scene | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `location.create` | New Location | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `plotline.create` | New Plotline | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `plotline.link` | Link Plotline to Scene | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `plotline.shares` | Set Plotline Word Shares | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `event.create` | New Story Event | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `event.create-timed` | New Timed Story Event | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `event.edit-time` | Edit Event Chronology | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `chronology.settings` | Chronology Settings | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scene.edit-time` | Edit Scene Chronology | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scene.edit-time-boundaries` | Set Scene Interval Boundaries | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `event.edit-time-boundaries` | Set Event Interval Boundaries | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `event.link` | Link Event to Scene | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `issue.create` | New Writing Issue | Review > Direct actions | Review > direct action | Frequent | Retain as direct action |
| `issue.link` | Link Issue to Scene | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `issue.status` | Change Issue Status | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `research.create` | New Research Note | Tools > Direct actions | Tools > direct action | Frequent | Retain as direct action |
| `research.import` | Import Reading Document | File > Files & Imports | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `research.open` | Open Reading Document | Tools > Research & Reading | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `research.highlight` | Add Reading Highlight | Tools > Research & Reading | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `research.annotation` | Add Reading Note | Tools > Research & Reading | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `research.bookmark` | Add Reading Bookmark | Tools > Research & Reading | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `research.term` | Add Reading Term | Tools > Research & Reading | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `research.entity-link` | Link Reference to Story Entity | Tools > Research & Reading | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `research.follow-link` | Follow Reading Link | Tools > Research & Reading | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `search.project` | Search Project | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.saved-query` | Run Saved Query | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.save-query` | Save Scene Query | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.query-expression` | Set Nested Query Expression | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.list-queries` | List Saved Queries | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-manager` | Keybinding Manager | Tools > Direct actions | Tools > Preferences / Settings | Frequent | Move to Tools Preferences/Settings; preserve registry ID and bindings |
| `settings.keybinding-set` | Bind Command Shortcut | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-remove` | Remove Command Shortcut | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-list` | List Command Shortcuts | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-context-set` | Bind Context Key Sequence | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-context-remove` | Remove Context Key Sequence | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-user-context-set` | Bind User Context Key Sequence | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-user-context-remove` | Remove User Context Key Sequence | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-user-set` | Bind User Command Shortcut | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-user-remove` | Remove User Command Shortcut | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-reset` | Reset Project Command Shortcuts | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `settings.keybinding-user-reset` | Reset User Command Shortcuts | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `search.query-evidence` | Inspect Query Match Evidence | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.clear-query` | Clear Query View | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.query-assert-save` | Save Query Assertions | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `search.query-assert-check` | Check Query Assertions | Navigate > Search & Navigation | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `analysis.coverage-add` | Add Coverage Rule | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.event-dependency-add` | Add Event Dependency | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.event-characters-set` | Set Event Participants | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.event-locations-set` | Set Event Locations | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.event-relationships` | Show Event Relationships | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.event-dependency-list` | List Event Dependencies | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.event-dependency-check` | Check Event Dependencies | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.event-dependency-update` | Update Event Dependency | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.event-dependency-remove` | Remove Event Dependency | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.location-rule-add` | Add Location Conflict Rule | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.location-rule-list` | List Location Conflict Rules | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.location-rule-update` | Update Location Conflict Rule | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.location-rule-check` | Check Location Conflict Rules | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.location-rule-remove` | Remove Location Conflict Rule | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.coverage-check` | Check Coverage Rules | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.coverage-list` | List Coverage Rules | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.coverage-remove` | Remove Coverage Rule | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `analysis.spellcheck` | Spellcheck Manuscript | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `link.create` | Create Story Link | Insert > Direct actions | Insert > direct action | Frequent | Retain as direct action |
| `link.delete` | Remove Story Link | Insert > Direct actions | Insert > direct action | Frequent | Retain as direct action |
| `link.outgoing` | Show Outgoing Links | Insert > Links & Notes | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `link.backlinks` | Show Backlinks | Insert > Links & Notes | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `link.broken` | Find Broken Links | Insert > Links & Notes | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `anchor.list` | List Passage Anchors | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `anchor.move` | Reattach Passage Anchor | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `compile.preview-inclusions` | Preview Document Inclusions | Publish > Direct actions | Publish > direct action | Frequent | Retain as direct action |
| `revision.snapshot` | Create Snapshot | Review > Direct actions | Review > direct action | Frequent | Retain as direct action |
| `revision.history` | Show Revision History | Review > Direct actions | Review > direct action | Frequent | Retain as direct action |
| `revision.diff` | Compare Snapshot | Review > Direct actions | Review > direct action | Frequent | Retain as direct action |
| `revision.restore` | Restore Snapshot Range | Review > Revisions & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `revision.restore-lines` | Restore Selected Snapshot Lines | Review > Revisions & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.snapshot` | Create Project Snapshot | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.snapshot-on-exit` | Automatic Snapshot on Exit | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.history` | Show Project History | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.remote-add` | Add Git Backup Remote | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.remotes` | List Git Backup Remotes | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.back-up` | Back Up Branch to Remote | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.retrieve` | Retrieve Remote Branch | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.diff` | Compare Project Snapshots | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.restore-new` | Restore Snapshot as New Project | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.restore-preview` | Preview Full Project Restore | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.restore-project` | Restore Full Project from Snapshot | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.restore-scene` | Restore Scene from Snapshot | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.restore-archived-scene` | Restore Archived Scene with Original ID | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `git.restore-deleted-scene` | Recover Deleted Scene as New | Tools > Backup & Recovery | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `extract.propose` | Extract Story Candidates | Tools > Direct actions | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `extract.approve` | Approve Candidate | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `extract.dismiss` | Dismiss Candidate | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `extract.review` | Review Candidate Proposals | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `extract.knowledge` | List Approved Knowledge | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `reading.jump` | Jump to Reading Offset | Review > Reading & Annotations | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `reading.list-bookmarks` | List Reading Bookmarks | Review > Direct actions | Review > direct action | Frequent | Retain as direct action |
| `reading.list-annotations` | List Reading Annotations | Review > Direct actions | Review > direct action | Frequent | Retain as direct action |
| `reading.annotation-jump` | Jump to Annotation | Review > Reading & Annotations | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `reading.bookmark-jump` | Jump to Bookmark | Review > Reading & Annotations | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `reading.list-terms` | List Reading Terms | Review > Reading & Annotations | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `reading.links` | List Reading Links | Review > Reading & Annotations | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `workspace.open` | Open Workspace | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scene.open` | Open Scene | Navigate > Direct actions | Navigate > direct action | Frequent | Retain as direct action |
| `editor.narrow-passage` | Narrow to Selection | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `editor.widen` | Widen Editing View | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `view.follow-scene-card` | Follow Scene Card | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.pin-reference` | Pin Reference Pane | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.toggle-references` | Hide or Show Reference Panes | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.reference-layout` | Set Reference Split Layout | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.reference-outline` | Open Outline Reference | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.reference-timeline` | Open Timeline Reference | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.reference-search` | Open Search Results Reference | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `view.reference-diagnostics` | Open Compile Diagnostics Reference | Navigate > Workspaces & Outline | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `compile.jump-diagnostic` | Jump to Compile Diagnostic | Publish > Compile & Export | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.next` | Next Window | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.previous` | Previous Window | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.move-reference-earlier` | Move Reference Earlier | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.move-reference-later` | Move Reference Later | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.close` | Close Reference Window | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.return-to-writing` | Return to Writing | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.maximize` | Maximize Current Window | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.restore-layout` | Restore Window Layout | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `window.swap-sides` | Swap Editor and Reference Sides | Navigate > Scenes & Documents | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scratchpad.create` | Quick Capture | Tools > Direct actions | Tools > direct action | Frequent | Retain as direct action |
| `scratchpad.list` | Scratchpad Notes | Tools > Writing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scratchpad.pin` | Pin Scratchpad Note | Tools > Writing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scratchpad.promote` | Promote Scratchpad Note | Tools > Writing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `mention.list` | Scene Mentions | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `mention.preview-at-cursor` | Preview Mention at Cursor | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `language.typography-preview` | Preview Typography Suggestions | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `mention.jump` | Jump to Story Mention | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `mention.appearances` | Find Confirmed References | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `mention.confirm` | Confirm Entity Mention | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `mention.dismiss` | Dismiss Entity Mention | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `object.create` | New Story Object | Review > Issues & Review | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `comment.create` | Comment on Selection | Edit > Direct actions | Edit > direct action | Frequent | Retain as direct action |
| `comment.list` | Scene Comments | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `comment.state` | Resolve or Reopen Comment | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.create` | Create Manuscript Assembly | Publish > Manuscript Assemblies | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.list` | List Manuscript Assemblies | Publish > Manuscript Assemblies | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.edit` | Edit Assembly Scenes | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.resume` | Resume Assembly Draft | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.drafts` | List Assembly Drafts | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.add` | Add Scene to Assembly | Publish > Manuscript Assemblies | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.add-content` | Add Content Unit to Assembly | Publish > Manuscript Assemblies | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `project.word-count-policy` | Set Word-count Policy | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scene.language` | Set Scene Language | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `content.language` | Set Prose Document Language | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scene.word-count-policy` | Set Scene Word-count Policy | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `scene.passage-language` | Tag Passage Language | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.move` | Reorder Assembly Item | Publish > Manuscript Assemblies | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.include` | Include or Exclude Assembly Item | Publish > Manuscript Assemblies | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `assembly.compile` | Compile Assembly | Publish > Direct actions | Publish > direct action | Frequent | Retain as direct action |
| `board.create` | Create Planning Board | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.open` | Open Planning Board | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.back` | Board List | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.list` | List Board Items | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.export` | Export Planning Board | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.export-svg` | Export Planning Board SVG | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.move` | Move Planning Card | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.resize` | Resize Planning Card | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.connect` | Connect Planning Cards | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.add-scene` | Place Scene on Board | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `board.add` | Add Planning Card | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `writing.stats` | Writing Statistics | Tools > Direct actions | Tools > direct action | Frequent | Retain as direct action |
| `edit.snippet-save` | Define Session Snippet | Insert > Writing Tools | Tools > Manage Snippets / command search | Specialized / rare | Move out of Insert; preserve registry ID, shortcut and macro support |
| `edit.snippet-list` | List Effective Snippets | Insert > Writing Tools | Tools > Manage Snippets | Occasional | Provide a direct management entry; preserve registry ID and searchable actions |
| `edit.snippet-insert` | Insert Snippet… | Insert > Writing Tools | Insert > Insert Snippet… chooser | Frequent | Combine insertion variants behind one chooser |
| `edit.snippet-preview` | Preview Effective Snippet | Insert > Writing Tools | Tools > Manage Snippets / command search | Specialized / rare | Keep searchable; remove as a peer in Insert |
| `edit.snippet-user-set` | Define Personal Snippet | Insert > Writing Tools | Tools > Manage Snippets / command search | Specialized / rare | Keep searchable; remove as a peer in Insert |
| `edit.snippet-user-remove` | Remove Personal Snippet | Insert > Writing Tools | Tools > Manage Snippets / command search | Specialized / rare | Keep searchable; remove as a peer in Insert |
| `edit.snippet-user-insert` | Insert Personal Snippet | Insert > Writing Tools | Insert > Insert Snippet… chooser / command search | Specialized / rare | Consolidate insertion in the chooser; preserve direct command |
| `edit.snippet-user-preview` | Preview Personal Snippet | Insert > Writing Tools | Tools > Manage Snippets / command search | Specialized / rare | Keep searchable; remove as a peer in Insert |
| `edit.register-save` | Save Editor Register | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.register-insert` | Insert Editor Register | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.register-rotate` | Rotate Editor Registers | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.multi-insert` | Multi-cursor Insert | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.multi-replace` | Replace Multiple Selections | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.multi-replace-matches` | Replace All Active-Buffer Matches | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.macro` | Run Edit Macro | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.macro-define` | Define Edit Macro | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.macro-run` | Run Named Edit Macro | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.macro-preview` | Preview Named Edit Macro | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.macro-record` | Record Edit Macro | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.macro-stop` | Stop Macro Recording | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `command-macro.record` | Record Command Sequence | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `command-macro.stop` | Stop Command Recording | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `command-macro.run` | Run Command Sequence | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `command-macro.list` | List Command Sequences | Edit > Editing Preferences | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.text-expansion-set` | Set Text Expansion | Insert > Writing Tools | Tools > Project Text Expansion Settings / command search | Specialized / rare | Move to Tools; preserve registry ID and bindings |
| `edit.text-expansion-remove` | Remove Text Expansion | Insert > Writing Tools | Tools > Project Text Expansion Settings / command search | Specialized / rare | Move to Tools; preserve registry ID and bindings |
| `edit.text-expansion` | Project Text Expansion Settings | Tools > Advanced Tools | Tools > Project Text Expansion Settings | Frequent | Retain as a direct Tools action |
| `edit.text-expansion-user-set` | Set Personal Text Expansion | Insert > Writing Tools | Tools > Personal Text Expansion Settings / command search | Specialized / rare | Move to Tools; preserve registry ID and bindings |
| `edit.text-expansion-user-remove` | Remove Personal Text Expansion | Insert > Writing Tools | Tools > Personal Text Expansion Settings / command search | Specialized / rare | Move to Tools; preserve registry ID and bindings |
| `edit.text-expansion-user` | Personal Text Expansion Settings | Insert > Writing Tools | Tools > Personal Text Expansion Settings | Frequent | Retain as a direct Tools action |
| `edit.text-expand` | Expand Abbreviation | Insert > Writing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `document.new-content-unit` | Create Direct Prose Unit | Insert > Direct actions | Insert > direct action | Frequent | Retain as direct action |
| `document.open-content-unit` | Open Direct Prose Unit | Insert > Manuscript Structure | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.preferences` | Editor Preferences | Edit > Text & Editing Tools | Tools > Editor Preferences | Frequent | Move to Tools; preserve registry ID and bindings |
| `edit.bookmark-save` | Save Passage Bookmark | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.bookmark-list` | List Passage Bookmarks | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.bookmark-jump` | Jump to Passage Bookmark | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.mark-set` | Set Temporary Mark | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.mark-list` | List Temporary Marks | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.mark-jump` | Jump to Temporary Mark | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.mark-clear` | Clear Temporary Mark | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.kill-ring-list` | List Kill Ring | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.kill-ring-insert` | Insert Kill Ring Entry | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.yank-pop` | Cycle Previous Yank | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.cursor-add` | Add Multi-cursor | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `edit.cursor-clear` | Clear Multi-cursors | Edit > Text & Editing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `challenge.start` | Start Writing Challenge | Tools > Writing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `challenge.finish` | Finish Writing Challenge | Tools > Writing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `challenge.idle` | Set Kamikaze Idle Threshold | Tools > Writing Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `project.template-save` | Save Project as Template | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `story-template-list` | List Story Bible Templates | Tools > Direct actions | Search All Commands (flat command palette) | Occasional | Keep capability; remove from primary menu and expose through command search |
| `story-template-import` | Import Story Bible Template | File > Files & Imports | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `story-profile-show` | Show Story Bible Profile | Tools > Advanced Tools | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `story-profile-import` | Import Story Bible Profile | File > Files & Imports | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `series.open` | Open Series Books | File > Projects & Files | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `series.create` | Create Series | File > Projects & Files | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `series.select` | Select Series | File > Projects & Files | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `series.add-current` | Add Current Book to Series | File > Projects & Files | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `series.import-scrivener` | Import Scrivener Book into Series | File > Direct actions | File > direct action | Frequent | Retain as direct action |
| `series.new-book` | New Book in Series | File > Projects & Files | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |
| `series.references` | Read Series References | File > Projects & Files | Search All Commands (flat command palette) | Specialized / rare | Keep capability; remove from primary menu and expose through command search |

## Implementation status

- Flat menus, direct shortcut display, universal Search All Commands access, and a single Insert Snippet entry are implemented in the current working changes.
- Snippet selection opens a project/personal chooser; insertion remains undoable and uses existing placeholder-value syntax.
- Text-expansion settings and editor preferences have been moved out of Insert into Tools. Individual snippet management commands remain searchable while a dedicated preferences manager is unfinished.
- Automated regression tests cover flat navigation, no Tab menu mode, command search access, snippet source selection, and configuration placement. Human UAT is still required.

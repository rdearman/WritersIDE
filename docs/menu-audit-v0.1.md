# Terminal menu audit — 24 November 2026

This is the complete registered command inventory at the flat-menu baseline (`4cfa92b`). It maps every registered command ID to its current author-facing access point, proposed access point, frequency signal, and disposition. The source of truth is `command_registry()` plus `MENU_COMMON_COMMANDS` in `apps/terminal/src/main.rs`; the regression test `menus_are_flat_curated_lists_and_keep_all_commands_searchable` checks that menu IDs are registered, curated lists are unique, and all other registered actions remain searchable. The UAT 18 contextual-menu update is recorded below as a dated supplement.

## Frequency interpretation

The application does not collect command-use telemetry. Frequency is therefore **not measured**. “Routine candidate” means intentionally placed in a primary menu for immediate discovery; “searchable-only candidate” means retained in the command registry/palette because it is not currently curated into a primary action list. This is a placement signal, not a claim about observed use. The existing `advanced` flag is recorded separately for maintainers and is not presented as user frequency data.

## Current and proposed command placement

No capability or command ID is removed. Primary menus remain concise, single-level action lists; less common actions remain in **Search All Commands…** / command palette under the existing resolved menu category. `menu.*` entries are the top-level menu headings, not author actions. Insert Snippet remains one chooser action; snippet and text-expansion configuration remain in Tools.

| Command ID | Current label | Current location | Proposed location | Frequency signal | Disposition |
|---|---|---|---|---|---|
| `view.dashboard` | Dashboard | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.outline` | Outline | Navigate menu; command palette / Search All Commands | Navigate menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `view.cards` | Scene Cards | Search All Commands… / command palette → Navigate | Navigate menu; command palette / Search All Commands | Routine candidate; not measured | Promote to primary list for fiction planning |
| `view.editor` | Manuscript | Navigate menu; command palette / Search All Commands | Navigate menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `view.plot-grid` | Plot Grid | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.characters` | Characters | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.story-bible` | Story Bible | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.timeline` | Timeline | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.analytics` | Analytics | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.research` | Research | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.issues` | Issues | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.search` | Search | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.reading` | Reference | Navigate menu; command palette / Search All Commands | Navigate menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `view.revisions` | Revisions | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.planning-boards` | Planning Boards | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.assemblies` | Manuscript Assemblies | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.series` | Series Books | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `menu.file` | Open File Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.edit` | Open Edit Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.insert` | Open Insert Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.navigate` | Open Navigate Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.view` | Open View Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.review` | Open Review Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.publish` | Open Publish Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.tools` | Open Tools Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.help` | Open Help Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `menu.scene` | Open Scene Menu | Added after the flat-menu baseline; available in Scene Cards | Scene Cards context menu; Publish is hidden in this workspace | Navigation infrastructure; not measured | Add contextual menu heading |
| `command.palette-open` | Command Palette | All primary menus (except Tools/Help) and Tools | All primary menus (except Tools/Help) and Tools | Routine discovery access; not measured | Retain searchable command entry |
| `menu.main` | Main Menu | Menu bar | Menu bar | Navigation infrastructure; not measured | Retain as menu heading |
| `file.new-project` | New Project | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `file.open-project` | Open Project | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `file.open-text` | Open Text File… | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `file.import-document` | Import Document… | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `file.import-scrivener` | Import Scrivener as New Project… | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `file.import-scrivener-current` | Import Scrivener into Current Project… | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `file.recent-projects` | Recent Projects | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `file.close-project` | Close Project | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine project action | Close safely and return to the start screen; dirty or temporary work requires Save, Discard, or Cancel |
| `file.save` | Save Project | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Save the current book; prompts for a permanent location on a temporary start project |
| `file.save-as` | Save Project As… | Not registered in the prior audit | File menu; command palette / Search All Commands | Routine project action | Create and validate an independent project copy; never overwrite |
| `file.quit` | Quit | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `edit.resolve-save-conflict` | Resolve Save Conflict | Review menu; command palette / Search All Commands | Review menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `help.context` | Context Help | Help menu; command palette / Search All Commands | Help menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `help.show-welcome` | Show Welcome | Search All Commands… / command palette → Help | Search All Commands… / command palette → Help | Searchable-only candidate; not measured | Retain in searchable registry |
| `help.resume-tutorial` | Resume Tutorial | Help menu; command palette / Search All Commands | Help menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `help.restart-tutorial` | Restart Tutorial | Search All Commands… / command palette → Help | Search All Commands… / command palette → Help | Searchable-only candidate; not measured | Retain in searchable registry |
| `help.search` | Search Commands | Help menu; command palette / Search All Commands | Help menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `revision.restore-hunk` | Restore Selected Revision Hunk | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.run` | Compile | Publish menu; command palette / Search All Commands | Publish menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `compile.expand-transclusions` | Compile with Transclusions | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.selected` | Compile Selected Stages | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.selected-expand-transclusions` | Compile Selected Stages with Transclusions | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.toggle-inclusion` | Move Scene Between Included and Excluded | Edit menu; command palette / Search All Commands | Edit menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `view.formatting-marks` | Show Formatting Marks | View menu; command palette / Search All Commands | View menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `compile.format` | Compile / Export… | Publish menu; command palette / Search All Commands | Publish menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `compile.configuration` | Compile Configuration | Publish menu; command palette / Search All Commands | Publish menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `compile.results` | Compile Results | Publish menu; command palette / Search All Commands | Publish menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `compile.suppressions` | Diagnostic Suppressions | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.suppress` | Suppress Compile Finding | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.unsuppress` | Revoke Diagnostic Suppression | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.preview-output` | Preview Latest Compile Output | Publish menu; command palette / Search All Commands | Publish menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `compile.open-output-directory` | Open Compile Output Directory | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.next-diagnostic` | Next Compile Diagnostic | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.previous-diagnostic` | Previous Compile Diagnostic | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `review.merge` | Merge Editorial Revision | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `review.resolve` | Resolve Editorial Conflicts | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `review.docx` | Accept or Reject DOCX Revisions | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `review.inspect` | Inspect DOCX Review Markup | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.create` | New Scene | Insert menu; command palette / Search All Commands | Insert menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `scene.move` | Move Scene to Chapter… | Edit menu; command palette / Search All Commands; Scene context menu | Edit menu; command palette / Search All Commands; Scene context menu | Routine candidate; not measured | Retain; opens an arrow-operated chapter chooser |
| `scene.rename` | Rename Scene | Edit menu; command palette / Search All Commands | Edit menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `scene.status` | Set Scene Status | Edit menu; command palette / Search All Commands | Edit menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `scene.field` | Edit Scene Planning Field | Edit menu; command palette / Search All Commands | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.find-panel` | Find in Manuscript | Edit menu; command palette / Search All Commands | Edit menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `search.find-open` | Find in All Open Files | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.find-project` | Find in Project | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.occur` | List Current Scene Occurrences | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.grep` | Search Manuscript Text | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.tags` | Go to Definition | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.refs` | Find References | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.return-position` | Return to Previous Position | Navigate menu; command palette / Search All Commands | Navigate menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `search.tags-export` | Export Emacs TAGS Index | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.history-older` | Previous Search Query | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.history-newer` | Next Search Query | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.replace-panel` | Find and Replace | Edit menu; command palette / Search All Commands | Edit menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `search.replace-next` | Replace Next in Current Scene | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.replace-undo` | Undo Last Scoped Replacement | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.replace-preview` | Preview Find and Replace | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.replace-apply` | Apply Replacement Preview | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.replace-cancel` | Cancel Replacement Preview | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.subchapter` | New Chapter in Section | Search All Commands… / command palette → Insert | Search All Commands… / command palette → Insert | Searchable-only candidate; not measured | Retain in searchable registry |
| `structure.section` | New Section | Insert menu; command palette / Search All Commands | Insert menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `structure.chapter` | New Chapter | Insert menu; command palette / Search All Commands | Insert menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `character.create` | New Character | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `character.profile` | Show Character Profile | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `character.set` | Edit Character Field | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `character.alias` | Add Character Alias | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `character.un-alias` | Remove Character Alias | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `character.link` | Link Character to Scene | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `location.create` | New Location | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `plotline.create` | New Plotline | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `plotline.link` | Link Plotline to Scene | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `plotline.shares` | Set Plotline Word Shares | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `event.create` | New Story Event | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `event.create-timed` | New Timed Story Event | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `event.edit-time` | Edit Event Chronology | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `chronology.settings` | Chronology Settings | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.edit-time` | Edit Scene Chronology | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.edit-time-boundaries` | Set Scene Interval Boundaries | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `event.edit-time-boundaries` | Set Event Interval Boundaries | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `event.link` | Link Event to Scene | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `issue.create` | New Writing Issue | Review menu; command palette / Search All Commands | Review menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `issue.link` | Link Issue to Scene | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `issue.status` | Change Issue Status | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `research.create` | New Research Note | Tools menu; command palette / Search All Commands | Tools menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `research.import` | Import Reading Document | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `research.open` | Open Reading Document | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `research.highlight` | Add Reading Highlight | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `research.annotation` | Add Reading Note | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `research.bookmark` | Add Reading Bookmark | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `research.term` | Add Reading Term | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `research.entity-link` | Link Reference to Story Entity | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `research.follow-link` | Follow Reading Link | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.project` | Search Project | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.saved-query` | Run Saved Query | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.save-query` | Save Scene Query | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.query-expression` | Set Nested Query Expression | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.list-queries` | List Saved Queries | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-manager` | Keybinding Manager | Tools menu; command palette / Search All Commands | Tools menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `settings.keybinding-set` | Bind Command Shortcut | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-remove` | Remove Command Shortcut | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-list` | List Command Shortcuts | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-context-set` | Bind Context Key Sequence | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-context-remove` | Remove Context Key Sequence | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-user-context-set` | Bind User Context Key Sequence | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-user-context-remove` | Remove User Context Key Sequence | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-user-set` | Bind User Command Shortcut | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-user-remove` | Remove User Command Shortcut | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-reset` | Reset Project Command Shortcuts | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `settings.keybinding-user-reset` | Reset User Command Shortcuts | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.query-evidence` | Inspect Query Match Evidence | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.clear-query` | Clear Query View | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.query-assert-save` | Save Query Assertions | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `search.query-assert-check` | Check Query Assertions | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.coverage-add` | Add Coverage Rule | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.event-dependency-add` | Add Event Dependency | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.event-characters-set` | Set Event Participants | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.event-locations-set` | Set Event Locations | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.event-relationships` | Show Event Relationships | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.event-dependency-list` | List Event Dependencies | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.event-dependency-check` | Check Event Dependencies | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.event-dependency-update` | Update Event Dependency | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.event-dependency-remove` | Remove Event Dependency | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.location-rule-add` | Add Location Conflict Rule | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.location-rule-list` | List Location Conflict Rules | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.location-rule-update` | Update Location Conflict Rule | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.location-rule-check` | Check Location Conflict Rules | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.location-rule-remove` | Remove Location Conflict Rule | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.coverage-check` | Check Coverage Rules | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.coverage-list` | List Coverage Rules | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.coverage-remove` | Remove Coverage Rule | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `analysis.spellcheck` | Spellcheck Manuscript | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `link.create` | Create Story Link | Insert menu; command palette / Search All Commands | Insert menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `link.delete` | Remove Story Link | Insert menu; command palette / Search All Commands | Insert menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `link.outgoing` | Show Outgoing Links | Search All Commands… / command palette → Insert | Search All Commands… / command palette → Insert | Searchable-only candidate; not measured | Retain in searchable registry |
| `link.backlinks` | Show Backlinks | Search All Commands… / command palette → Insert | Search All Commands… / command palette → Insert | Searchable-only candidate; not measured | Retain in searchable registry |
| `link.broken` | Find Broken Links | Search All Commands… / command palette → Insert | Search All Commands… / command palette → Insert | Searchable-only candidate; not measured | Retain in searchable registry |
| `anchor.list` | List Passage Anchors | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `anchor.move` | Reattach Passage Anchor | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.preview-inclusions` | Preview Document Inclusions | Publish menu; command palette / Search All Commands | Publish menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `revision.snapshot` | Create Snapshot | Review menu; command palette / Search All Commands | Review menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `revision.history` | Show Revision History | Review menu; command palette / Search All Commands | Review menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `revision.diff` | Compare Snapshot | Review menu; command palette / Search All Commands | Review menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `revision.restore` | Restore Snapshot Range | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `revision.restore-lines` | Restore Selected Snapshot Lines | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.snapshot` | Create Project Snapshot | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.snapshot-on-exit` | Automatic Snapshot on Exit | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.history` | Show Project History | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.remote-add` | Add Git Backup Remote | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.remotes` | List Git Backup Remotes | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.back-up` | Back Up Branch to Remote | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.retrieve` | Retrieve Remote Branch | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.diff` | Compare Project Snapshots | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.restore-new` | Restore Snapshot as New Project | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.restore-preview` | Preview Full Project Restore | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.restore-project` | Restore Full Project from Snapshot | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.restore-scene` | Restore Scene from Snapshot | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.restore-archived-scene` | Restore Archived Scene with Original ID | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `git.restore-deleted-scene` | Recover Deleted Scene as New | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `extract.propose` | Extract Story Candidates | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `extract.approve` | Approve Candidate | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `extract.dismiss` | Dismiss Candidate | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `extract.review` | Review Candidate Proposals | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `extract.knowledge` | List Approved Knowledge | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `reading.jump` | Jump to Reading Offset | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `reading.list-bookmarks` | List Reading Bookmarks | Review menu; command palette / Search All Commands | Review menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `reading.list-annotations` | List Reading Annotations | Review menu; command palette / Search All Commands | Review menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `reading.annotation-jump` | Jump to Annotation | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `reading.bookmark-jump` | Jump to Bookmark | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `reading.list-terms` | List Reading Terms | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `reading.links` | List Reading Links | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `workspace.open` | Open Workspace | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.open` | Open Scene | Navigate menu; command palette / Search All Commands | Navigate menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `editor.narrow-passage` | Narrow to Selection | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `editor.widen` | Widen Editing View | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.follow-scene-card` | Follow Scene Card | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.pin-reference` | Pin Reference Pane | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.toggle-references` | Hide or Show Reference Panes | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.reference-layout` | Set Reference Split Layout | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.reference-outline` | Open Outline Reference | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.reference-timeline` | Open Timeline Reference | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.reference-search` | Open Search Results Reference | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `view.reference-diagnostics` | Open Compile Diagnostics Reference | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `compile.jump-diagnostic` | Jump to Compile Diagnostic | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.next` | Next Window | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.previous` | Previous Window | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.move-reference-earlier` | Move Reference Earlier | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.move-reference-later` | Move Reference Later | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.close` | Close Reference Window | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.return-to-writing` | Return to Writing | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.maximize` | Maximize Current Window | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.restore-layout` | Restore Window Layout | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `window.swap-sides` | Swap Editor and Reference Sides | Search All Commands… / command palette → Navigate | Search All Commands… / command palette → Navigate | Searchable-only candidate; not measured | Retain in searchable registry |
| `scratchpad.create` | Quick Capture | Tools menu; command palette / Search All Commands | Tools menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `scratchpad.list` | Scratchpad Notes | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `scratchpad.pin` | Pin Scratchpad Note | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `scratchpad.promote` | Promote Scratchpad Note | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `mention.list` | Scene Mentions | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `mention.preview-at-cursor` | Preview Mention at Cursor | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `language.typography-preview` | Preview Typography Suggestions | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `mention.jump` | Jump to Story Mention | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `mention.appearances` | Find Confirmed References | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `mention.confirm` | Confirm Entity Mention | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `mention.dismiss` | Dismiss Entity Mention | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `object.create` | New Story Object | Search All Commands… / command palette → Review | Search All Commands… / command palette → Review | Searchable-only candidate; not measured | Retain in searchable registry |
| `comment.create` | Comment on Selection | Edit menu; command palette / Search All Commands | Edit menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `comment.list` | Scene Comments | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `comment.state` | Resolve or Reopen Comment | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.create` | Create Manuscript Assembly | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.list` | List Manuscript Assemblies | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.edit` | Edit Assembly Scenes | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.resume` | Resume Assembly Draft | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.drafts` | List Assembly Drafts | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.add` | Add Scene to Assembly | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.add-content` | Add Content Unit to Assembly | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `project.word-count-policy` | Set Word-count Policy | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.language` | Set Scene Language | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `content.language` | Set Prose Document Language | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.word-count-policy` | Set Scene Word-count Policy | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `scene.passage-language` | Tag Passage Language | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.move` | Reorder Assembly Item | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.include` | Include or Exclude Assembly Item | Search All Commands… / command palette → Publish | Search All Commands… / command palette → Publish | Searchable-only candidate; not measured | Retain in searchable registry |
| `assembly.compile` | Compile Assembly | Publish menu; command palette / Search All Commands | Publish menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `board.create` | Create Planning Board | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.open` | Open Planning Board | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.back` | Board List | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.list` | List Board Items | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.export` | Export Planning Board | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.export-svg` | Export Planning Board SVG | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.move` | Move Planning Card | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.resize` | Resize Planning Card | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.connect` | Connect Planning Cards | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.add-scene` | Place Scene on Board | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `board.add` | Add Planning Card | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `writing.stats` | Writing Statistics | Tools menu; command palette / Search All Commands | Tools menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `edit.snippet-save` | Define Session Snippet | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.snippet-list` | Manage Snippets… | Tools menu; command palette / Search All Commands | Tools menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `edit.snippet-insert` | Insert Snippet… | Insert menu; command palette / Search All Commands | Insert menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `edit.snippet-preview` | Preview Effective Snippet | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.snippet-user-set` | Define Personal Snippet | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.snippet-user-remove` | Remove Personal Snippet | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.snippet-user-insert` | Insert Personal Snippet | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.snippet-user-preview` | Preview Personal Snippet | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.register-save` | Save Editor Register | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.register-insert` | Insert Editor Register | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.register-rotate` | Rotate Editor Registers | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.multi-insert` | Multi-cursor Insert | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.multi-replace` | Replace Multiple Selections | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.multi-replace-matches` | Replace All Active-Buffer Matches | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.macro` | Run Edit Macro | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.macro-define` | Define Edit Macro | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.macro-run` | Run Named Edit Macro | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.macro-preview` | Preview Named Edit Macro | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.macro-record` | Record Edit Macro | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.macro-stop` | Stop Macro Recording | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `command-macro.record` | Record Command Sequence | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `command-macro.stop` | Stop Command Recording | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `command-macro.run` | Run Command Sequence | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `command-macro.list` | List Command Sequences | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.text-expansion-set` | Set Text Expansion | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.text-expansion-remove` | Remove Text Expansion | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.text-expansion` | Project Text Expansion Settings | Tools menu; command palette / Search All Commands | Tools menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `edit.text-expansion-user-set` | Set Personal Text Expansion | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.text-expansion-user-remove` | Remove Personal Text Expansion | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.text-expansion-user` | Personal Text Expansion Settings | Tools menu; command palette / Search All Commands | Tools menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `edit.text-expand` | Expand Abbreviation | Search All Commands… / command palette → Insert | Search All Commands… / command palette → Insert | Searchable-only candidate; not measured | Retain in searchable registry |
| `document.new-content-unit` | Create Direct Prose Unit | Insert menu; command palette / Search All Commands | Insert menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `document.open-content-unit` | Open Direct Prose Unit | Search All Commands… / command palette → Insert | Search All Commands… / command palette → Insert | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.preferences` | Editor Preferences | Tools menu; command palette / Search All Commands | Tools menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `edit.bookmark-save` | Save Passage Bookmark | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.bookmark-list` | List Passage Bookmarks | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.bookmark-jump` | Jump to Passage Bookmark | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.mark-set` | Set Temporary Mark | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.mark-list` | List Temporary Marks | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.mark-jump` | Jump to Temporary Mark | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.mark-clear` | Clear Temporary Mark | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.kill-ring-list` | List Kill Ring | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.kill-ring-insert` | Insert Kill Ring Entry | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.yank-pop` | Cycle Previous Yank | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.cursor-add` | Add Multi-cursor | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `edit.cursor-clear` | Clear Multi-cursors | Search All Commands… / command palette → Edit | Search All Commands… / command palette → Edit | Searchable-only candidate; not measured | Retain in searchable registry |
| `challenge.start` | Start Writing Challenge | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `challenge.finish` | Finish Writing Challenge | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `challenge.idle` | Set Kamikaze Idle Threshold | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `project.template-save` | Save Project as Template | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `story-template-list` | List Story Bible Templates | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `story-template-import` | Import Story Bible Template | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `story-profile-show` | Show Story Bible Profile | Search All Commands… / command palette → Tools | Search All Commands… / command palette → Tools | Searchable-only candidate; not measured | Retain in searchable registry |
| `story-profile-import` | Import Story Bible Profile | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `series.open` | Open Series Books | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `series.create` | Create Series | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `series.select` | Select Series | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `series.add-current` | Add Current Book to Series | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `series.import-scrivener` | Import Scrivener Book into Series | File menu; command palette / Search All Commands | File menu; command palette / Search All Commands | Routine candidate; not measured | Retain in primary list |
| `series.new-book` | New Book in Series | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |
| `series.references` | Read Series References | Search All Commands… / command palette → File | Search All Commands… / command palette → File | Searchable-only candidate; not measured | Retain in searchable registry |

**Registry at flat-menu baseline:** 323 registered commands, including top-level menu headings. **Baseline curated direct command IDs:** 56. The contextual Scene menu adds one registered menu command and nine curated contextual menu slots (some reuse baseline command IDs); Save Project As and Close Project add two project actions, bringing the current registry to 326 commands.

## Alpha UAT 18 contextual-menu supplement

The latest UAT direction keeps stable global actions and adds workspace-specific menu roots. In Scene Cards, **Scene** replaces **Publish** in the visible menu bar; Publish remains available from manuscript workspaces and command search. The Scene menu provides New Scene, Write Selected Scene, Rename Selected Scene, Edit Scene Synopsis, Move Scene Earlier/Later, Move Scene to Chapter, Include / Exclude Scene, and Search All Commands. It reuses the shared registry and dispatcher. The card keybar displays Up/Down selection, Left/Right scene movement, Shift+Left/Right chapter-block movement, Tab collection switching, Insert scene creation, Shift+Insert chapter creation and the effective Include / Exclude binding (Space by default). Unplaced scenes have a persistent independent order and can move into the first chapter or back to Unplaced while retaining stable identity. Remaining work includes bulk chapter assignment and further contextual menu coverage for Chapter, Research, Character and Timeline.

## Audit limits and follow-up

This audit describes implementation placement and keeps all commands available through existing search/dispatch paths. There is no historical usage data, so the application cannot honestly rank actions by actual use. Human UAT is still required to approve the labels and placement. If usage telemetry is ever added, it must be opt-in and local; it is not required for this menu design.

## Alpha UAT 18 — project save actions

The File menu now presents **Save Project** and **Save Project As…**. The first
Save Project action on a temporary start project opens the shared destination
browser after journalling any dirty manuscript buffer. A persistent **[TEMPORARY]**
status remains visible until the user confirms a destination. Save Project As
creates a separate staged project copy, including project files, metadata,
recovery/settings directories and local `.git` history; validates the copy
before switching to it; and refuses existing destinations (including symlinks).
The source remains active if the operation is cancelled or fails. Automated
checks cover manuscript persistence, attachments, Git metadata, collision
protection and cancellation. A no-argument launch shows a project-selection
welcome screen; explicit project arguments still open directly.

## Close Project workflow

**File → Close Project** closes the active project and returns to the start
screen. Dirty text offers Save and Close, Close Without Saving, or Cancel.
Temporary start projects always prompt to save a permanent copy, discard, or
cancel. The temporary directory is owned by the running application and is
removed only when discarded/closed or when another project replaces it.
Automated tests verify that the old project can be reopened, saved text persists
before closing, and cancellation leaves the current buffer intact.

## Alpha UAT 19 — contextual Edit actions

The Edit menu exposes Undo, Redo, Cut Selection, Copy Selection, Paste, Delete
Selection, Select All, Find Next and Find Previous as direct menu actions. The
registry displays conventional shortcuts (`Ctrl+Z`, `Ctrl+Y` / `Ctrl+Shift+Z`,
`Ctrl+X/C/V/A`, `Ctrl+F/H`, and `F3` / `Shift+F3`). Shortcut dispatch reuses the
existing editor operations; reference-pane `Ctrl+C` retains its passage-copy
meaning, and scoped replacement undo remains available through `Ctrl+Z`.
Automated menu-path regression coverage verifies selection, clipboard, cut,
undo, redo and paste. The Edit menu also exposes paragraph indentation, reflow
and case conversion as direct actions. These transform the current paragraph
when there is no selection, or selected text/paragraphs when selected; successful
changes are undoable. Indentation follows the project's tab or spaces
preference. Reflow wraps to 78 grapheme clusters while retaining blank
paragraph breaks. Case actions include uppercase, lowercase and title case.
Regression coverage invokes these operations through the Edit menu and checks
their results and undo behaviour.

## Alpha UAT 19 — manuscript insertion actions

The Insert menu now also exposes **Insert Date/Time** and **Insert File…**.
Date/time inserts an explicit UTC timestamp. Insert File reuses the shared
two-pane browser, accepts UTF-8 text files, preserves source bytes, and inserts
the chosen text at the manuscript cursor as one undoable edit. Menu integration,
undo, save/reopen persistence, and source preservation are exercised in both
unit and Linux PTY tests.

## 25 November 2026 — contextual View menu

The View menu now exposes the existing reference-pane operations when they are
applicable in the manuscript editor: open Outline, Timeline, and Compile
Diagnostics references; follow the active scene card; pin a focused reference;
hide/show open reference panes; switch or reorder multiple reference windows;
close a reference; return focus to writing; maximize/restore; and swap editor
and reference sides. Formatting marks are shown only while editing, and
rendered Markdown only in the Reference workspace. Actions that require an open
scene, focused reference, or multiple windows are omitted until that condition
holds. This reduces irrelevant menu entries while preserving command IDs and
the searchable command registry.

The application still renders its editor and reference-pane model rather than
the recursive `PaneLayout` geometry module. Therefore generic recursive splits,
same-buffer independent windows, and arbitrary view-buffer switching remain
unimplemented under #13; this menu update does not claim those requirements as
complete. Unit regressions verify menu visibility by workspace and invoke
reference open, return-to-writing, and close through the actual View menu.

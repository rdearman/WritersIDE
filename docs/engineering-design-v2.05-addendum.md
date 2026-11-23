# Engineering Design v2.05 Addendum — Independent scenes and visual card outliner

This addendum records the scene-independence review and implementation for Issues #20 and #55.

## Existing model and migration decision

The existing SQLite model already separates `scene` from `scene_placement`. A scene row and its portable manuscript file can exist without a placement row; `get_scene` reads placement with a `LEFT JOIN`, and `SceneRecord.chapter_id` and `position` are optional. The placement table remains the canonical ordered relationship between a scene and a chapter. Its existing foreign key and triggers continue to require that any placement target is a chapter.

The earlier API was narrower than the schema: `create_scene` always required and inserted a chapter placement, and `move_scene` rejected a scene without one. No schema migration is needed. `create_unplaced_scene` now creates the same stable scene, content file, defaults and metadata without a placement. The normal Insert → New Scene workflow asks only for a title and creates an Included, Unplaced card. The author can assign it through Edit → Move Scene, which offers a numbered list of chapter titles and places it at the end of the chosen chapter. IDs and command syntax remain available to expert CLI operations, not ordinary menu-driven author workflows.

Unplaced scenes remain searchable and editable. They are retained in the Included or Excluded planning collection, but are omitted from Compile IR until explicitly placed in a chapter. This keeps the publication sequence unambiguous. Chapter-based non-fiction remains represented by chapter content units and does not require scenes.

## Card outliner

Scene Cards remains a visual manuscript outliner, not a Kanban board. It presents Included and Excluded collections as two columns when width permits and as stacked collections on narrow terminals. Each collection is ordered by the canonical scene list and groups cards under derived chapter headings; unplaced scenes appear under an Unplaced heading. Cards show title, status, word count and an optional synopsis. Chapter headings are presentation-only group labels derived from structural nodes, not inserted manuscript text or replacement chapter-boundary records.

Up/Down selects a card, Ctrl+Up/Ctrl+Down reorders a placed scene within its chapter, Space reversibly toggles inclusion, and Enter opens the selected scene. Edit → Move Scene provides chapter selection without requiring UUIDs. Reordering and cross-chapter moves use the existing transactional placement service; inclusion remains stored separately in `scene_exclusion`. Stable scene identity, prose and metadata are unchanged by these operations. Excluded scenes remain editable and default Compile omits them.

## Chapter boundaries

The existing relational model remains authoritative: structural chapter nodes define boundaries and `scene_placement(chapter_id, position)` defines a scene's chapter and order. The card UI renders chapter headers as an author-facing representation of those boundaries. Chapter markers in a flat manuscript sequence may be evaluated as a future view or export presentation, but no such marker is persisted in canonical Markdown and the relational placement model is not replaced.

## Validation

Regression tests cover unplaced creation and reopen, Compile omission until placement, explicit placement and compilation, title-only menu creation, human-readable chapter selection, card grouping, inclusion state, ordering, persistence and stable scene content. The existing direct-chapter content-unit model remains unchanged. Human UAT is still required for Issue #55; this addendum does not mark that issue complete for acceptance.

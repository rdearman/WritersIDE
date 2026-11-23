# Engineering Design v2.08 Addendum — Included and Excluded scene cards

This addendum records the implementation response to the revised GitHub Issue #55 specification.

Scene Cards contains two ordered, chapter-grouped collections: **Included** and **Excluded**. Included scenes remain in canonical manuscript order and are compiled by default. Excluded scenes retain the same stable identity, prose, synopsis and metadata, but are omitted from compilation until restored. The excluded collection preserves its ordering independently; moving a scene between collections does not delete it.

The terminal presents the collections side by side at widths of 100 cells or more, and stacked below that threshold. Each card identifies its collection, title, status and word count, with an optional synopsis. Selection, reorder, include/exclude and open operations continue to use the shared scene identity and application services. Cross-chapter movement remains available through Edit → Move Scene.

Automated tests cover both terminal widths, separate ordered collections, reversible exclusion, persistence and preservation of manuscript text. The issue remains open because the revised card interaction and narrow-terminal presentation still require human UAT.

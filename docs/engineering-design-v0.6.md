# Writer's IDE — Engineering Design v0.6

**Status:** Active implementation design  
**Baseline:** Engineering Design v0.5  
**Migration level:** 4 (analytics are read-only projections; no schema change)

## Analysis module

`story-analysis` is a pure projection crate. It accepts stable identifiers, typed participation roles, plotline memberships, manuscript order, word counts and `StoryTime` values prepared by the application façade. It does not open SQLite, write project data, or reorder scenes. Storage exposes narrow read-only relationship queries, while `WriterApp::analytics_dataset` builds the DTO used by the terminal and CLI.

Implemented projections:

- Character/scene heatmap rows retaining POV and participation roles.
- Character co-appearance pairs, counting each character once per scene.
- Subplot gaps in manuscript-order scene intervals with a configurable minimum.
- Uncertainty-aware chronology flags. A pair is flagged when the existing `StoryTime` comparison cannot prove the intervals disjoint; unknown dates are explicitly provisional, not diagnosed as a confirmed contradiction.
- Word-budget redistribution preview. Existing explicit targets remain fixed; remaining project words are split among scenes without targets in manuscript order, with integer remainder assigned to earliest scenes. A preview never writes targets.

The analytics CLI prints these results, and the terminal Analytics workspace summarizes them. Reports must continue to distinguish a suggestion or uncertainty from a continuity error. This initial slice does not yet provide graphical heatmaps, character interaction timelines, a user-configurable budget policy, weighted subplot importance, nor plotline word attribution reports; those remain outstanding.

## API and validation contracts

- `analytics_dataset` includes active scenes in canonical manuscript ordering and resolves participating character/plotline IDs to display names.
- A missing optional project word target suppresses budget previews; target zero remains a valid target.
- No analytics function mutates entities, relationships, budgets or order.
- A gap count is the number of intervening manuscript scenes between consecutive appearances of a plotline.
- Chronology flags are candidate overlaps only. They are not continuity errors and must be resolved against richer character/location constraints.
- Arithmetic for word aggregation is saturating; redistribution uses integer division and deterministic remainder assignment.

## Acceptance coverage

1. Character interactions count unique character pairs once per scene.
2. Subplot gaps count intervening manuscript scenes and honor the requested threshold.
3. Unknown/uncertain chronology remains a candidate finding instead of an asserted ordering.
4. Budget previews preserve explicit targets, allocate the remaining total deterministically and do not mutate the story model.
5. Application dataset reads preserve scene ordering and resolve names from canonical relationship tables.
6. CLI and terminal Analytics workspace render the projection from the same application API.

## Remaining decisions

- Whether budget preview should redistribute across all scenes proportionally, target only unbudgeted scenes, or offer multiple policies.
- Whether character interactions should distinguish co-presence, dialogue and causal interaction.
- Plotline appearance gap thresholds and weighting should be project-configurable.
- A later visual backend may render matrices as Unicode heatmaps in terminal and interactive charts on desktop.
- Uncertainty diagnostics can become more precise when travel, character whereabouts and event duration constraints are modeled.

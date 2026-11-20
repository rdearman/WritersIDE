//! Read-only analytical projections over the canonical story model.
use std::collections::{BTreeMap, BTreeSet};
use story_core::{ParticipationRole, TimeOrdering};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterOnScene {
    pub id: String,
    pub name: String,
    pub role: ParticipationRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotlineOnScene {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct SceneAnalysisInput {
    pub scene_id: String,
    pub title: String,
    pub chapter_id: Option<String>,
    pub actual_words: u32,
    pub target_words: Option<u64>,
    pub start_time: story_core::StoryTime,
    pub end_time: story_core::StoryTime,
    pub characters: Vec<CharacterOnScene>,
    pub plotlines: Vec<PlotlineOnScene>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeatmapRow {
    pub scene_id: String,
    pub scene_title: String,
    pub chapter_id: Option<String>,
    /// Character IDs present in this scene; role is retained for POV distinction.
    pub participants: Vec<(String, String, ParticipationRole)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterInteraction {
    pub first_character: String,
    pub second_character: String,
    pub shared_scenes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotlineGap {
    pub plotline_id: String,
    pub plotline_name: String,
    pub from_scene_id: String,
    pub to_scene_id: String,
    pub intervening_scenes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChronologyFinding {
    pub first_scene_id: String,
    pub second_scene_id: String,
    pub comparison: TimeOrdering,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterChronologyStatus {
    DefiniteOverlap,
    PossibleOverlap,
    Unverified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterChronologyFinding {
    pub first_scene_id: String,
    pub second_scene_id: String,
    pub shared_character_ids: Vec<String>,
    pub status: CharacterChronologyStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetPreview {
    pub target_words: u64,
    pub fixed_words: u64,
    pub unallocated_words: u64,
    pub suggested_scene_targets: BTreeMap<String, u64>,
}

pub fn character_heatmap(scenes: &[SceneAnalysisInput]) -> Vec<HeatmapRow> {
    scenes
        .iter()
        .map(|s| HeatmapRow {
            scene_id: s.scene_id.clone(),
            scene_title: s.title.clone(),
            chapter_id: s.chapter_id.clone(),
            participants: s
                .characters
                .iter()
                .map(|c| (c.id.clone(), c.name.clone(), c.role))
                .collect(),
        })
        .collect()
}

pub fn character_interactions(scenes: &[SceneAnalysisInput]) -> Vec<CharacterInteraction> {
    let mut counts = BTreeMap::<(String, String), u32>::new();
    for scene in scenes {
        let ids: BTreeSet<_> = scene.characters.iter().map(|c| c.id.clone()).collect();
        let ids: Vec<_> = ids.into_iter().collect();
        for (i, first) in ids.iter().enumerate() {
            for second in ids.iter().skip(i + 1) {
                *counts.entry((first.clone(), second.clone())).or_default() += 1;
            }
        }
    }
    counts
        .into_iter()
        .map(
            |((first_character, second_character), shared_scenes)| CharacterInteraction {
                first_character,
                second_character,
                shared_scenes,
            },
        )
        .collect()
}

/// Reports long gaps measured in manuscript-order scenes. The threshold is caller-controlled.
pub fn subplot_gaps(scenes: &[SceneAnalysisInput], minimum_gap: usize) -> Vec<PlotlineGap> {
    let mut occurrences = BTreeMap::<(String, String), Vec<(usize, String)>>::new();
    for (index, scene) in scenes.iter().enumerate() {
        for plotline in &scene.plotlines {
            occurrences
                .entry((plotline.id.clone(), plotline.name.clone()))
                .or_default()
                .push((index, scene.scene_id.clone()));
        }
    }
    let mut gaps = Vec::new();
    for ((plotline_id, plotline_name), entries) in occurrences {
        for pair in entries.windows(2) {
            let intervening_scenes = pair[1].0.saturating_sub(pair[0].0 + 1);
            if intervening_scenes >= minimum_gap {
                gaps.push(PlotlineGap {
                    plotline_id: plotline_id.clone(),
                    plotline_name: plotline_name.clone(),
                    from_scene_id: pair[0].1.clone(),
                    to_scene_id: pair[1].1.clone(),
                    intervening_scenes,
                });
            }
        }
    }
    gaps
}

/// Generates a preview only: existing nonzero targets are fixed, while scenes without targets
/// share the remaining project budget. Remainders go to earliest manuscript-order scenes.
pub fn preview_budget_redistribution(
    target_words: u64,
    scenes: &[SceneAnalysisInput],
) -> BudgetPreview {
    let fixed_words = scenes
        .iter()
        .filter_map(|s| s.target_words)
        .fold(0_u64, u64::saturating_add);
    let unallocated_words = target_words.saturating_sub(fixed_words);
    let eligible: Vec<_> = scenes.iter().filter(|s| s.target_words.is_none()).collect();
    let mut suggested_scene_targets = BTreeMap::new();
    if !eligible.is_empty() {
        let base = unallocated_words / eligible.len() as u64;
        let remainder = unallocated_words % eligible.len() as u64;
        for (i, scene) in eligible.iter().enumerate() {
            suggested_scene_targets.insert(
                scene.scene_id.clone(),
                base + u64::from((i as u64) < remainder),
            );
        }
    }
    BudgetPreview {
        target_words,
        fixed_words,
        unallocated_words,
        suggested_scene_targets,
    }
}

/// Returns scene pairs whose uncertain story-time intervals overlap or cannot be ordered.
/// It deliberately does not infer a contradiction from missing or ambiguous values.
pub fn chronology_ambiguities(scenes: &[SceneAnalysisInput]) -> Vec<ChronologyFinding> {
    let mut findings = Vec::new();
    for (i, a) in scenes.iter().enumerate() {
        for b in scenes.iter().skip(i + 1) {
            let definitely_apart = matches!(a.end_time.compare(b.start_time), TimeOrdering::Before)
                || matches!(b.end_time.compare(a.start_time), TimeOrdering::Before);
            if !definitely_apart {
                findings.push(ChronologyFinding {
                    first_scene_id: a.scene_id.clone(),
                    second_scene_id: b.scene_id.clone(),
                    comparison: TimeOrdering::Ambiguous,
                });
            }
        }
    }
    findings
}

/// Reports same-character scene intervals that may overlap without treating uncertainty as a
/// proven conflict. Touching end/start boundaries are considered adjacent, not overlapping.
pub fn character_chronology_findings(
    scenes: &[SceneAnalysisInput],
) -> Vec<CharacterChronologyFinding> {
    use CharacterChronologyStatus::*;
    use TimeOrdering::*;

    let mut findings = Vec::new();
    for (index, first) in scenes.iter().enumerate() {
        for second in scenes.iter().skip(index + 1) {
            let first_characters = first
                .characters
                .iter()
                .map(|character| character.id.as_str())
                .collect::<BTreeSet<_>>();
            let shared_character_ids = second
                .characters
                .iter()
                .filter(|character| first_characters.contains(character.id.as_str()))
                .map(|character| character.id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if shared_character_ids.is_empty() {
                continue;
            }

            let first_before_second =
                matches!(first.end_time.compare(second.start_time), Before | Same);
            let second_before_first =
                matches!(second.end_time.compare(first.start_time), Before | Same);
            if first_before_second || second_before_first {
                continue;
            }

            let definite_overlap = matches!(first.start_time.compare(second.end_time), Before)
                && matches!(second.start_time.compare(first.end_time), Before);
            let has_unknown_endpoint = [
                first.start_time,
                first.end_time,
                second.start_time,
                second.end_time,
            ]
            .into_iter()
            .any(|time| time.precision == story_core::TimePrecision::Unknown);
            let status = if definite_overlap {
                DefiniteOverlap
            } else if has_unknown_endpoint {
                Unverified
            } else {
                PossibleOverlap
            };
            findings.push(CharacterChronologyFinding {
                first_scene_id: first.scene_id.clone(),
                second_scene_id: second.scene_id.clone(),
                shared_character_ids,
                status,
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(
        id: &str,
        target_words: Option<u64>,
        characters: &[&str],
        plotlines: &[&str],
    ) -> SceneAnalysisInput {
        SceneAnalysisInput {
            scene_id: id.into(),
            title: id.into(),
            chapter_id: Some("chapter".into()),
            actual_words: 10,
            target_words,
            start_time: story_core::StoryTime::unknown(),
            end_time: story_core::StoryTime::unknown(),
            characters: characters
                .iter()
                .map(|id| CharacterOnScene {
                    id: (*id).into(),
                    name: (*id).into(),
                    role: ParticipationRole::Major,
                })
                .collect(),
            plotlines: plotlines
                .iter()
                .map(|id| PlotlineOnScene {
                    id: (*id).into(),
                    name: (*id).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn interaction_matrix_counts_each_shared_scene_once() {
        let scenes = [
            scene("a", None, &["x", "y", "x"], &[]),
            scene("b", None, &["x", "y"], &[]),
        ];
        assert_eq!(
            character_interactions(&scenes),
            vec![CharacterInteraction {
                first_character: "x".into(),
                second_character: "y".into(),
                shared_scenes: 2
            }]
        );
    }

    #[test]
    fn budget_preview_never_mutates_and_distributes_remainder_deterministically() {
        let scenes = [
            scene("a", Some(3), &[], &[]),
            scene("b", None, &[], &[]),
            scene("c", None, &[], &[]),
        ];
        let preview = preview_budget_redistribution(10, &scenes);
        assert_eq!(preview.unallocated_words, 7);
        assert_eq!(preview.fixed_words, 3);
        assert_eq!(preview.suggested_scene_targets.get("b"), Some(&4));
        assert_eq!(preview.suggested_scene_targets.get("c"), Some(&3));
        assert!(scenes[1].target_words.is_none());
    }

    #[test]
    fn gaps_and_unknown_chronology_are_reported_as_provisional() {
        let scenes = [
            scene("a", None, &[], &["plot"]),
            scene("b", None, &[], &[]),
            scene("c", None, &[], &["plot"]),
        ];
        assert_eq!(subplot_gaps(&scenes, 1)[0].intervening_scenes, 1);
        assert_eq!(chronology_ambiguities(&scenes).len(), 3);
    }

    #[test]
    fn same_character_chronology_distinguishes_confirmed_possible_and_unverified_overlap() {
        let character = |id: &str| CharacterOnScene {
            id: id.into(),
            name: "Mara".into(),
            role: ParticipationRole::Major,
        };
        let mut first = scene("first", None, &["Mara"], &[]);
        first.characters = vec![character("mara-id")];
        first.start_time = story_core::StoryTime::exact(0);
        first.end_time = story_core::StoryTime::exact(10);
        let mut definite = scene("definite", None, &["Mara"], &[]);
        definite.characters = vec![character("mara-id")];
        definite.start_time = story_core::StoryTime::exact(5);
        definite.end_time = story_core::StoryTime::exact(20);
        let finding = character_chronology_findings(&[first.clone(), definite]).remove(0);
        assert_eq!(finding.status, CharacterChronologyStatus::DefiniteOverlap);
        assert_eq!(finding.shared_character_ids, vec!["mara-id"]);

        let mut possible = scene("possible", None, &["Mara"], &[]);
        possible.characters = vec![character("mara-id")];
        possible.start_time = story_core::StoryTime::new(
            None,
            Some(10),
            Some(14),
            story_core::TimePrecision::Bounded,
        )
        .unwrap();
        possible.end_time = story_core::StoryTime::exact(20);
        let mut uncertain_end = first.clone();
        uncertain_end.end_time =
            story_core::StoryTime::new(None, Some(8), Some(12), story_core::TimePrecision::Bounded)
                .unwrap();
        assert_eq!(
            character_chronology_findings(&[uncertain_end, possible])[0].status,
            CharacterChronologyStatus::PossibleOverlap
        );

        let mut unknown = scene("unknown", None, &["Mara"], &[]);
        unknown.characters = vec![character("mara-id")];
        assert_eq!(
            character_chronology_findings(&[first.clone(), unknown])[0].status,
            CharacterChronologyStatus::Unverified
        );

        let mut adjacent = scene("adjacent", None, &["Mara"], &[]);
        adjacent.characters = vec![character("mara-id")];
        adjacent.start_time = story_core::StoryTime::exact(10);
        adjacent.end_time = story_core::StoryTime::exact(15);
        assert!(character_chronology_findings(&[first.clone(), adjacent]).is_empty());

        let mut unrelated = scene("unrelated", None, &["Mara"], &[]);
        unrelated.characters = vec![character("different-id")];
        unrelated.start_time = story_core::StoryTime::exact(5);
        unrelated.end_time = story_core::StoryTime::exact(20);
        assert!(character_chronology_findings(&[first, unrelated]).is_empty());
    }
}

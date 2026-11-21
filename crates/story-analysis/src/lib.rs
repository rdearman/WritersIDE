//! Read-only analytical projections over the canonical story model.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use story_core::{ParticipationRole, TimeOrdering};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProofreadingConfig {
    pub version: u16,
    pub rules: Vec<ProofreadingRule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofreadingRule {
    pub id: String,
    pub enabled: bool,
    /// Built-ins: `repeated-word`, `double-space`, `space-before-punctuation`; custom: `regex`.
    pub kind: String,
    pub pattern: Option<String>,
    pub severity: String,
    pub message: String,
}

impl Default for ProofreadingConfig {
    fn default() -> Self {
        Self {
            version: 1,
            rules: vec![
                ProofreadingRule {
                    id: "repeated-word".into(),
                    enabled: true,
                    kind: "repeated-word".into(),
                    pattern: None,
                    severity: "warning".into(),
                    message: "Repeated word".into(),
                },
                ProofreadingRule {
                    id: "double-space".into(),
                    enabled: true,
                    kind: "double-space".into(),
                    pattern: None,
                    severity: "information".into(),
                    message: "Repeated spaces".into(),
                },
                ProofreadingRule {
                    id: "space-before-punctuation".into(),
                    enabled: true,
                    kind: "space-before-punctuation".into(),
                    pattern: None,
                    severity: "information".into(),
                    message: "Space before punctuation".into(),
                },
            ],
        }
    }
}

impl ProofreadingConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!(
                "unsupported diagnostics configuration version {}",
                self.version
            ));
        }
        let mut ids = BTreeSet::new();
        for rule in &self.rules {
            if rule.id.trim().is_empty() || !ids.insert(rule.id.as_str()) {
                return Err(format!(
                    "diagnostic rule ID is empty or duplicated: {}",
                    rule.id
                ));
            }
            if !matches!(rule.severity.as_str(), "error" | "warning" | "information") {
                return Err(format!("invalid severity in rule {}", rule.id));
            }
            match rule.kind.as_str() {
                "repeated-word" | "double-space" | "space-before-punctuation" => {}
                "regex" => {
                    let pattern = rule
                        .pattern
                        .as_deref()
                        .ok_or_else(|| format!("regex rule {} has no pattern", rule.id))?;
                    regex::Regex::new(pattern)
                        .map_err(|error| format!("invalid regex in rule {}: {error}", rule.id))?;
                }
                other => return Err(format!("unknown diagnostic rule kind: {other}")),
            }
        }
        Ok(())
    }
    pub fn to_toml(&self) -> Result<String, String> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|error| error.to_string())
    }
    pub fn from_toml(source: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(source).map_err(|error| error.to_string())?;
        config.validate()?;
        Ok(config)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofreadingFinding {
    pub byte_range: std::ops::Range<usize>,
    pub category: String,
    pub severity: String,
    pub message: String,
}

#[cfg(test)]
mod proofread_tests {
    use super::{ProofreadingConfig, ProofreadingRule, proofread, proofread_with_config};

    #[test]
    fn mechanical_findings_are_unicode_byte_mapped_and_non_mutating() {
        let text = "Élodie said said  hello !";
        let findings = proofread(text);
        assert_eq!(
            findings
                .iter()
                .map(|finding| finding.category.as_str())
                .collect::<Vec<_>>(),
            ["repeated-word", "double-space", "space-before-punctuation"]
        );
        for finding in findings {
            assert!(text.get(finding.byte_range).is_some());
        }
        assert_eq!(text, "Élodie said said  hello !");
    }

    #[test]
    fn configurable_regex_rules_round_trip_validate_and_report_utf8_offsets() {
        let mut config = ProofreadingConfig::default();
        config.rules.push(ProofreadingRule {
            id: "passive-voice-hint".into(),
            enabled: true,
            kind: "regex".into(),
            pattern: Some(r"\bwas\s+\w+ed\b".into()),
            severity: "warning".into(),
            message: "Review possible passive voice".into(),
        });
        let serialized = config.to_toml().unwrap();
        let parsed = ProofreadingConfig::from_toml(&serialized).unwrap();
        let text = "Élodie was followed.";
        let finding = proofread_with_config(text, &parsed)
            .unwrap()
            .into_iter()
            .find(|finding| finding.category == "passive-voice-hint")
            .unwrap();
        assert_eq!(&text[finding.byte_range.clone()], "was followed");
        assert_eq!(finding.severity, "warning");
        assert!(ProofreadingConfig::from_toml("version=1\n[[rules]]\nid='x'\nenabled=true\nkind='regex'\npattern='['\nseverity='warning'\nmessage='bad'").is_err());
    }
}

/// Deterministic mechanical checks. These are suggestions and never edit manuscript text.
pub fn proofread(text: &str) -> Vec<ProofreadingFinding> {
    proofread_with_config(text, &ProofreadingConfig::default())
        .expect("default proofreader config is valid")
}

/// Runs deterministic advisory rules without modifying source text.
pub fn proofread_with_config(
    text: &str,
    config: &ProofreadingConfig,
) -> Result<Vec<ProofreadingFinding>, String> {
    config.validate()?;
    let mut findings = Vec::new();
    for rule in config.rules.iter().filter(|rule| rule.enabled) {
        match rule.kind.as_str() {
            "repeated-word" => {
                let words = text.unicode_word_indices().collect::<Vec<_>>();
                for pair in words.windows(2) {
                    let (a_start, a) = pair[0];
                    let (b_start, b) = pair[1];
                    let between = &text[a_start + a.len()..b_start];
                    if !between.is_empty()
                        && between.chars().all(char::is_whitespace)
                        && a.eq_ignore_ascii_case(b)
                    {
                        findings.push(finding(
                            rule,
                            b_start..b_start + b.len(),
                            format!("{}: {b}", rule.message),
                        ));
                    }
                }
            }
            "double-space" => {
                for (offset, _) in text.match_indices("  ") {
                    findings.push(finding(rule, offset..offset + 2, rule.message.clone()));
                }
            }
            "space-before-punctuation" => {
                let chars = text.char_indices().collect::<Vec<_>>();
                for pair in chars.windows(2) {
                    let (offset, ch) = pair[0];
                    let (next_offset, next) = pair[1];
                    if ch == ' ' && matches!(next, ',' | '.' | ';' | ':' | '!' | '?') {
                        findings.push(finding(
                            rule,
                            offset..next_offset + next.len_utf8(),
                            format!("{}: {next}", rule.message),
                        ));
                    }
                }
            }
            "regex" => {
                let regex =
                    regex::Regex::new(rule.pattern.as_deref().expect("validated regex rule"))
                        .map_err(|error| error.to_string())?;
                for matched in regex.find_iter(text) {
                    findings.push(finding(
                        rule,
                        matched.start()..matched.end(),
                        format!("{}: {}", rule.message, matched.as_str()),
                    ));
                }
            }
            _ => unreachable!("validated rule kind"),
        }
    }
    findings.sort_by_key(|finding| (finding.byte_range.start, finding.category.clone()));
    Ok(findings)
}

fn finding(
    rule: &ProofreadingRule,
    byte_range: std::ops::Range<usize>,
    message: String,
) -> ProofreadingFinding {
    ProofreadingFinding {
        byte_range,
        category: rule.id.clone(),
        severity: rule.severity.clone(),
        message,
    }
}

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

/// Explicit, read-only coverage expectations for story-planning reviews.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageRequirement {
    pub id: String,
    pub label: String,
    pub entity_id: String,
    pub dimension: CoverageDimension,
    pub minimum_scenes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageDimension {
    Character,
    Plotline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageResult {
    pub requirement_id: String,
    pub label: String,
    pub matching_scene_ids: Vec<String>,
    pub minimum_scenes: usize,
    pub coverage_met: bool,
}

/// Evaluate author-defined coverage requirements without changing the manuscript.
pub fn evaluate_coverage(
    scenes: &[SceneAnalysisInput],
    requirements: &[CoverageRequirement],
) -> Vec<CoverageResult> {
    requirements
        .iter()
        .map(|requirement| {
            let matching = scenes
                .iter()
                .filter(|scene| match requirement.dimension {
                    CoverageDimension::Character => scene
                        .characters
                        .iter()
                        .any(|item| item.id == requirement.entity_id),
                    CoverageDimension::Plotline => scene
                        .plotlines
                        .iter()
                        .any(|item| item.id == requirement.entity_id),
                })
                .map(|scene| scene.scene_id.clone())
                .collect::<BTreeSet<_>>();
            CoverageResult {
                requirement_id: requirement.id.clone(),
                label: requirement.label.clone(),
                matching_scene_ids: matching.iter().cloned().collect(),
                minimum_scenes: requirement.minimum_scenes,
                coverage_met: matching.len() >= requirement.minimum_scenes,
            }
        })
        .collect()
}

/// A saved-query assertion is a declarative expectation over the current result set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryAssertion {
    AtLeast(usize),
    AtMost(usize),
    Exactly(usize),
    NonEmpty,
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssertionResult {
    pub passed: bool,
    pub actual: usize,
    pub expectation: String,
}

pub fn evaluate_query_assertion(actual: usize, assertion: &QueryAssertion) -> AssertionResult {
    let (passed, expectation) = match assertion {
        QueryAssertion::AtLeast(n) => (actual >= *n, format!("at least {n}")),
        QueryAssertion::AtMost(n) => (actual <= *n, format!("at most {n}")),
        QueryAssertion::Exactly(n) => (actual == *n, format!("exactly {n}")),
        QueryAssertion::NonEmpty => (actual > 0, "non-empty".into()),
        QueryAssertion::Empty => (actual == 0, "empty".into()),
    };
    AssertionResult {
        passed,
        actual,
        expectation,
    }
}

#[cfg(test)]
mod inspection_tests {
    use super::*;

    #[test]
    fn coverage_and_query_assertions_are_read_only_and_report_gaps() {
        let mut scenes = vec![SceneAnalysisInput {
            scene_id: "s1".into(),
            title: "One".into(),
            chapter_id: None,
            actual_words: 0,
            target_words: None,
            start_time: story_core::StoryTime::unknown(),
            end_time: story_core::StoryTime::unknown(),
            characters: vec![],
            plotlines: vec![],
        }];
        scenes[0].characters.push(CharacterOnScene {
            id: "c1".into(),
            name: "Ada".into(),
            role: ParticipationRole::Major,
        });
        let requirement = CoverageRequirement {
            id: "arc-a".into(),
            label: "Arc A appears".into(),
            entity_id: "c1".into(),
            dimension: CoverageDimension::Character,
            minimum_scenes: 2,
        };
        let coverage = evaluate_coverage(&scenes, &[requirement]);
        assert_eq!(coverage[0].matching_scene_ids, ["s1"]);
        assert!(!coverage[0].coverage_met);
        assert!(evaluate_query_assertion(1, &QueryAssertion::Exactly(1)).passed);
        assert!(!evaluate_query_assertion(0, &QueryAssertion::NonEmpty).passed);
    }
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

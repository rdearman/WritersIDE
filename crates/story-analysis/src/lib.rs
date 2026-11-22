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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofreadingPreset {
    Drafting,
    Copyedit,
    Publication,
    Quiet,
    Thorough,
    Extended,
    Pedantic,
}

impl ProofreadingPreset {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "drafting" | "standard" => Some(Self::Drafting),
            "copyedit" => Some(Self::Copyedit),
            "publication" => Some(Self::Publication),
            "quiet" => Some(Self::Quiet),
            "thorough" => Some(Self::Thorough),
            "extended" => Some(Self::Extended),
            "pedantic" => Some(Self::Pedantic),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Drafting => "drafting",
            Self::Copyedit => "copyedit",
            Self::Publication => "publication",
            Self::Quiet => "quiet",
            Self::Thorough => "thorough",
            Self::Extended => "extended",
            Self::Pedantic => "pedantic",
        }
    }
}

impl Default for ProofreadingConfig {
    fn default() -> Self {
        Self::preset(ProofreadingPreset::Drafting)
    }
}

impl ProofreadingConfig {
    pub fn preset(preset: ProofreadingPreset) -> Self {
        let mut config = Self {
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
        };
        match preset {
            ProofreadingPreset::Drafting => {}
            ProofreadingPreset::Copyedit => {
                for rule in &mut config.rules {
                    rule.severity = "warning".into();
                }
            }
            ProofreadingPreset::Publication => {
                for rule in &mut config.rules {
                    rule.severity = if rule.kind == "repeated-word" {
                        "error"
                    } else {
                        "warning"
                    }
                    .into();
                }
            }
            ProofreadingPreset::Quiet => {
                for rule in &mut config.rules {
                    rule.severity = "off".into();
                }
            }
            ProofreadingPreset::Thorough | ProofreadingPreset::Extended => {
                for rule in &mut config.rules {
                    rule.severity = "warning".into();
                }
            }
            ProofreadingPreset::Pedantic => {
                for rule in &mut config.rules {
                    rule.severity = "error".into();
                }
            }
        }
        config
    }

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
            if !matches!(
                rule.severity.as_str(),
                "off" | "error" | "warning" | "information" | "info"
            ) {
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypographySuggestion {
    pub byte_range: std::ops::Range<usize>,
    pub original: String,
    pub replacement: String,
    pub message: String,
}

/// Return optional, non-mutating typography suggestions for a document language.
/// Suggestions are byte-mapped to the original UTF-8 text and require explicit author action.
pub fn typography_suggestions(text: &str, language: &str) -> Vec<TypographySuggestion> {
    let primary = language
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut suggestions = Vec::new();

    if primary == "fr" {
        let chars = text.char_indices().collect::<Vec<_>>();
        for pair in chars.windows(2) {
            let (space_start, space) = pair[0];
            let (punctuation_start, punctuation) = pair[1];
            if matches!(space, ' ' | '\u{00a0}')
                && matches!(punctuation, ';' | ':' | '!' | '?')
                && space_start > 0
                && !text[..space_start]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace)
            {
                let end = punctuation_start + punctuation.len_utf8();
                suggestions.push(TypographySuggestion {
                    byte_range: space_start..end,
                    original: text[space_start..end].to_owned(),
                    replacement: format!("\u{202f}{punctuation}"),
                    message: "French typography convention: use a narrow no-break space before this punctuation".into(),
                });
            }
        }
        let quote_offsets = text
            .char_indices()
            .filter_map(|(offset, character)| (character == '"').then_some(offset))
            .collect::<Vec<_>>();
        for pair in quote_offsets.as_chunks::<2>().0 {
            for (offset, replacement) in [(pair[0], "«\u{202f}"), (pair[1], "\u{202f}»")] {
                suggestions.push(TypographySuggestion {
                    byte_range: offset..offset + 1,
                    original: "\"".into(),
                    replacement: replacement.into(),
                    message: "French quotation style suggestion".into(),
                });
            }
        }
    } else if primary == "it" {
        let chars = text.char_indices().collect::<Vec<_>>();
        for (index, (offset, character)) in chars.iter().copied().enumerate() {
            if character == '\''
                && index > 0
                && index + 1 < chars.len()
                && chars[index - 1].1.is_alphabetic()
                && chars[index + 1].1.is_alphabetic()
            {
                suggestions.push(TypographySuggestion {
                    byte_range: offset..offset + character.len_utf8(),
                    original: "'".into(),
                    replacement: "’".into(),
                    message: "Italian typographic apostrophe suggestion".into(),
                });
            }
        }
        let quote_offsets = text
            .char_indices()
            .filter_map(|(offset, character)| (character == '"').then_some(offset))
            .collect::<Vec<_>>();
        for pair in quote_offsets.as_chunks::<2>().0 {
            for (offset, replacement) in [(pair[0], "“"), (pair[1], "”")] {
                suggestions.push(TypographySuggestion {
                    byte_range: offset..offset + 1,
                    original: "\"".into(),
                    replacement: replacement.into(),
                    message: "Italian quotation style suggestion".into(),
                });
            }
        }
    }
    suggestions.sort_by_key(|suggestion| (suggestion.byte_range.start, suggestion.byte_range.end));
    suggestions
}

#[cfg(test)]
mod proofread_tests {
    use super::{
        ProofreadingConfig, ProofreadingPreset, ProofreadingRule, proofread, proofread_with_config,
        typography_suggestions,
    };

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

    #[test]
    fn diagnostic_presets_are_named_deterministic_and_keep_custom_rules_editable() {
        let drafting = ProofreadingConfig::preset(ProofreadingPreset::Drafting);
        let copyedit = ProofreadingConfig::preset(ProofreadingPreset::Copyedit);
        let publication = ProofreadingConfig::preset(ProofreadingPreset::Publication);
        assert_eq!(drafting.rules[0].severity, "warning");
        assert_eq!(drafting.rules[1].severity, "information");
        assert!(copyedit.rules.iter().all(|rule| rule.severity == "warning"));
        assert_eq!(publication.rules[0].severity, "error");
        assert_eq!(publication.rules[1].severity, "warning");
        let quiet = ProofreadingConfig::preset(ProofreadingPreset::Quiet);
        assert!(quiet.rules.iter().all(|rule| rule.severity == "off"));
        assert!(
            proofread_with_config("word word", &quiet)
                .unwrap()
                .is_empty()
        );
        let mut info_alias = drafting.clone();
        info_alias.rules[0].severity = "info".into();
        assert_eq!(
            proofread_with_config("word word", &info_alias).unwrap()[0].severity,
            "information"
        );
        assert_eq!(
            ProofreadingPreset::parse("standard"),
            Some(ProofreadingPreset::Drafting)
        );
        assert_eq!(
            ProofreadingPreset::parse("copyedit").unwrap().name(),
            "copyedit"
        );
        assert!(ProofreadingPreset::parse("unknown").is_none());
        for config in [drafting, copyedit, publication, quiet] {
            assert_eq!(
                ProofreadingConfig::from_toml(&config.to_toml().unwrap()).unwrap(),
                config
            );
        }
    }

    #[test]
    fn typography_suggestions_are_language_scoped_non_mutating_and_utf8_mapped() {
        let french = "Déjà vu ! \"où ?\"";
        let suggestions = typography_suggestions(french, "fr-FR");
        assert_eq!(french, "Déjà vu ! \"où ?\"");
        assert!(suggestions.iter().any(|suggestion| {
            suggestion.original == " !"
                && suggestion.replacement == "\u{202f}!"
                && suggestion.byte_range.start > 0
        }));
        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.replacement == "«\u{202f}")
        );
        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.replacement == "\u{202f}»")
        );

        let italian = "L'amore è \"qui\".";
        let suggestions = typography_suggestions(italian, "it-IT");
        assert!(
            suggestions.iter().any(|suggestion| {
                suggestion.original == "'" && suggestion.replacement == "’"
            })
        );
        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.replacement == "“")
        );
        assert!(
            suggestions
                .iter()
                .any(|suggestion| suggestion.replacement == "”")
        );
        assert!(typography_suggestions("Where?", "en-GB").is_empty());
        assert!(typography_suggestions("안녕하세요?", "ko-KR").is_empty());
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
    for rule in config
        .rules
        .iter()
        .filter(|rule| rule.enabled && rule.severity != "off")
    {
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
        severity: if rule.severity == "info" {
            "information".into()
        } else {
            rule.severity.clone()
        },
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoryEntityOnScene {
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
    pub start_boundary: story_core::IntervalBoundary,
    pub end_boundary: story_core::IntervalBoundary,
    pub characters: Vec<CharacterOnScene>,
    pub plotlines: Vec<PlotlineOnScene>,
    pub locations: Vec<StoryEntityOnScene>,
    pub events: Vec<StoryEntityOnScene>,
}

fn scene_interval(scene: &SceneAnalysisInput) -> story_core::StoryInterval {
    story_core::StoryInterval {
        start: scene.start_time,
        end: scene.end_time,
        start_boundary: scene.start_boundary,
        end_boundary: scene.end_boundary,
    }
}

fn interval_point(
    time: story_core::StoryTime,
    boundary: story_core::IntervalBoundary,
) -> story_core::StoryInterval {
    story_core::StoryInterval {
        start: time,
        end: time,
        start_boundary: boundary,
        end_boundary: boundary,
    }
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
    Pov,
    Location,
    Event,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
                    CoverageDimension::Character => scene.characters.iter().any(|item| {
                        item.id == requirement.entity_id
                            && item.role != ParticipationRole::Mentioned
                    }),
                    CoverageDimension::Plotline => scene
                        .plotlines
                        .iter()
                        .any(|item| item.id == requirement.entity_id),
                    CoverageDimension::Pov => scene.characters.iter().any(|item| {
                        item.id == requirement.entity_id && item.role == ParticipationRole::Pov
                    }),
                    CoverageDimension::Location => scene
                        .locations
                        .iter()
                        .any(|item| item.id == requirement.entity_id),
                    CoverageDimension::Event => scene
                        .events
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssertionResult {
    pub passed: bool,
    pub actual: usize,
    pub expectation: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum InspectionStatus {
    Pass,
    Fail,
    Unknown,
    Skip,
}

/// Deterministic author assertions never infer success from absent records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InspectionResult {
    pub status: InspectionStatus,
    pub assertion: String,
    pub evidence: Vec<String>,
}

/// Evaluate an explicitly recorded event-order assertion. Ambiguous intervals stay unknown.
pub fn evaluate_event_precedence(
    earlier_label: &str,
    later_label: &str,
    comparison: Option<TimeOrdering>,
) -> InspectionResult {
    let status = match comparison {
        Some(TimeOrdering::Before) => InspectionStatus::Pass,
        Some(TimeOrdering::After) => InspectionStatus::Fail,
        Some(TimeOrdering::Same | TimeOrdering::Ambiguous) | None => InspectionStatus::Unknown,
    };
    let evidence = comparison
        .map(|value| vec![format!("Recorded interval comparison: {value:?}")])
        .unwrap_or_else(|| vec!["One or both events have no usable recorded story time".into()]);
    InspectionResult {
        status,
        assertion: format!("{earlier_label} precedes {later_label}"),
        evidence,
    }
}

/// Evaluate knowledge only when the author has recorded whether the character knows the fact.
pub fn evaluate_recorded_knowledge(
    character: &str,
    claim: &str,
    recorded_knows: Option<bool>,
    check_enabled: bool,
    evidence: Vec<String>,
) -> InspectionResult {
    let status = if !check_enabled {
        InspectionStatus::Skip
    } else {
        match recorded_knows {
            Some(true) => InspectionStatus::Pass,
            Some(false) => InspectionStatus::Fail,
            None => InspectionStatus::Unknown,
        }
    };
    let evidence = if evidence.is_empty() && recorded_knows.is_none() {
        vec!["No author-approved knowledge record exists; this is unknown, not a pass".into()]
    } else {
        evidence
    };
    InspectionResult {
        status,
        assertion: format!("{character} knows {claim}"),
        evidence,
    }
}

pub fn evaluate_query_assertion(
    actual: usize,
    assertion: &story_core::QueryAssertion,
) -> AssertionResult {
    let actual_u64 = u64::try_from(actual).unwrap_or(u64::MAX);
    let (passed, expectation) = match assertion {
        story_core::QueryAssertion::AtLeast(n) => (actual_u64 >= *n, format!("at least {n}")),
        story_core::QueryAssertion::AtMost(n) => (actual_u64 <= *n, format!("at most {n}")),
        story_core::QueryAssertion::Exactly(n) => (actual_u64 == *n, format!("exactly {n}")),
        story_core::QueryAssertion::NonEmpty => (actual > 0, "non-empty".into()),
        story_core::QueryAssertion::Empty => (actual == 0, "empty".into()),
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
    fn recorded_story_assertions_never_turn_missing_or_ambiguous_facts_into_passes() {
        assert_eq!(
            evaluate_event_precedence("arrival", "departure", Some(TimeOrdering::Before)).status,
            InspectionStatus::Pass
        );
        assert_eq!(
            evaluate_event_precedence("arrival", "departure", Some(TimeOrdering::After)).status,
            InspectionStatus::Fail
        );
        assert_eq!(
            evaluate_event_precedence("arrival", "departure", Some(TimeOrdering::Ambiguous)).status,
            InspectionStatus::Unknown
        );
        let missing = evaluate_recorded_knowledge("Mara", "the secret", None, true, vec![]);
        assert_eq!(missing.status, InspectionStatus::Unknown);
        assert!(missing.evidence[0].contains("unknown, not a pass"));
        assert_eq!(
            evaluate_recorded_knowledge("Mara", "the secret", None, false, vec![]).status,
            InspectionStatus::Skip
        );
    }

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
            start_boundary: story_core::IntervalBoundary::Inclusive,
            end_boundary: story_core::IntervalBoundary::Inclusive,
            characters: vec![],
            plotlines: vec![],
            locations: vec![],
            events: vec![],
        }];
        scenes[0].characters.push(CharacterOnScene {
            id: "c1".into(),
            name: "Ada".into(),
            role: ParticipationRole::Major,
        });
        scenes[0].characters.push(CharacterOnScene {
            id: "c-pov".into(),
            name: "POV".into(),
            role: ParticipationRole::Pov,
        });
        scenes[0].locations.push(StoryEntityOnScene {
            id: "loc".into(),
            name: "Dock".into(),
        });
        scenes[0].events.push(StoryEntityOnScene {
            id: "event".into(),
            name: "Signal".into(),
        });
        scenes[0].characters.push(CharacterOnScene {
            id: "c-mentioned".into(),
            name: "Named only".into(),
            role: ParticipationRole::Mentioned,
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
        let mention_only = CoverageRequirement {
            id: "mention-only".into(),
            label: "Mention is not presence".into(),
            entity_id: "c-mentioned".into(),
            dimension: CoverageDimension::Character,
            minimum_scenes: 1,
        };
        assert!(!evaluate_coverage(&scenes, &[mention_only])[0].coverage_met);
        for (dimension, entity_id) in [
            (CoverageDimension::Pov, "c-pov"),
            (CoverageDimension::Location, "loc"),
            (CoverageDimension::Event, "event"),
        ] {
            let requirement = CoverageRequirement {
                id: entity_id.into(),
                label: entity_id.into(),
                entity_id: entity_id.into(),
                dimension,
                minimum_scenes: 1,
            };
            assert!(evaluate_coverage(&scenes, &[requirement])[0].coverage_met);
        }
        assert!(evaluate_query_assertion(1, &story_core::QueryAssertion::Exactly(1)).passed);
        assert!(!evaluate_query_assertion(0, &story_core::QueryAssertion::NonEmpty).passed);
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
            let a_interval = scene_interval(a);
            let b_interval = scene_interval(b);
            let definitely_apart = matches!(a_interval.compare(b_interval), TimeOrdering::Before)
                || matches!(a_interval.compare(b_interval), TimeOrdering::After);
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
                .filter(|character| character.role != ParticipationRole::Mentioned)
                .map(|character| character.id.as_str())
                .collect::<BTreeSet<_>>();
            let shared_character_ids = second
                .characters
                .iter()
                .filter(|character| character.role != ParticipationRole::Mentioned)
                .filter(|character| first_characters.contains(character.id.as_str()))
                .map(|character| character.id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if shared_character_ids.is_empty() {
                continue;
            }

            let first_interval = scene_interval(first);
            let second_interval = scene_interval(second);
            let first_before_second = matches!(first_interval.compare(second_interval), Before);
            let second_before_first = matches!(first_interval.compare(second_interval), After);
            if first_before_second || second_before_first {
                continue;
            }

            let first_starts_before_second_ends =
                interval_point(first.start_time, first.start_boundary)
                    .compare(interval_point(second.end_time, second.end_boundary));
            let second_starts_before_first_ends =
                interval_point(second.start_time, second.start_boundary)
                    .compare(interval_point(first.end_time, first.end_boundary));
            let definite_overlap = matches!(first_starts_before_second_ends, Before | Same)
                && matches!(second_starts_before_first_ends, Before | Same);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LocationRuleFindingStatus {
    DefiniteConflict,
    PossibleConflict,
    Unverified,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocationRuleFinding {
    pub rule_id: String,
    pub first_scene_id: String,
    pub second_scene_id: String,
    pub shared_character_ids: Vec<String>,
    pub status: LocationRuleFindingStatus,
    pub description: String,
}

/// Applies explicit author-defined incompatible-location rules to scenes with a
/// shared participant. Missing or unbounded chronology remains unverified.
pub fn location_incompatibility_findings(
    scenes: &[SceneAnalysisInput],
    rules: &[story_core::LocationIncompatibilityRule],
) -> Vec<LocationRuleFinding> {
    use LocationRuleFindingStatus::*;
    use TimeOrdering::*;
    let mut findings = Vec::new();
    for rule in rules.iter().filter(|rule| rule.enabled) {
        let first_location = rule.first_location.to_string();
        let second_location = rule.second_location.to_string();
        for (index, first) in scenes.iter().enumerate() {
            let first_has_a = first.locations.iter().any(|item| item.id == first_location);
            let first_has_b = first
                .locations
                .iter()
                .any(|item| item.id == second_location);
            for second in scenes.iter().skip(index + 1) {
                let second_has_a = second
                    .locations
                    .iter()
                    .any(|item| item.id == first_location);
                let second_has_b = second
                    .locations
                    .iter()
                    .any(|item| item.id == second_location);
                if !((first_has_a && second_has_b) || (first_has_b && second_has_a)) {
                    continue;
                }
                let first_chars = first
                    .characters
                    .iter()
                    .filter(|item| item.role != ParticipationRole::Mentioned)
                    .map(|item| item.id.as_str())
                    .collect::<BTreeSet<_>>();
                let shared = second
                    .characters
                    .iter()
                    .filter(|item| item.role != ParticipationRole::Mentioned)
                    .filter(|item| first_chars.contains(item.id.as_str()))
                    .map(|item| item.id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                if shared.is_empty() {
                    continue;
                }
                let first_interval = scene_interval(first);
                let second_interval = scene_interval(second);
                if matches!(first_interval.compare(second_interval), Before | After) {
                    continue;
                }
                let bounded = [
                    first.start_time,
                    first.end_time,
                    second.start_time,
                    second.end_time,
                ]
                .into_iter()
                .all(|time| time.lower.is_some() && time.upper.is_some());
                let definite = matches!(
                    interval_point(first.start_time, first.start_boundary)
                        .compare(interval_point(second.end_time, second.end_boundary)),
                    Before | Same
                ) && matches!(
                    interval_point(second.start_time, second.start_boundary)
                        .compare(interval_point(first.end_time, first.end_boundary)),
                    Before | Same
                );
                let status = if definite {
                    DefiniteConflict
                } else if bounded {
                    PossibleConflict
                } else {
                    Unverified
                };
                findings.push(LocationRuleFinding {
                    rule_id: rule.id.to_string(),
                    first_scene_id: first.scene_id.clone(),
                    second_scene_id: second.scene_id.clone(),
                    shared_character_ids: shared,
                    status,
                    description: rule.description.clone(),
                });
            }
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
            start_boundary: story_core::IntervalBoundary::Inclusive,
            end_boundary: story_core::IntervalBoundary::Inclusive,
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
            locations: vec![],
            events: vec![],
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
        adjacent.start_boundary = story_core::IntervalBoundary::Exclusive;
        assert!(character_chronology_findings(&[first.clone(), adjacent]).is_empty());

        let mut unrelated = scene("unrelated", None, &["Mara"], &[]);
        unrelated.characters = vec![character("different-id")];
        unrelated.start_time = story_core::StoryTime::exact(5);
        unrelated.end_time = story_core::StoryTime::exact(20);
        assert!(character_chronology_findings(&[first, unrelated]).is_empty());
    }

    #[test]
    fn mentioned_characters_do_not_count_as_scene_presence_for_chronology() {
        let mut first = scene("first", None, &["Mara"], &[]);
        let mut second = scene("second", None, &["Mara"], &[]);
        first.characters[0].role = ParticipationRole::Mentioned;
        second.start_time = story_core::StoryTime::exact(1);
        second.end_time = story_core::StoryTime::exact(2);
        assert!(character_chronology_findings(&[first, second]).is_empty());
    }

    #[test]
    fn location_incompatibility_rules_report_definite_possible_and_unknown_cases() {
        let location_a = story_core::LocationId::new();
        let location_b = story_core::LocationId::new();
        let rule = story_core::LocationIncompatibilityRule {
            id: story_core::ContinuityRuleId::new(),
            first_location: location_a,
            second_location: location_b,
            description: "The sealed room and the bridge are too far apart".into(),
            enabled: true,
        };
        let mut first = scene("first", None, &["Mara"], &[]);
        first.locations.push(StoryEntityOnScene {
            id: location_a.to_string(),
            name: "Sealed room".into(),
        });
        first.start_time = story_core::StoryTime::exact(0);
        first.end_time = story_core::StoryTime::exact(10);
        let mut definite = scene("definite", None, &["Mara"], &[]);
        definite.locations.push(StoryEntityOnScene {
            id: location_b.to_string(),
            name: "Bridge".into(),
        });
        definite.start_time = story_core::StoryTime::exact(5);
        definite.end_time = story_core::StoryTime::exact(15);
        assert_eq!(
            location_incompatibility_findings(
                &[first.clone(), definite],
                std::slice::from_ref(&rule)
            )[0]
            .status,
            LocationRuleFindingStatus::DefiniteConflict
        );

        let mut possible = scene("possible", None, &["Mara"], &[]);
        possible.locations.push(StoryEntityOnScene {
            id: location_b.to_string(),
            name: "Bridge".into(),
        });
        possible.start_time = story_core::StoryTime::new(
            Some(12),
            Some(8),
            Some(14),
            story_core::TimePrecision::Bounded,
        )
        .unwrap();
        possible.end_time = story_core::StoryTime::exact(20);
        assert_eq!(
            location_incompatibility_findings(
                &[first.clone(), possible],
                std::slice::from_ref(&rule)
            )[0]
            .status,
            LocationRuleFindingStatus::PossibleConflict
        );

        let mut unknown = scene("unknown", None, &["Mara"], &[]);
        unknown.locations.push(StoryEntityOnScene {
            id: location_b.to_string(),
            name: "Bridge".into(),
        });
        assert_eq!(
            location_incompatibility_findings(&[first, unknown], std::slice::from_ref(&rule))[0]
                .status,
            LocationRuleFindingStatus::Unverified
        );
        let disabled = story_core::LocationIncompatibilityRule {
            enabled: false,
            ..rule
        };
        assert!(location_incompatibility_findings(&[], &[disabled]).is_empty());
    }

    #[test]
    fn location_rules_respect_exclusive_touching_bounds_and_ignore_same_location() {
        let location_a = story_core::LocationId::new();
        let location_b = story_core::LocationId::new();
        let rule = story_core::LocationIncompatibilityRule {
            id: story_core::ContinuityRuleId::new(),
            first_location: location_a,
            second_location: location_b,
            description: "Cannot be at both places at once".into(),
            enabled: true,
        };
        let mut first = scene("first", None, &["Mara"], &[]);
        first.locations.push(StoryEntityOnScene {
            id: location_a.to_string(),
            name: "Harbour".into(),
        });
        first.start_time = story_core::StoryTime::exact(0);
        first.end_time = story_core::StoryTime::exact(10);
        let mut second = scene("second", None, &["Mara"], &[]);
        second.locations.push(StoryEntityOnScene {
            id: location_b.to_string(),
            name: "Palace".into(),
        });
        second.start_time = story_core::StoryTime::exact(10);
        second.end_time = story_core::StoryTime::exact(20);
        second.start_boundary = story_core::IntervalBoundary::Exclusive;
        assert!(
            location_incompatibility_findings(
                &[first.clone(), second.clone()],
                std::slice::from_ref(&rule)
            )
            .is_empty()
        );

        second.start_boundary = story_core::IntervalBoundary::Inclusive;
        assert_eq!(
            location_incompatibility_findings(
                &[first.clone(), second],
                std::slice::from_ref(&rule)
            )[0]
            .status,
            LocationRuleFindingStatus::DefiniteConflict
        );

        assert!(location_incompatibility_findings(&[first.clone(), first], &[rule]).is_empty());
    }
}

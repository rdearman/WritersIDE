use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

/// Expands a `${cursor}` marker while preserving its UTF-8 byte position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnippetExpansion {
    pub text: String,
    pub cursor_byte: usize,
}

pub fn expand_snippet(
    source: &str,
    values: &BTreeMap<String, String>,
) -> Result<SnippetExpansion, String> {
    let mut text = String::new();
    let mut cursor_byte = None;
    let mut rest = source;
    while let Some(open) = rest.find("${") {
        text.push_str(&rest[..open]);
        rest = &rest[open + 2..];
        let close = rest.find('}').ok_or("unterminated snippet placeholder")?;
        let name = &rest[..close];
        if name == "cursor" {
            if cursor_byte.replace(text.len()).is_some() {
                return Err("snippet has more than one cursor marker".into());
            }
        } else {
            text.push_str(
                values
                    .get(name)
                    .ok_or_else(|| format!("missing snippet value: {name}"))?,
            );
        }
        rest = &rest[close + 1..];
    }
    text.push_str(rest);
    let cursor_byte = cursor_byte.unwrap_or(text.len());
    Ok(SnippetExpansion { text, cursor_byte })
}

/// Apply one insertion at each UTF-8 byte anchor. Anchors are interpreted against the original
/// source and applied right-to-left, so earlier insertions cannot shift later anchors.
pub fn insert_at_anchors(
    source: &str,
    anchors: &[usize],
    insertion: &str,
) -> Result<String, String> {
    let mut ordered = anchors.to_vec();
    ordered.sort_unstable();
    ordered.dedup();
    if ordered
        .iter()
        .any(|offset| !source.is_char_boundary(*offset))
    {
        return Err("multi-cursor anchor is not a UTF-8 boundary".into());
    }
    let mut result = source.to_owned();
    for offset in ordered.into_iter().rev() {
        result.insert_str(offset, insertion);
    }
    Ok(result)
}

/// Replace several original-buffer selections in one edit. Ranges use UTF-8 byte offsets but
/// must start and end at extended grapheme boundaries. Overlapping or repeated ranges are rejected.
pub fn replace_at_grapheme_ranges(
    source: &str,
    ranges: &[std::ops::Range<usize>],
    replacement: &str,
) -> Result<String, String> {
    if ranges.is_empty() {
        return Err("multi-selection replacement needs at least one range".into());
    }
    let mut boundaries = source
        .grapheme_indices(true)
        .map(|(offset, _)| offset)
        .collect::<BTreeSet<_>>();
    boundaries.insert(source.len());
    let mut ordered = ranges.to_vec();
    ordered.sort_by_key(|range| (range.start, range.end));
    if ordered.iter().any(|range| {
        range.start > range.end
            || range.end > source.len()
            || !boundaries.contains(&range.start)
            || !boundaries.contains(&range.end)
    }) {
        return Err("multi-selection range is outside the buffer or splits a grapheme".into());
    }
    if ordered
        .windows(2)
        .any(|pair| pair[1].start < pair[0].end || pair[1].start == pair[0].start)
    {
        return Err("multi-selection ranges overlap or repeat an insertion point".into());
    }
    let mut result = source.to_owned();
    for range in ordered.into_iter().rev() {
        result.replace_range(range, replacement);
    }
    Ok(result)
}

/// A small replayable edit macro. Operations are intentionally data-only and can be recorded,
/// persisted or dispatched by any frontend without depending on a terminal UI.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EditMacroOp {
    Insert(String),
    Backspace,
    DeleteForward,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveLineStart,
    MoveLineEnd,
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    SelectLineStart,
    SelectLineEnd,
    Replace { from: String, to: String },
    UppercaseSelection,
    LowercaseSelection,
}

pub fn run_edit_macro(
    source: &str,
    selection: std::ops::Range<usize>,
    operations: &[EditMacroOp],
) -> Result<(String, std::ops::Range<usize>), String> {
    if selection.start > selection.end
        || selection.end > source.len()
        || !source.is_char_boundary(selection.start)
        || !source.is_char_boundary(selection.end)
    {
        return Err("invalid UTF-8 selection range".into());
    }
    let mut before = source[..selection.start].to_owned();
    let mut selected = source[selection.clone()].to_owned();
    let after = source[selection.end..].to_owned();
    let mut cursor = selected.len();
    let mut anchor: Option<usize> = None;
    for operation in operations {
        match operation {
            EditMacroOp::Insert(text) => {
                if let Some(selection_anchor) = anchor.take() {
                    let start = selection_anchor.min(cursor);
                    let end = selection_anchor.max(cursor);
                    selected.replace_range(start..end, "");
                    cursor = start;
                }
                selected.insert_str(cursor, text);
                cursor += text.len();
            }
            EditMacroOp::Backspace => {
                if let Some(selection_anchor) = anchor.take() {
                    let start = selection_anchor.min(cursor);
                    let end = selection_anchor.max(cursor);
                    selected.replace_range(start..end, "");
                    cursor = start;
                } else if let Some(previous) = selected[..cursor]
                    .grapheme_indices(true)
                    .next_back()
                    .map(|(i, _)| i)
                {
                    selected.replace_range(previous..cursor, "");
                    cursor = previous;
                }
            }
            EditMacroOp::DeleteForward => {
                if let Some(selection_anchor) = anchor.take() {
                    let start = selection_anchor.min(cursor);
                    let end = selection_anchor.max(cursor);
                    selected.replace_range(start..end, "");
                    cursor = start;
                } else if let Some((offset, grapheme)) =
                    selected[cursor..].grapheme_indices(true).next()
                {
                    let start = cursor + offset;
                    selected.replace_range(start..start + grapheme.len(), "");
                }
            }
            EditMacroOp::MoveLeft | EditMacroOp::SelectLeft => {
                if matches!(operation, EditMacroOp::SelectLeft) {
                    anchor.get_or_insert(cursor);
                } else {
                    anchor = None;
                }
                cursor = selected[..cursor]
                    .grapheme_indices(true)
                    .next_back()
                    .map_or(0, |(i, _)| i);
            }
            EditMacroOp::MoveRight | EditMacroOp::SelectRight => {
                if matches!(operation, EditMacroOp::SelectRight) {
                    anchor.get_or_insert(cursor);
                } else {
                    anchor = None;
                }
                if let Some((offset, grapheme)) = selected[cursor..].grapheme_indices(true).next() {
                    cursor += offset + grapheme.len();
                }
            }
            EditMacroOp::MoveUp
            | EditMacroOp::MoveDown
            | EditMacroOp::SelectUp
            | EditMacroOp::SelectDown => {
                if matches!(operation, EditMacroOp::SelectUp | EditMacroOp::SelectDown) {
                    anchor.get_or_insert(cursor);
                } else {
                    anchor = None;
                }
                let prior_lines = selected[..cursor].matches('\n').count();
                let lines = selected.split('\n').collect::<Vec<_>>();
                let current_line_start = selected[..cursor].rfind('\n').map_or(0, |i| i + 1);
                let column = selected[current_line_start..cursor].graphemes(true).count();
                let target_line =
                    if matches!(operation, EditMacroOp::MoveUp | EditMacroOp::SelectUp) {
                        prior_lines.saturating_sub(1)
                    } else {
                        (prior_lines + 1).min(lines.len().saturating_sub(1))
                    };
                let line_start = lines
                    .iter()
                    .take(target_line)
                    .map(|line| line.len() + 1)
                    .sum::<usize>();
                let target = lines[target_line];
                let target_column = target
                    .grapheme_indices(true)
                    .nth(column)
                    .map_or(target.len(), |(offset, _)| offset);
                cursor = line_start + target_column;
            }
            EditMacroOp::MoveLineStart | EditMacroOp::SelectLineStart => {
                if matches!(operation, EditMacroOp::SelectLineStart) {
                    anchor.get_or_insert(cursor);
                } else {
                    anchor = None;
                }
                cursor = selected[..cursor].rfind('\n').map_or(0, |i| i + 1);
            }
            EditMacroOp::MoveLineEnd | EditMacroOp::SelectLineEnd => {
                if matches!(operation, EditMacroOp::SelectLineEnd) {
                    anchor.get_or_insert(cursor);
                } else {
                    anchor = None;
                }
                cursor += selected[cursor..]
                    .find('\n')
                    .unwrap_or(selected.len() - cursor);
            }
            EditMacroOp::Replace { from, to } => {
                anchor = None;
                selected = selected.replace(from, to);
                cursor = selected.len();
            }
            EditMacroOp::UppercaseSelection => {
                if let Some(selection_anchor) = anchor.take() {
                    let start = selection_anchor.min(cursor);
                    let end = selection_anchor.max(cursor);
                    let replacement = selected[start..end].to_uppercase();
                    selected.replace_range(start..end, &replacement);
                    cursor = start + replacement.len();
                } else {
                    selected = selected.to_uppercase();
                    cursor = selected.len();
                }
            }
            EditMacroOp::LowercaseSelection => {
                if let Some(selection_anchor) = anchor.take() {
                    let start = selection_anchor.min(cursor);
                    let end = selection_anchor.max(cursor);
                    let replacement = selected[start..end].to_lowercase();
                    selected.replace_range(start..end, &replacement);
                    cursor = start + replacement.len();
                } else {
                    selected = selected.to_lowercase();
                    cursor = selected.len();
                }
            }
        }
    }
    let start = before.len();
    before.push_str(&selected);
    before.push_str(&after);
    Ok((before, start..start + selected.len()))
}

#[cfg(test)]
mod editor_tool_tests {
    use super::*;
    #[test]
    fn snippets_support_unicode_fields_and_cursor_markers() {
        let values = [("name".to_owned(), "Élodie".to_owned())]
            .into_iter()
            .collect();
        let expanded = expand_snippet("Hello ${name}.${cursor}", &values).unwrap();
        assert_eq!(expanded.text, "Hello Élodie.");
        assert_eq!(&expanded.text[..expanded.cursor_byte], "Hello Élodie.");
        assert!(expand_snippet("${missing}", &BTreeMap::new()).is_err());
    }
    #[test]
    fn multi_cursor_insert_applies_right_to_left_at_utf8_boundaries() {
        assert_eq!(insert_at_anchors("é ab", &[3, 0], "X").unwrap(), "Xé Xab");
        assert!(insert_at_anchors("é", &[1], "x").is_err());
    }

    #[test]
    fn multi_selection_replace_is_grapheme_safe_and_rejects_ambiguous_ranges() {
        let source = "É🙂e\u{301}x";
        let ranges = [0.."É".len(), "É🙂".len()..source.len()];
        assert_eq!(
            replace_at_grapheme_ranges(source, &ranges, "word").unwrap(),
            "word🙂word"
        );
        assert!(replace_at_grapheme_ranges(source, std::slice::from_ref(&(1..2)), "x").is_err());
        assert!(
            replace_at_grapheme_ranges(
                source,
                std::slice::from_ref(&("É🙂".len().."É🙂e".len())),
                "x"
            )
            .is_err()
        );
        assert!(replace_at_grapheme_ranges("abcd", &[0..3, 2..4], "x").is_err());
        assert!(replace_at_grapheme_ranges(source, &[0..0, 0..0], "x").is_err());
        assert!(replace_at_grapheme_ranges("abcd", &[0..0, 0..2], "x").is_err());
    }
    #[test]
    fn edit_macros_transform_selection_without_touching_surrounding_text() {
        let (text, range) = run_edit_macro(
            "Say hello!",
            4..9,
            &[
                EditMacroOp::UppercaseSelection,
                EditMacroOp::Insert(" friend".into()),
            ],
        )
        .unwrap();
        assert_eq!(text, "Say HELLO friend!");
        assert_eq!(&text[range], "HELLO friend");
    }

    #[test]
    fn recorded_cursor_operations_replay_grapheme_safe_edits() {
        let operations = [
            EditMacroOp::MoveLineEnd,
            EditMacroOp::Insert("!".into()),
            EditMacroOp::MoveLeft,
            EditMacroOp::Backspace,
        ];
        let (text, range) = run_edit_macro("aé", 0..3, &operations).unwrap();
        assert_eq!(text, "a!");
        assert_eq!(&text[range], "a!");
    }

    #[test]
    fn recorded_selection_operations_replace_graphemes_without_touching_neighbors() {
        let operations = [EditMacroOp::SelectLeft, EditMacroOp::Insert("X".into())];
        let (text, range) = run_edit_macro("aéz", 0..4, &operations).unwrap();
        assert_eq!(text, "aéX");
        assert_eq!(&text[range], "aéX");
    }

    #[test]
    fn recorded_vertical_navigation_preserves_grapheme_column() {
        let operations = [EditMacroOp::MoveUp, EditMacroOp::Insert("!".into())];
        let (text, _) = run_edit_macro("abcd\nx", 0..6, &operations).unwrap();
        assert_eq!(text, "a!bcd\nx");
    }
}
use uuid::Uuid;

/// Project-wide policy for mapping multilingual prose to writing-budget units.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WordCountPolicy {
    /// Count Unicode word-boundary segments (UAX #29), irrespective of locale.
    #[default]
    UnicodeWords,
    /// Count graphemes in selected unspaced scripts individually; count other scripts by Unicode words.
    /// This is a writing-budget convention, not linguistic tokenization.
    UnspacedScriptGraphemes,
}

impl WordCountPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnicodeWords => "unicode-words",
            Self::UnspacedScriptGraphemes => "script-graphemes",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "unicode-words" => Some(Self::UnicodeWords),
            "script-graphemes" => Some(Self::UnspacedScriptGraphemes),
            _ => None,
        }
    }
}

/// Counts writing-budget units using the selected, deterministic project policy.
pub fn count_words_with_policy(text: &str, policy: WordCountPolicy) -> u32 {
    let count = match policy {
        WordCountPolicy::UnicodeWords => text.unicode_words().count(),
        WordCountPolicy::UnspacedScriptGraphemes => {
            let mut count = 0usize;
            let mut other_script_run = String::new();
            for grapheme in text.graphemes(true) {
                if is_unspaced_script_grapheme(grapheme) {
                    count += other_script_run.unicode_words().count();
                    other_script_run.clear();
                    count += 1;
                } else {
                    other_script_run.push_str(grapheme);
                }
            }
            count + other_script_run.unicode_words().count()
        }
    };
    count.min(u32::MAX as usize) as u32
}

/// Counts Unicode word-boundary segments, the backward-compatible default policy.
pub fn count_words(text: &str) -> u32 {
    count_words_with_policy(text, WordCountPolicy::UnicodeWords)
}

/// Returns whether a grapheme belongs to one of the script ranges counted individually.
pub fn is_unspaced_script_grapheme(grapheme: &str) -> bool {
    grapheme.chars().any(is_unspaced_script_character)
}

fn is_unspaced_script_character(character: char) -> bool {
    character.is_alphanumeric()
        && matches!(
            character as u32,
            0x1100..=0x11ff
                | 0x2e80..=0x2eff
                | 0x3041..=0x3096
                | 0x309d..=0x309f
                | 0x30a1..=0x30fa
                | 0x30fc..=0x30ff
                | 0x3100..=0x312f
                | 0x3130..=0x318f
                | 0x31a0..=0x31bf
                | 0x31f0..=0x31ff
                | 0x3400..=0x4dbf
                | 0x4e00..=0x9fff
                | 0xac00..=0xd7af
                | 0xf900..=0xfaff
                | 0x0e00..=0x0e7f
                | 0x0e80..=0x0eff
                | 0x1000..=0x109f
                | 0x1780..=0x17ff
                | 0x20000..=0x2fa1f
        )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindMode {
    Normal,
    Extended,
    Wildcard,
    Regex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FindOptions {
    pub mode: FindMode,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self {
            mode: FindMode::Normal,
            case_sensitive: true,
            whole_word: false,
        }
    }
}

pub fn find_matches(
    text: &str,
    pattern: &str,
    mode: FindMode,
) -> Result<Vec<std::ops::Range<usize>>, String> {
    find_matches_with_options(
        text,
        pattern,
        FindOptions {
            mode,
            ..FindOptions::default()
        },
    )
}

pub fn find_matches_with_options(
    text: &str,
    pattern: &str,
    options: FindOptions,
) -> Result<Vec<std::ops::Range<usize>>, String> {
    if pattern.is_empty() {
        return Err("search pattern cannot be empty".into());
    }
    let mut source = match options.mode {
        FindMode::Normal => regex::escape(pattern),
        FindMode::Extended => regex::escape(&decode_extended(pattern)?),
        FindMode::Wildcard => wildcard_expression(pattern),
        FindMode::Regex => pattern.to_owned(),
    };
    if options.whole_word {
        source = format!(r"\b(?:{source})\b");
    }
    let re = regex::RegexBuilder::new(&source)
        .case_insensitive(!options.case_sensitive)
        .multi_line(true)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))?;
    Ok(re.find_iter(text).map(|m| m.start()..m.end()).collect())
}

/// Finds text with Unicode normalization and locale-aware case handling while
/// returning byte ranges into the original, unchanged UTF-8 source.
///
/// Turkish and Azerbaijani use the dotted/dotless-I mappings defined for their
/// language. Other language tags use Rust's Unicode lowercase mapping. When
/// `accent_sensitive` is false, canonical combining marks are ignored. Regex
/// mode remains the responsibility of [`find_matches_with_options`] because
/// transforming a regular expression would change its syntax.
pub fn find_matches_with_locale(
    text: &str,
    pattern: &str,
    options: FindOptions,
    language_tag: &str,
    accent_sensitive: bool,
) -> Result<Vec<std::ops::Range<usize>>, String> {
    use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

    if options.case_sensitive || options.mode == FindMode::Regex {
        return find_matches_with_options(text, pattern, options);
    }
    if pattern.is_empty() {
        return Err("search pattern cannot be empty".into());
    }

    fn turkic_language(tag: &str) -> bool {
        matches!(
            tag.split('-')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "tr" | "az"
        )
    }

    fn fold_cluster(cluster: &str, turkic: bool, accent_sensitive: bool) -> String {
        let mut mapped = String::new();
        for character in cluster.chars() {
            match (turkic, character) {
                (true, 'I') => mapped.push('ı'),
                (true, 'İ') => mapped.push('i'),
                _ => mapped.extend(character.to_lowercase()),
            }
        }
        if accent_sensitive {
            mapped.nfc().collect()
        } else {
            mapped
                .nfd()
                .filter(|character| !is_combining_mark(*character))
                .collect()
        }
    }

    fn fold_source(
        text: &str,
        turkic: bool,
        accent_sensitive: bool,
    ) -> (String, Vec<std::ops::Range<usize>>) {
        use unicode_segmentation::UnicodeSegmentation;
        let mut folded = String::new();
        let mut map = Vec::new();
        for (start, cluster) in text.grapheme_indices(true) {
            let end = start + cluster.len();
            for character in fold_cluster(cluster, turkic, accent_sensitive).chars() {
                folded.push(character);
                map.push(start..end);
            }
        }
        (folded, map)
    }

    let turkic = turkic_language(language_tag);
    let folded_pattern = fold_cluster(pattern, turkic, accent_sensitive);
    let mut expression = match options.mode {
        FindMode::Normal => regex::escape(&folded_pattern),
        FindMode::Extended => regex::escape(&fold_cluster(
            &decode_extended(pattern)?,
            turkic,
            accent_sensitive,
        )),
        FindMode::Wildcard => wildcard_expression(&folded_pattern),
        FindMode::Regex => unreachable!(),
    };
    if options.whole_word {
        expression = format!(r"\b(?:{expression})\b");
    }
    let regex = regex::RegexBuilder::new(&expression)
        .case_insensitive(false)
        .multi_line(true)
        .build()
        .map_err(|error| format!("invalid regular expression: {error}"))?;
    let (folded_text, source_ranges) = fold_source(text, turkic, accent_sensitive);
    Ok(regex
        .find_iter(&folded_text)
        .filter_map(|matched| {
            let first = source_ranges.get(folded_text[..matched.start()].chars().count())?;
            let last = source_ranges.get(
                folded_text[..matched.end()]
                    .chars()
                    .count()
                    .saturating_sub(1),
            )?;
            Some(first.start..last.end)
        })
        .collect())
}

/// Replaces the selected locale-aware literal matches while preserving every
/// unselected grapheme exactly as it appeared in the source text.
pub fn replace_selected_matches_with_locale(
    text: &str,
    pattern: &str,
    replacement: &str,
    options: FindOptions,
    selected: &[bool],
    language_tag: &str,
    accent_sensitive: bool,
) -> Result<(String, usize), String> {
    if options.case_sensitive || options.mode == FindMode::Regex {
        return replace_selected_matches_with_options(
            text,
            pattern,
            replacement,
            options,
            selected,
        );
    }
    let matches = find_matches_with_locale(text, pattern, options, language_tag, accent_sensitive)?;
    if matches.len() != selected.len() {
        return Err(
            "selection must contain one choice for every match in the unchanged source".into(),
        );
    }
    let mut output = String::new();
    let mut cursor = 0;
    let mut count = 0;
    for (matched, replace) in matches.iter().zip(selected) {
        output.push_str(&text[cursor..matched.start]);
        if *replace {
            output.push_str(replacement);
            count += 1;
        } else {
            output.push_str(&text[matched.clone()]);
        }
        cursor = matched.end;
    }
    output.push_str(&text[cursor..]);
    Ok((output, count))
}

pub fn replace_matches(
    text: &str,
    pattern: &str,
    replacement: &str,
    mode: FindMode,
) -> Result<(String, usize), String> {
    replace_matches_with_options(
        text,
        pattern,
        replacement,
        FindOptions {
            mode,
            ..FindOptions::default()
        },
    )
}

pub fn replace_matches_with_options(
    text: &str,
    pattern: &str,
    replacement: &str,
    options: FindOptions,
) -> Result<(String, usize), String> {
    let regex = build_find_regex(pattern, options)?;
    let selected = vec![true; regex.find_iter(text).count()];
    replace_selected_matches_with_options(text, pattern, replacement, options, &selected)
}

/// Replace only the indexed matches from a stable preview. `selected` must
/// contain one choice for every match in the unchanged source text.
pub fn replace_selected_matches_with_options(
    text: &str,
    pattern: &str,
    replacement: &str,
    options: FindOptions,
    selected: &[bool],
) -> Result<(String, usize), String> {
    if pattern.is_empty() {
        return Err("search pattern cannot be empty".into());
    }
    let regex = build_find_regex(pattern, options)?;
    let captures = regex.captures_iter(text).collect::<Vec<_>>();
    if captures.len() != selected.len() {
        return Err("replacement selection no longer matches the preview source".into());
    }
    let replacement = match options.mode {
        FindMode::Normal => replacement.to_owned(),
        FindMode::Extended => decode_extended(replacement)?,
        FindMode::Wildcard => replacement.to_owned(),
        FindMode::Regex => replacement.to_owned(),
    };
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    let mut count = 0;
    for (captures, selected) in captures.iter().zip(selected) {
        let matched = captures
            .get(0)
            .ok_or("regular expression returned an empty capture set")?;
        output.push_str(&text[cursor..matched.start()]);
        if *selected {
            count += 1;
            if options.mode == FindMode::Regex {
                captures.expand(&replacement, &mut output);
            } else {
                output.push_str(&replacement);
            }
        } else {
            output.push_str(matched.as_str());
        }
        cursor = matched.end();
    }
    output.push_str(&text[cursor..]);
    Ok((output, count))
}

fn build_find_regex(pattern: &str, options: FindOptions) -> Result<regex::Regex, String> {
    if pattern.is_empty() {
        return Err("search pattern cannot be empty".into());
    }
    let mut source = match options.mode {
        FindMode::Normal => regex::escape(pattern),
        FindMode::Extended => regex::escape(&decode_extended(pattern)?),
        FindMode::Wildcard => wildcard_expression(pattern),
        FindMode::Regex => pattern.to_owned(),
    };
    if options.whole_word {
        source = format!(r"\b(?:{source})\b");
    }
    regex::RegexBuilder::new(&source)
        .case_insensitive(!options.case_sensitive)
        .multi_line(true)
        .build()
        .map_err(|error| format!("invalid regular expression: {error}"))
}

/// Converts a deliberately small, author-facing wildcard syntax to a regex.
/// `*` matches zero or more Unicode scalar values, `?` matches one, and all
/// other characters are literal. Wildcards are distinct from regex syntax.
fn wildcard_expression(pattern: &str) -> String {
    let mut expression = String::new();
    for character in pattern.chars() {
        match character {
            '*' => expression.push_str(".*"),
            '?' => expression.push('.'),
            other => expression.push_str(&regex::escape(&other.to_string())),
        }
    }
    expression
}

fn decode_extended(text: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            result.push(
                match chars.next().ok_or("trailing escape in extended search")? {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\\' => '\\',
                    other => return Err(format!("unsupported escape \\{other}")),
                },
            );
        } else {
            result.push(ch);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod multilingual_tests {
    use super::count_words;

    #[test]
    fn word_count_uses_unicode_boundaries_for_multilingual_text() {
        assert_eq!(count_words("café déjà-vu l’esprit"), 4);
        assert_eq!(count_words("你好 世界"), 4);
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn script_grapheme_policy_is_explicit_and_mixed_script_deterministic() {
        use super::{WordCountPolicy, count_words_with_policy};
        assert_eq!(
            count_words_with_policy("私は猫です", WordCountPolicy::UnspacedScriptGraphemes),
            5
        );
        assert!(count_words("한국어") < 3);
        assert_eq!(
            count_words_with_policy("Hello世界 again", WordCountPolicy::UnspacedScriptGraphemes),
            4
        );
        assert_eq!(
            count_words_with_policy("café déjà-vu", WordCountPolicy::UnspacedScriptGraphemes),
            3
        );
        assert_eq!(
            count_words_with_policy("我👩‍🚀爱你", WordCountPolicy::UnspacedScriptGraphemes),
            3
        );
        assert_eq!(
            count_words_with_policy("「私。」", WordCountPolicy::UnspacedScriptGraphemes),
            1
        );
        assert_eq!(count_words("ภาษาไทย"), 7);
        assert_eq!(
            count_words_with_policy("ภาษาไทย", WordCountPolicy::UnspacedScriptGraphemes),
            7
        );
    }

    #[test]
    fn multilingual_budget_policies_cover_issue_four_languages_and_hangul_jamo() {
        use super::{WordCountPolicy, count_words_with_policy};
        let unicode = WordCountPolicy::UnicodeWords;
        let script = WordCountPolicy::UnspacedScriptGraphemes;
        for sentence in ["L'autrice è qui.", "L’autrice est là."] {
            assert_eq!(count_words_with_policy(sentence, unicode), 3);
        }
        assert_eq!(count_words_with_policy("한국어", unicode), 1);
        assert_eq!(count_words_with_policy("한국어", script), 3);
        // Decomposed Hangul jamo are counted by extended grapheme cluster,
        // keeping a composed syllable and its jamo representation consistent.
        assert_eq!(count_words_with_policy("한글", script), 2);
        assert_eq!(count_words_with_policy("cafe\u{301}", unicode), 1);
    }

    #[test]
    fn locale_aware_find_maps_turkish_and_accent_folds_to_original_byte_ranges() {
        use super::{
            FindMode, FindOptions, find_matches_with_locale, replace_selected_matches_with_locale,
        };
        let options = FindOptions {
            mode: FindMode::Normal,
            case_sensitive: false,
            whole_word: false,
        };

        let turkish = "Işık İSTANBUL ıslak iz";
        let matches = find_matches_with_locale(turkish, "ışık", options, "tr-TR", true).unwrap();
        assert_eq!(&turkish[matches[0].clone()], "Işık");
        let matches = find_matches_with_locale(turkish, "istanbul", options, "tr", true).unwrap();
        assert_eq!(&turkish[matches[0].clone()], "İSTANBUL");

        let source = "Cafe\u{301} CAFÉ cafe";
        let insensitive =
            find_matches_with_locale(source, "café", options, "fr-FR", false).unwrap();
        assert_eq!(insensitive.len(), 3);
        assert_eq!(&source[insensitive[0].clone()], "Cafe\u{301}");
        assert_eq!(&source[insensitive[1].clone()], "CAFÉ");
        let sensitive = find_matches_with_locale(source, "café", options, "fr-FR", true).unwrap();
        assert_eq!(sensitive.len(), 2);
        assert_eq!(&source[sensitive[0].clone()], "Cafe\u{301}");
        assert_eq!(&source[sensitive[1].clone()], "CAFÉ");
        assert_eq!(
            replace_selected_matches_with_locale(
                source,
                "café",
                "tea",
                options,
                &[true, false, true],
                "fr-FR",
                false,
            )
            .unwrap(),
            ("tea CAFÉ tea".into(), 2)
        );
        assert_eq!(
            replace_selected_matches_with_locale(
                "cat CAT",
                r"(cat)",
                "<$1>",
                FindOptions {
                    mode: FindMode::Regex,
                    case_sensitive: false,
                    whole_word: false,
                },
                &[true, true],
                "tr-TR",
                false,
            )
            .unwrap(),
            ("<cat> <CAT>".into(), 2)
        );
    }

    #[test]
    fn find_replace_support_normal_extended_and_regex_modes() {
        use super::{FindMode, find_matches, replace_matches};
        let source = "one\ntwo 22";
        assert_eq!(
            find_matches(source, "two", FindMode::Normal).unwrap().len(),
            1
        );
        assert_eq!(
            find_matches(source, r"\ntwo", FindMode::Extended)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            replace_matches(source, r"(\w+) (\d+)", "$2 $1", FindMode::Regex).unwrap(),
            ("one\n22 two".to_owned(), 1)
        );
        assert!(find_matches(source, "[", FindMode::Regex).is_err());
    }

    #[test]
    fn wildcard_search_is_explicit_literal_safe_and_supports_unicode_and_replacement() {
        use super::{FindMode, find_matches, replace_matches};
        let text = "cat cot cut cöt catapult\nfox";
        assert_eq!(
            find_matches(text, "c?t", FindMode::Wildcard).unwrap(),
            [0..3, 4..7, 8..11, 12..16, 17..20]
        );
        assert_eq!(
            find_matches(text, "c*t", FindMode::Wildcard).unwrap(),
            std::iter::once(0..25).collect::<Vec<_>>()
        );
        assert_eq!(
            find_matches(text, "c.[t", FindMode::Wildcard).unwrap(),
            Vec::<std::ops::Range<usize>>::new()
        );
        assert_eq!(
            replace_matches(text, "c?t", "dog", FindMode::Wildcard).unwrap(),
            ("dog dog dog dog dogapult\nfox".into(), 5)
        );
    }

    #[test]
    fn find_options_support_case_insensitive_and_unicode_word_boundaries() {
        use super::{
            FindMode, FindOptions, find_matches_with_options, replace_matches_with_options,
        };
        let options = FindOptions {
            mode: FindMode::Normal,
            case_sensitive: false,
            whole_word: true,
        };
        let source = "CAT catalog cat café";
        let found = find_matches_with_options(source, "cat", options).unwrap();
        assert_eq!(
            found
                .iter()
                .map(|range| &source[range.clone()])
                .collect::<Vec<_>>(),
            ["CAT", "cat"]
        );
        assert_eq!(
            replace_matches_with_options(source, "cat", "fox", options).unwrap(),
            ("fox catalog fox café".to_owned(), 2)
        );
    }

    #[test]
    fn selected_regex_replacements_keep_capture_groups_and_skip_unselected_matches() {
        use super::{
            FindMode, FindOptions, replace_matches_with_options,
            replace_selected_matches_with_options,
        };
        let options = FindOptions {
            mode: FindMode::Regex,
            case_sensitive: true,
            whole_word: false,
        };
        assert_eq!(
            replace_selected_matches_with_options(
                "CH12 and CH34",
                r"CH([0-9]+)",
                "Ch$1",
                options,
                &[true, false],
            )
            .unwrap(),
            ("Ch12 and CH34".into(), 1)
        );
        assert_eq!(
            replace_matches_with_options("A $1 cat", "cat", "$1", FindOptions::default()).unwrap(),
            ("A $1 $1".into(), 1)
        );
        assert!(
            replace_selected_matches_with_options(
                "cat cat",
                "cat",
                "fox",
                FindOptions::default(),
                &[true],
            )
            .is_err()
        );
        assert_eq!(
            replace_matches_with_options("CH12\nCH34\nxCH56", r"^CH([0-9]+)$", "Ch$1", options,)
                .unwrap(),
            ("Ch12\nCh34\nxCH56".into(), 2)
        );
    }

    #[test]
    fn regex_zero_width_matches_replace_at_unicode_boundaries_safely() {
        use super::{
            FindMode, FindOptions, find_matches_with_options, replace_matches_with_options,
            replace_selected_matches_with_options,
        };
        let options = FindOptions {
            mode: FindMode::Regex,
            case_sensitive: true,
            whole_word: false,
        };
        let source = "café 한국";
        let matches = find_matches_with_options(source, r"\b", options).unwrap();
        assert_eq!(matches.len(), 4);
        assert!(matches.iter().all(
            |range| source.is_char_boundary(range.start) && source.is_char_boundary(range.end)
        ));
        assert_eq!(
            replace_matches_with_options(source, r"\b", "|", options).unwrap(),
            ("|café| |한국|".into(), 4)
        );
        assert_eq!(
            replace_selected_matches_with_options(
                source,
                r"\b",
                "|",
                options,
                &[false, true, false, false]
            )
            .unwrap(),
            ("café| 한국".into(), 1)
        );
    }

    #[test]
    fn regex_named_capture_replacement_uses_standard_rust_replacement_syntax() {
        use super::{FindMode, FindOptions, replace_matches_with_options};
        assert_eq!(
            replace_matches_with_options(
                "CH42",
                r"(?P<prefix>CH)(?P<number>[0-9]+)",
                "${prefix}apter $number",
                FindOptions {
                    mode: FindMode::Regex,
                    case_sensitive: true,
                    whole_word: false,
                },
            )
            .unwrap(),
            ("CHapter 42".into(), 1)
        );
    }
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);
        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }
        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
id_type!(ProjectId);
id_type!(NodeId);
id_type!(SceneId);
id_type!(CharacterId);
id_type!(PlotlineId);
id_type!(EventId);
id_type!(LocationId);
id_type!(FieldId);
id_type!(ImportRunId);
id_type!(SourceItemId);
id_type!(ProvenanceId);
id_type!(ResearchId);
id_type!(LinkId);
id_type!(AnchorId);
id_type!(QueryId);
id_type!(IssueId);
id_type!(ReadingId);
id_type!(AnnotationId);
id_type!(BookmarkId);
id_type!(ReadingTermId);
id_type!(SnapshotId);
id_type!(KnowledgeClaimId);
id_type!(ContentUnitId);
id_type!(StoryObjectId);
id_type!(MentionId);
id_type!(ScratchpadId);
id_type!(PersonalCommentId);
id_type!(AssemblyId);
id_type!(AssemblyItemId);
id_type!(PlanningBoardId);
id_type!(PlanningBoardItemId);
id_type!(ContinuityRuleId);
id_type!(PlanningBoardConnectorId);
id_type!(AssemblyDraftId);
id_type!(StoryBibleTemplateId);
id_type!(SeriesId);
id_type!(BookId);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryBibleEntityKind {
    Character,
    Location,
    Object,
    Plotline,
    Event,
}
impl StoryBibleEntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Location => "location",
            Self::Object => "object",
            Self::Plotline => "plotline",
            Self::Event => "event",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryBibleFieldType {
    Text,
    LongText,
    Integer,
    Decimal,
    Boolean,
    DateTime,
    Enum,
    EntityRef,
    EntityRefs,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryBibleFieldDefinition {
    pub id: String,
    pub label: String,
    pub field_type: StoryBibleFieldType,
    pub required: bool,
    pub enum_values: Vec<String>,
    pub reference_kind: Option<StoryBibleEntityKind>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryBibleTemplate {
    pub id: StoryBibleTemplateId,
    pub name: String,
    pub entity_kind: StoryBibleEntityKind,
    pub fields: Vec<StoryBibleFieldDefinition>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum StoryBibleFieldValue {
    Text(String),
    LongText(String),
    Integer(i64),
    Decimal(String),
    Boolean(bool),
    DateTime(String),
    Enum(String),
    EntityRef(String),
    EntityRefs(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryBibleProfile {
    pub entity_kind: StoryBibleEntityKind,
    pub entity_id: String,
    pub template_id: StoryBibleTemplateId,
    pub values: BTreeMap<String, StoryBibleFieldValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssemblyDraftItem {
    pub scene_id: SceneId,
    pub expected_hash: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssemblyDraft {
    pub id: AssemblyDraftId,
    pub assembly_id: AssemblyId,
    pub project_id: ProjectId,
    pub created_at: String,
    pub items: Vec<AssemblyDraftItem>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MentionEntityKind {
    Character,
    Location,
    Object,
}
impl MentionEntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Location => "location",
            Self::Object => "object",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScratchpadNote {
    pub id: ScratchpadId,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub position: u32,
    pub state: String,
    pub destination_kind: Option<String>,
    pub destination_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRestoreSelection {
    pub current_start: usize,
    pub current_end: usize,
    pub snapshot_start: usize,
    pub snapshot_end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadingAnnotationDraft {
    pub kind: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub quote: String,
    pub body: String,
    pub prefix_context: String,
    pub suffix_context: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EntityRef {
    Scene(SceneId),
    Character(CharacterId),
    Plotline(PlotlineId),
    Research(ResearchId),
    Reading(ReadingId),
}
impl EntityRef {
    pub fn kind(self) -> &'static str {
        match self {
            Self::Scene(_) => "scene",
            Self::Character(_) => "character",
            Self::Plotline(_) => "plotline",
            Self::Research(_) => "research",
            Self::Reading(_) => "reading",
        }
    }
    pub fn id(self) -> String {
        match self {
            Self::Scene(id) => id.to_string(),
            Self::Character(id) => id.to_string(),
            Self::Plotline(id) => id.to_string(),
            Self::Research(id) => id.to_string(),
            Self::Reading(id) => id.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LinkKind {
    Related,
    Cites,
    Transcludes,
    Includes,
    Navigates,
}
impl LinkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Related => "related",
            Self::Cites => "cites",
            Self::Transcludes => "transcludes",
            Self::Includes => "includes",
            Self::Navigates => "navigates",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PassageAnchor {
    pub id: AnchorId,
    pub scene_id: SceneId,
    pub start_byte: u64,
    pub end_byte: u64,
    pub quote: String,
    pub prefix_context: String,
    pub suffix_context: String,
    pub source_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PassageLanguageTag {
    pub anchor_id: AnchorId,
    pub language_code: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryLink {
    pub id: LinkId,
    pub source: EntityRef,
    pub source_anchor: Option<AnchorId>,
    pub target: EntityRef,
    pub target_anchor: Option<AnchorId>,
    pub kind: LinkKind,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SceneQueryFilter {
    Status(SceneStatus),
    ExcludeStatus(SceneStatus),
    Character(CharacterId),
    AnyCharacter(Vec<CharacterId>),
    /// Character is an explicitly confirmed participant (mentions do not count).
    CharacterPresent(CharacterId),
    /// Character is explicitly listed as mentioned, which is distinct from presence.
    CharacterMentioned(CharacterId),
    /// Character absence was explicitly recorded by the author.
    CharacterAbsent(CharacterId),
    Plotline(PlotlineId),
    AnyPlotline(Vec<PlotlineId>),
    Location(LocationId),
    Event(EventId),
    Chapter(String),
    TextContains(String),
    /// Included scene membership in a named manuscript assembly.
    Assembly(AssemblyId),
    IssueStatus(String),
    CustomMetadataEquals {
        field_id: FieldId,
        value: FieldValue,
    },
    /// Matches a scene whose entity-reference-list field contains this target.
    CustomMetadataContainsEntity {
        field_id: FieldId,
        target_kind: String,
        target_id: String,
    },
    CustomMetadataExists {
        field_id: FieldId,
    },
    CustomMetadataTextContains {
        field_id: FieldId,
        text: String,
    },
    CustomMetadataNumberCompare {
        field_id: FieldId,
        comparison: MetadataComparison,
        value: FieldValue,
    },
    TimeBefore(i64),
    TimeAfter(i64),
    /// Matches only when the complete known start interval is before the cutoff.
    TimeBeforeDefinitely(i64),
    /// Matches when the known start interval permits a time before the cutoff.
    TimeBeforePossibly(i64),
    /// Matches only when the complete known start interval is after the cutoff.
    TimeAfterDefinitely(i64),
    /// Matches when the known start interval permits a time after the cutoff.
    TimeAfterPossibly(i64),
    UnresolvedIssue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataComparison {
    LessThan,
    LessThanOrEqual,
    Equal,
    NotEqual,
    GreaterThanOrEqual,
    GreaterThan,
}
impl MetadataComparison {
    pub fn sql_operator(self) -> &'static str {
        match self {
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::Equal => "=",
            Self::NotEqual => "<>",
            Self::GreaterThanOrEqual => ">=",
            Self::GreaterThan => ">",
        }
    }
}

/// A composable, saved-query predicate. Empty `All`/`Any` groups are invalid.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum QueryExpression {
    Filter(SceneQueryFilter),
    All(Vec<QueryExpression>),
    Any(Vec<QueryExpression>),
    Not(Box<QueryExpression>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SavedSceneQuery {
    pub id: QueryId,
    pub name: String,
    pub description: String,
    pub filters: Vec<SceneQueryFilter>,
}

/// A traceable story-model fact that can be inspected from a query match.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueryEvidence {
    pub source_kind: String,
    pub source_id: String,
    pub detail: String,
    /// UTF-8 byte range in the canonical manuscript file, when the fact comes
    /// from a specific passage rather than scene-level structured metadata.
    pub passage_bytes: Option<(u64, u64)>,
    /// UTF-8 byte range within a metadata field, when evidence comes from a
    /// scene title or synopsis rather than its canonical manuscript file.
    pub field_bytes: Option<(String, u64, u64)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum QueryAssertion {
    AtLeast(u64),
    AtMost(u64),
    Exactly(u64),
    NonEmpty,
    Empty,
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid story time: {0}")]
    InvalidStoryTime(String),
    #[error("invalid word allocation: {0}")]
    InvalidAllocation(String),
    #[error("invalid custom field value: {0}")]
    InvalidField(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TimePrecision {
    Exact,
    Approximate,
    Bounded,
    Unknown,
}
pub const STORY_TICKS_PER_DAY: i64 = 86_400;

/// Story epoch is project-defined; each tick is one second. Unknown is not tick zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryTime {
    pub nominal: Option<i64>,
    pub lower: Option<i64>,
    pub upper: Option<i64>,
    pub precision: TimePrecision,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntervalBoundary {
    Inclusive,
    Exclusive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryInterval {
    pub start: StoryTime,
    pub end: StoryTime,
    pub start_boundary: IntervalBoundary,
    pub end_boundary: IntervalBoundary,
}

impl StoryInterval {
    pub fn closed(start: StoryTime, end: StoryTime) -> Self {
        Self {
            start,
            end,
            start_boundary: IntervalBoundary::Inclusive,
            end_boundary: IntervalBoundary::Inclusive,
        }
    }

    /// Compare whole intervals only when their uncertainty bounds prove an
    /// ordering. A shared endpoint overlaps only when both intervals include it.
    pub fn compare(self, other: Self) -> TimeOrdering {
        let a_end = self.end.upper;
        let b_start = other.start.lower;
        let b_end = other.end.upper;
        let a_start = self.start.lower;
        match (a_end, b_start) {
            (Some(a), Some(b))
                if a < b
                    || (a == b
                        && (self.end_boundary == IntervalBoundary::Exclusive
                            || other.start_boundary == IntervalBoundary::Exclusive)) =>
            {
                return TimeOrdering::Before;
            }
            _ => {}
        }
        match (b_end, a_start) {
            (Some(b), Some(a))
                if b < a
                    || (b == a
                        && (other.end_boundary == IntervalBoundary::Exclusive
                            || self.start_boundary == IntervalBoundary::Exclusive)) =>
            {
                return TimeOrdering::After;
            }
            _ => {}
        }
        if self.start == other.start
            && self.end == other.end
            && self.start_boundary == other.start_boundary
            && self.end_boundary == other.end_boundary
        {
            TimeOrdering::Same
        } else {
            TimeOrdering::Ambiguous
        }
    }

    pub fn duration(self) -> Result<Option<i64>, DomainError> {
        self.start.duration_until(self.end)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeOrdering {
    Before,
    After,
    Same,
    Ambiguous,
}
impl StoryTime {
    pub fn unknown() -> Self {
        Self {
            nominal: None,
            lower: None,
            upper: None,
            precision: TimePrecision::Unknown,
        }
    }
    pub fn exact(tick: i64) -> Self {
        Self {
            nominal: Some(tick),
            lower: Some(tick),
            upper: Some(tick),
            precision: TimePrecision::Exact,
        }
    }
    pub fn new(
        nominal: Option<i64>,
        lower: Option<i64>,
        upper: Option<i64>,
        precision: TimePrecision,
    ) -> Result<Self, DomainError> {
        match precision {
            TimePrecision::Unknown if nominal.is_some() || lower.is_some() || upper.is_some() => {
                return Err(DomainError::InvalidStoryTime(
                    "unknown time must not have ticks".into(),
                ));
            }
            TimePrecision::Unknown => (),
            TimePrecision::Exact if nominal.is_none() => {
                return Err(DomainError::InvalidStoryTime(
                    "exact time requires nominal tick".into(),
                ));
            }
            TimePrecision::Approximate if nominal.is_none() => {
                return Err(DomainError::InvalidStoryTime(
                    "approximate time requires nominal tick".into(),
                ));
            }
            TimePrecision::Bounded if lower.is_none() || upper.is_none() => {
                return Err(DomainError::InvalidStoryTime(
                    "bounded time requires both bounds".into(),
                ));
            }
            _ => (),
        }
        if let (Some(lo), Some(hi)) = (lower, upper)
            && lo > hi
        {
            return Err(DomainError::InvalidStoryTime(
                "lower bound exceeds upper bound".into(),
            ));
        }
        if let Some(n) = nominal
            && (lower.is_some_and(|x| n < x) || upper.is_some_and(|x| n > x))
        {
            return Err(DomainError::InvalidStoryTime(
                "nominal is outside bounds".into(),
            ));
        }
        if precision == TimePrecision::Exact && (lower != nominal || upper != nominal) {
            return Err(DomainError::InvalidStoryTime(
                "exact time bounds must equal nominal".into(),
            ));
        }
        Ok(Self {
            nominal,
            lower,
            upper,
            precision,
        })
    }
    pub fn compare(self, other: Self) -> TimeOrdering {
        let (Some((al, ah)), Some((bl, bh))) = (self.interval(), other.interval()) else {
            return TimeOrdering::Ambiguous;
        };
        if ah < bl {
            TimeOrdering::Before
        } else if bh < al {
            TimeOrdering::After
        } else if al == ah && bl == bh && al == bl {
            TimeOrdering::Same
        } else {
            TimeOrdering::Ambiguous
        }
    }
    fn interval(self) -> Option<(i64, i64)> {
        match self.precision {
            TimePrecision::Exact => self.nominal.map(|n| (n, n)),
            TimePrecision::Bounded => Some((self.lower?, self.upper?)),
            TimePrecision::Approximate => Some((self.lower?, self.upper?)),
            TimePrecision::Unknown => None,
        }
    }
    pub fn duration_until(self, end: Self) -> Result<Option<i64>, DomainError> {
        match (self.nominal, end.nominal) {
            (Some(a), Some(b)) => b
                .checked_sub(a)
                .map(Some)
                .ok_or_else(|| DomainError::InvalidStoryTime("duration overflow".into())),
            _ => Ok(None),
        }
    }
    pub fn duration_bounds_until(self, end: Self) -> Result<Option<(i64, i64)>, DomainError> {
        let (Some((start_lo, start_hi)), Some((end_lo, end_hi))) =
            (self.interval(), end.interval())
        else {
            return Ok(None);
        };
        let min = end_lo
            .checked_sub(start_hi)
            .ok_or_else(|| DomainError::InvalidStoryTime("minimum duration overflow".into()))?;
        let max = end_hi
            .checked_sub(start_lo)
            .ok_or_else(|| DomainError::InvalidStoryTime("maximum duration overflow".into()))?;
        Ok(Some((min, max)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Section,
    Chapter,
    Group,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentUnitKind {
    FrontMatter,
    SectionBody,
    ChapterBody,
    BackMatter,
}

impl ContentUnitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FrontMatter => "front_matter",
            Self::SectionBody => "section_body",
            Self::ChapterBody => "chapter_body",
            Self::BackMatter => "back_matter",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "front_matter" => Some(Self::FrontMatter),
            "section_body" => Some(Self::SectionBody),
            "chapter_body" => Some(Self::ChapterBody),
            "back_matter" => Some(Self::BackMatter),
            _ => None,
        }
    }
}
impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Section => "section",
            Self::Chapter => "chapter",
            Self::Group => "group",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SceneStatus {
    Idea,
    Outlined,
    ReadyToDraft,
    Drafting,
    DraftComplete,
    Revising,
    Complete,
    /// The scene was imported with prose but the source did not provide a
    /// recognised authoring status. This avoids presenting existing writing
    /// as a new idea while making no claim that the draft is complete.
    Imported,
    Archived,
    /// Project-local author-defined status. The numeric identity is stable
    /// across label edits and ordering changes; its visible label is stored in
    /// the project's manuscript-status definitions.
    Custom(u16),
}
impl SceneStatus {
    pub fn as_str(self) -> String {
        match self {
            Self::Idea => "idea".into(),
            Self::Outlined => "outlined".into(),
            Self::ReadyToDraft => "ready_to_draft".into(),
            Self::Drafting => "drafting".into(),
            Self::DraftComplete => "draft_complete".into(),
            Self::Revising => "revising".into(),
            Self::Complete => "complete".into(),
            Self::Imported => "imported".into(),
            Self::Archived => "archived".into(),
            Self::Custom(code) => format!("custom_{code:05}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManuscriptStatusDefinition {
    pub status: SceneStatus,
    pub label: String,
    pub position: u32,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub author: String,
    pub genre: String,
    pub synopsis: String,
    #[serde(default)]
    pub book_type: BookType,
    #[serde(default)]
    pub template_id: Option<String>,
    pub target_words: Option<u64>,
    pub story_epoch_label: String,
    pub story_calendar_id: Option<String>,
    #[serde(default = "default_language_code")]
    pub language_code: String,
    #[serde(default)]
    pub word_count_policy: WordCountPolicy,
}

pub fn default_language_code() -> String {
    "en-GB".to_owned()
}

pub fn is_valid_language_tag(language_code: &str) -> bool {
    let parts = language_code.split('-').collect::<Vec<_>>();
    !parts.is_empty()
        && parts[0].len() >= 2
        && parts[0].len() <= 8
        && parts[0].bytes().all(|byte| byte.is_ascii_alphabetic())
        && parts.iter().skip(1).all(|part| {
            !part.is_empty()
                && part.len() <= 8
                && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinTemplate {
    BlankFiction,
    Novel,
    ShortStory,
    BlankNonfiction,
    NonfictionBook,
}

impl BuiltinTemplate {
    pub fn id(self) -> &'static str {
        match self {
            Self::BlankFiction => "blank-fiction",
            Self::Novel => "novel",
            Self::ShortStory => "short-story",
            Self::BlankNonfiction => "blank-nonfiction",
            Self::NonfictionBook => "nonfiction-book",
        }
    }
    pub fn book_type(self) -> BookType {
        match self {
            Self::BlankNonfiction | Self::NonfictionBook => BookType::Nonfiction,
            _ => BookType::Fiction,
        }
    }
    pub fn all() -> [Self; 5] {
        [
            Self::BlankFiction,
            Self::Novel,
            Self::ShortStory,
            Self::BlankNonfiction,
            Self::NonfictionBook,
        ]
    }
    pub fn parse(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|template| template.id() == id)
    }

    pub fn definition(self) -> ProjectTemplate {
        let mut template = ProjectTemplate {
            schema_version: 1,
            template_revision: 1,
            id: self.id().into(),
            name: match self {
                Self::BlankFiction => "Blank Fiction",
                Self::Novel => "Novel",
                Self::ShortStory => "Short Story",
                Self::BlankNonfiction => "Blank Non-fiction",
                Self::NonfictionBook => "Non-fiction Book",
            }
            .into(),
            book_type: self.book_type(),
            target_words: None,
            nodes: Vec::new(),
            content_units: Vec::new(),
            scenes: Vec::new(),
            custom_fields: Vec::new(),
        };
        match self {
            Self::BlankFiction | Self::BlankNonfiction => {}
            Self::Novel => template.nodes.push(ProjectTemplateNode {
                key: "chapter-1".into(),
                parent_key: None,
                kind: NodeKind::Chapter,
                title: "Chapter 1".into(),
                position: 0,
            }),
            Self::ShortStory => template.nodes.push(ProjectTemplateNode {
                key: "chapter-1".into(),
                parent_key: None,
                kind: NodeKind::Chapter,
                title: "Story".into(),
                position: 0,
            }),
            Self::NonfictionBook => {
                template.nodes.push(ProjectTemplateNode {
                    key: "chapter-1".into(),
                    parent_key: None,
                    kind: NodeKind::Chapter,
                    title: "Chapter 1".into(),
                    position: 0,
                });
                template.content_units.push(ProjectTemplateContentUnit {
                    key: "chapter-1-body".into(),
                    parent_key: Some("chapter-1".into()),
                    kind: ContentUnitKind::ChapterBody,
                    title: "Chapter 1 prose".into(),
                    position: 0,
                    target_words: None,
                });
            }
        }
        template
    }
}

/// A versioned, declarative project starter. It contains structure and empty
/// content-unit definitions only; manuscript prose and project secrets are
/// never copied into a new project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectTemplate {
    pub schema_version: u16,
    pub template_revision: u32,
    pub id: String,
    pub name: String,
    pub book_type: BookType,
    #[serde(default)]
    pub target_words: Option<u64>,
    #[serde(default)]
    pub nodes: Vec<ProjectTemplateNode>,
    #[serde(default)]
    pub content_units: Vec<ProjectTemplateContentUnit>,
    #[serde(default)]
    pub scenes: Vec<ProjectTemplateScene>,
    #[serde(default)]
    pub custom_fields: Vec<ProjectTemplateCustomField>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectTemplateNode {
    pub key: String,
    #[serde(default)]
    pub parent_key: Option<String>,
    pub kind: NodeKind,
    pub title: String,
    pub position: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectTemplateContentUnit {
    pub key: String,
    #[serde(default)]
    pub parent_key: Option<String>,
    pub kind: ContentUnitKind,
    pub title: String,
    pub position: u32,
    #[serde(default)]
    pub target_words: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectTemplateScene {
    pub key: String,
    pub chapter_key: String,
    pub title: String,
    pub position: u32,
    #[serde(default)]
    pub target_words: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectTemplateCustomField {
    pub key: String,
    pub name: String,
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default_value: Option<FieldValue>,
    #[serde(default)]
    pub enum_values: Vec<String>,
    #[serde(default)]
    pub reference_kind: Option<String>,
}

impl ProjectTemplate {
    pub fn validate(&self) -> Result<(), String> {
        const MAX_TEMPLATE_ITEMS: usize = 4096;
        let valid_key = |key: &str| {
            !key.is_empty()
                && key.len() <= 80
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        };
        if self.schema_version != 1 || self.template_revision == 0 {
            return Err("unsupported project-template schema or revision".into());
        }
        if !valid_key(&self.id) || self.name.trim().is_empty() {
            return Err("project-template ID and name are required".into());
        }
        let total = self.nodes.len()
            + self.content_units.len()
            + self.scenes.len()
            + self.custom_fields.len();
        if total > MAX_TEMPLATE_ITEMS {
            return Err("project template exceeds the 4096-item limit".into());
        }
        let mut keys = BTreeSet::new();
        for item in &self.nodes {
            if !valid_key(&item.key)
                || item.title.trim().is_empty()
                || !keys.insert(item.key.as_str())
            {
                return Err(
                    "project-template node keys and titles must be valid and unique".into(),
                );
            }
        }
        for item in &self.content_units {
            if !valid_key(&item.key)
                || item.title.trim().is_empty()
                || !keys.insert(item.key.as_str())
            {
                return Err(
                    "project-template content keys and titles must be valid and unique".into(),
                );
            }
        }
        for item in &self.scenes {
            if !valid_key(&item.key)
                || item.title.trim().is_empty()
                || !keys.insert(item.key.as_str())
            {
                return Err(
                    "project-template scene keys and titles must be valid and unique".into(),
                );
            }
        }
        for field in &self.custom_fields {
            if !valid_key(&field.key)
                || field.name.trim().is_empty()
                || !keys.insert(field.key.as_str())
            {
                return Err(
                    "project-template custom-field keys and names must be valid and unique".into(),
                );
            }
            if matches!(
                field.field_type,
                FieldType::EntityRef | FieldType::EntityRefs
            ) && !matches!(
                field.reference_kind.as_deref(),
                Some("character" | "location" | "plotline" | "event" | "scene")
            ) {
                return Err(format!(
                    "template field {} has an invalid entity-reference target",
                    field.key
                ));
            }
            if matches!(
                field.default_value,
                Some(FieldValue::EntityRef(_) | FieldValue::EntityRefs(_))
            ) {
                return Err(format!(
                    "template field {} cannot use project-specific entity IDs as defaults",
                    field.key
                ));
            }
            if field.field_type == FieldType::Enum
                && (field.enum_values.is_empty()
                    || field.enum_values.iter().any(|value| value.contains('\n')))
            {
                return Err(format!(
                    "template enum field {} needs non-empty single-line options",
                    field.key
                ));
            }
            if let Some(value) = field.default_value.as_ref() {
                let definition = CustomFieldDefinition {
                    id: FieldId::new(),
                    name: field.name.clone(),
                    field_type: field.field_type,
                    required: field.required,
                    default_value: None,
                    enum_values: field.enum_values.clone(),
                    reference_kind: field.reference_kind.clone(),
                };
                definition
                    .validate(value)
                    .map_err(|error| error.to_string())?;
            }
        }
        let nodes = self
            .nodes
            .iter()
            .map(|node| (node.key.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        for item in &self.nodes {
            if let Some(parent_key) = item.parent_key.as_deref() {
                let parent = nodes
                    .get(parent_key)
                    .ok_or_else(|| format!("template parent {parent_key} is missing"))?;
                if parent.kind == NodeKind::Chapter {
                    return Err("template chapters cannot contain structural children".into());
                }
            }
        }
        for item in &self.nodes {
            let mut visited = BTreeSet::new();
            let mut cursor = Some(item.key.as_str());
            while let Some(key) = cursor {
                if !visited.insert(key) {
                    return Err("project-template structure contains a cycle".into());
                }
                cursor = nodes.get(key).and_then(|node| node.parent_key.as_deref());
            }
        }
        let mut positions: BTreeSet<(Option<&str>, u32)> = BTreeSet::new();
        for item in &self.nodes {
            if !positions.insert((item.parent_key.as_deref(), item.position)) {
                return Err("project-template sibling node positions must be unique".into());
            }
        }
        let mut content_positions: BTreeSet<(Option<&str>, String, u32)> = BTreeSet::new();
        for item in &self.content_units {
            match (item.kind, item.parent_key.as_deref()) {
                (ContentUnitKind::ChapterBody, Some(key))
                    if nodes
                        .get(key)
                        .is_some_and(|node| node.kind == NodeKind::Chapter) => {}
                (ContentUnitKind::SectionBody, Some(key))
                    if nodes
                        .get(key)
                        .is_some_and(|node| node.kind == NodeKind::Section) => {}
                (ContentUnitKind::FrontMatter | ContentUnitKind::BackMatter, None) => {}
                _ => return Err("template content-unit kind and parent do not match".into()),
            }
            // Front matter and back matter are distinct root collections and
            // may each start at position zero.
            let root_kind = if item.parent_key.is_none() {
                format!("root:{:?}", item.kind)
            } else {
                "children".to_owned()
            };
            if !content_positions.insert((item.parent_key.as_deref(), root_kind, item.position)) {
                return Err("project-template content positions must be unique per parent".into());
            }
        }
        let mut scene_positions: BTreeSet<(&str, u32)> = BTreeSet::new();
        for item in &self.scenes {
            if !nodes
                .get(item.chapter_key.as_str())
                .is_some_and(|node| node.kind == NodeKind::Chapter)
            {
                return Err(format!(
                    "scene {} must reference a template chapter",
                    item.key
                ));
            }
            if !scene_positions.insert((item.chapter_key.as_str(), item.position)) {
                return Err("project-template scene positions must be unique per chapter".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookType {
    #[default]
    Fiction,
    Nonfiction,
    Custom,
}

impl BookType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fiction => "fiction",
            Self::Nonfiction => "nonfiction",
            Self::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fiction" => Some(Self::Fiction),
            "nonfiction" | "non-fiction" => Some(Self::Nonfiction),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructuralNode {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub kind: NodeKind,
    pub title: String,
    pub position: u32,
    pub target_words: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub id: SceneId,
    pub title: String,
    pub synopsis: String,
    pub status: SceneStatus,
    pub target_words: Option<u64>,
    pub purpose: String,
    pub goal: String,
    pub outcome: String,
    pub start_time: StoryTime,
    pub end_time: StoryTime,
    pub pov_character: Option<CharacterId>,
    pub start_location: Option<LocationId>,
    pub end_location: Option<LocationId>,
    /// Optional BCP 47 language override; `None` inherits the project default.
    #[serde(default)]
    pub language_code: Option<String>,
    /// Optional counting convention; `None` inherits the project policy.
    #[serde(default)]
    pub word_count_policy: Option<WordCountPolicy>,
    pub archived: bool,
}
impl Scene {
    pub fn effective_language_code<'a>(&'a self, project: &'a Project) -> &'a str {
        self.language_code
            .as_deref()
            .unwrap_or(&project.language_code)
    }

    pub fn effective_word_count_policy(&self, project: &Project) -> WordCountPolicy {
        self.word_count_policy.unwrap_or(project.word_count_policy)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenePlacement {
    pub scene_id: SceneId,
    pub chapter_id: NodeId,
    pub position: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ParticipationRole {
    Pov,
    Major,
    Minor,
    OffCamera,
    Mentioned,
}
impl ParticipationRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pov => "pov",
            Self::Major => "major",
            Self::Minor => "minor",
            Self::OffCamera => "off_camera",
            Self::Mentioned => "mentioned",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneCharacter {
    pub character_id: CharacterId,
    pub role: ParticipationRole,
    pub notes: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenePlotline {
    pub plotline_id: PlotlineId,
    pub role: String,
    pub share_basis_points: Option<u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneEvent {
    pub event_id: EventId,
    pub relationship_kind: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Character {
    pub id: CharacterId,
    pub full_name: String,
    pub preferred_name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub background: String,
    pub goals: String,
    pub motivations: String,
    pub secrets: String,
    pub internal_conflict: String,
    pub external_conflict: String,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plotline {
    pub id: PlotlineId,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub priority: i32,
    pub beginning_state: String,
    pub intended_development: String,
    pub intended_resolution: String,
    pub status: String,
    pub target_words: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    pub id: LocationId,
    pub parent: Option<LocationId>,
    pub name: String,
    pub description: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoryEvent {
    pub id: EventId,
    pub title: String,
    pub description: String,
    pub start: StoryTime,
    pub end: StoryTime,
    pub source_time_text: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventParticipationRole {
    Participant,
    Witness,
    Cause,
    Affected,
}
impl EventParticipationRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Participant => "participant",
            Self::Witness => "witness",
            Self::Cause => "cause",
            Self::Affected => "affected",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventCharacterParticipation {
    pub character_id: CharacterId,
    pub role: EventParticipationRole,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLocationRole {
    OccurredAt,
    StartsAt,
    EndsAt,
}
impl EventLocationRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OccurredAt => "occurred_at",
            Self::StartsAt => "starts_at",
            Self::EndsAt => "ends_at",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventLocationAssociation {
    pub location_id: LocationId,
    pub role: EventLocationRole,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoryEventDependency {
    pub predecessor: EventId,
    pub successor: EventId,
    pub kind: String,
    pub notes: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocationIncompatibilityRule {
    pub id: ContinuityRuleId,
    pub first_location: LocationId,
    pub second_location: LocationId,
    pub description: String,
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WordAttributionPolicy {
    Overlap,
    EqualShare,
    AuthorShares,
}
pub fn validate_shares(links: &[ScenePlotline]) -> Result<(), DomainError> {
    let unique: std::collections::BTreeSet<_> = links.iter().map(|x| x.plotline_id).collect();
    if unique.len() != links.len() {
        return Err(DomainError::InvalidAllocation(
            "duplicate plotline allocation".into(),
        ));
    }
    if links.iter().any(|x| x.share_basis_points.is_none()) {
        return Err(DomainError::InvalidAllocation(
            "every plotline needs a share".into(),
        ));
    }
    let total: u32 = links
        .iter()
        .map(|x| u32::from(x.share_basis_points.unwrap()))
        .sum();
    if total != 10_000 {
        return Err(DomainError::InvalidAllocation(format!(
            "shares total {total}, expected 10000"
        )));
    }
    Ok(())
}
/// Largest-remainder apportionment; ties use lexicographic UUID for stable results.
pub fn allocate_words(
    words: u64,
    links: &[ScenePlotline],
) -> Result<BTreeMap<PlotlineId, u64>, DomainError> {
    validate_shares(links)?;
    let mut rows: Vec<_> = links
        .iter()
        .map(|x| {
            let n = u128::from(words) * u128::from(x.share_basis_points.unwrap());
            (x.plotline_id, (n / 10_000) as u64, (n % 10_000) as u64)
        })
        .collect();
    let assigned: u64 = rows.iter().map(|x| x.1).sum();
    let mut left = words - assigned;
    rows.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
    });
    for row in &mut rows {
        if left == 0 {
            break;
        }
        row.1 += 1;
        left -= 1;
    }
    Ok(rows.into_iter().map(|x| (x.0, x.1)).collect())
}

pub fn attribute_words(
    words: u64,
    links: &[ScenePlotline],
    policy: WordAttributionPolicy,
) -> Result<BTreeMap<PlotlineId, u64>, DomainError> {
    if links
        .iter()
        .map(|l| l.plotline_id)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != links.len()
    {
        return Err(DomainError::InvalidAllocation(
            "duplicate plotline membership".into(),
        ));
    }
    match policy {
        WordAttributionPolicy::Overlap => {
            Ok(links.iter().map(|l| (l.plotline_id, words)).collect())
        }
        WordAttributionPolicy::AuthorShares => allocate_words(words, links),
        WordAttributionPolicy::EqualShare => {
            if links.is_empty() {
                return Ok(BTreeMap::new());
            }
            let base = 10_000 / (links.len() as u32);
            let rem = 10_000 % (links.len() as u32);
            let mut equal = links.to_vec();
            equal.sort_by_key(|l| l.plotline_id.to_string());
            for (i, l) in equal.iter_mut().enumerate() {
                l.share_basis_points = Some((base + u32::from((i as u32) < rem)) as u16);
            }
            allocate_words(words, &equal)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    Text,
    LongText,
    Integer,
    Decimal,
    Boolean,
    DateTime,
    Enum,
    EntityRef,
    EntityRefs,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FieldValue {
    Text(String),
    Integer(i64),
    Decimal(String),
    Boolean(bool),
    DateTime(String),
    Enum(String),
    EntityRef(String),
    EntityRefs(Vec<String>),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomFieldDefinition {
    pub id: FieldId,
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub default_value: Option<FieldValue>,
    pub enum_values: Vec<String>,
    pub reference_kind: Option<String>,
}
impl CustomFieldDefinition {
    pub fn validate(&self, value: &FieldValue) -> Result<(), DomainError> {
        let okay = matches!(
            (self.field_type, value),
            (FieldType::Text, FieldValue::Text(_))
                | (FieldType::LongText, FieldValue::Text(_))
                | (FieldType::Integer, FieldValue::Integer(_))
                | (FieldType::Decimal, FieldValue::Decimal(_))
                | (FieldType::Boolean, FieldValue::Boolean(_))
                | (FieldType::DateTime, FieldValue::DateTime(_))
                | (FieldType::Enum, FieldValue::Enum(_))
                | (FieldType::EntityRef, FieldValue::EntityRef(_))
                | (FieldType::EntityRefs, FieldValue::EntityRefs(_))
        );
        if !okay {
            return Err(DomainError::InvalidField(
                "value type does not match field".into(),
            ));
        }
        if let FieldValue::Enum(v) = value
            && !self.enum_values.contains(v)
        {
            return Err(DomainError::InvalidField(
                "value is outside enum options".into(),
            ));
        }
        if let FieldValue::Decimal(v) = value
            && !matches!(v.parse::<f64>(), Ok(n) if n.is_finite())
        {
            return Err(DomainError::InvalidField("invalid decimal value".into()));
        }
        if let FieldValue::DateTime(v) = value
            && !valid_datetime(v)
        {
            return Err(DomainError::InvalidField(
                "datetime must be RFC 3339".into(),
            ));
        }
        Ok(())
    }
}

fn valid_datetime(v: &str) -> bool {
    let b = v.as_bytes();
    if !v.is_ascii()
        || b.len() < 20
        || b.get(4) != Some(&b'-')
        || b.get(7) != Some(&b'-')
        || b.get(10) != Some(&b'T')
        || b.get(13) != Some(&b':')
        || b.get(16) != Some(&b':')
    {
        return false;
    }
    if ![0..4, 5..7, 8..10, 11..13, 14..16, 17..19]
        .iter()
        .all(|r| b[r.clone()].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let hour = (b[11] - b'0') as u32 * 10 + (b[12] - b'0') as u32;
    let minute = (b[14] - b'0') as u32 * 10 + (b[15] - b'0') as u32;
    let second = (b[17] - b'0') as u32 * 10 + (b[18] - b'0') as u32;
    if hour > 23 || minute > 59 || second > 60 {
        return false;
    }
    let mut rest = &v[19..];
    if rest.starts_with('.') {
        let fraction_end = rest[1..]
            .find(['Z', '+', '-'])
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        if fraction_end <= 1 || !rest[1..fraction_end].bytes().all(|c| c.is_ascii_digit()) {
            return false;
        }
        rest = &rest[fraction_end..];
    }
    if rest == "Z" {
        return true;
    }
    if let Some(zone) = rest.strip_prefix('+').or_else(|| rest.strip_prefix('-')) {
        let z = zone.as_bytes();
        return z.len() == 5
            && z[2] == b':'
            && z[..2].iter().all(u8::is_ascii_digit)
            && z[3..].iter().all(u8::is_ascii_digit)
            && z[..2]
                .iter()
                .fold(0u32, |n, d| n * 10 + u32::from(*d - b'0'))
                <= 23
            && z[3..]
                .iter()
                .fold(0u32, |n, d| n * 10 + u32::from(*d - b'0'))
                <= 59;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_is_not_zero_and_overlap_is_ambiguous() {
        assert_eq!(
            StoryTime::unknown().compare(StoryTime::exact(0)),
            TimeOrdering::Ambiguous
        );
        let a = StoryTime::new(Some(5), Some(0), Some(10), TimePrecision::Bounded).unwrap();
        assert_eq!(a.compare(StoryTime::exact(7)), TimeOrdering::Ambiguous);
    }

    #[test]
    fn story_intervals_handle_inclusive_exclusive_bounds_and_uncertainty() {
        use IntervalBoundary::{Exclusive, Inclusive};

        let earlier = StoryInterval::closed(StoryTime::exact(9), StoryTime::exact(10));
        let touching = StoryInterval::closed(StoryTime::exact(10), StoryTime::exact(16));
        assert_eq!(earlier.compare(touching), TimeOrdering::Ambiguous);

        let exclusive_end = StoryInterval {
            end_boundary: Exclusive,
            ..earlier
        };
        assert_eq!(exclusive_end.compare(touching), TimeOrdering::Before);
        let exclusive_start = StoryInterval {
            start_boundary: Exclusive,
            ..touching
        };
        assert_eq!(earlier.compare(exclusive_start), TimeOrdering::Before);

        let point = StoryInterval::closed(
            StoryTime::exact(15 * 3_600 + 13 * 60),
            StoryTime::exact(15 * 3_600 + 13 * 60),
        );
        let day_range =
            StoryInterval::closed(StoryTime::exact(9 * 3_600), StoryTime::exact(16 * 3_600));
        assert_eq!(point.compare(day_range), TimeOrdering::Ambiguous);
        assert_eq!(
            StoryInterval::closed(StoryTime::exact(10), StoryTime::unknown()).compare(day_range),
            TimeOrdering::Ambiguous
        );

        let overnight = StoryInterval::closed(
            StoryTime::exact(23 * 3_600),
            StoryTime::exact(STORY_TICKS_PER_DAY + 3_600),
        );
        assert_eq!(overnight.duration().unwrap(), Some(7_200));
        assert_eq!(overnight.start_boundary, Inclusive);
    }
    #[test]
    fn duration_overflow_is_checked() {
        assert!(
            StoryTime::exact(i64::MIN)
                .duration_until(StoryTime::exact(i64::MAX))
                .is_err()
        );
    }
    #[test]
    fn bounded_times_allow_no_nominal_and_duration_propagates_uncertainty() {
        let a = StoryTime::new(None, Some(-20), Some(-10), TimePrecision::Bounded).unwrap();
        let b = StoryTime::new(None, Some(5), Some(15), TimePrecision::Bounded).unwrap();
        assert_eq!(a.compare(b), TimeOrdering::Before);
        assert_eq!(a.duration_bounds_until(b).unwrap(), Some((15, 35)));
        let approximate = StoryTime::new(Some(2), None, None, TimePrecision::Approximate).unwrap();
        assert_eq!(
            approximate.compare(StoryTime::exact(3)),
            TimeOrdering::Ambiguous
        );
        assert_eq!(STORY_TICKS_PER_DAY, 86400);
    }
    #[test]
    fn shares_reject_incomplete_and_apportion_exactly() {
        let a = PlotlineId::new();
        let b = PlotlineId::new();
        let links = vec![
            ScenePlotline {
                plotline_id: a,
                role: "advances".into(),
                share_basis_points: Some(3333),
            },
            ScenePlotline {
                plotline_id: b,
                role: "advances".into(),
                share_basis_points: Some(6667),
            },
        ];
        let out = allocate_words(7, &links).unwrap();
        assert_eq!(out.values().sum::<u64>(), 7);
        assert!(validate_shares(&links[..1]).is_err());
    }
    #[test]
    fn attribution_policies_are_explicit_and_equal_share_totals_match() {
        let a = PlotlineId::new();
        let b = PlotlineId::new();
        let links = vec![
            ScenePlotline {
                plotline_id: a,
                role: "advances".into(),
                share_basis_points: None,
            },
            ScenePlotline {
                plotline_id: b,
                role: "advances".into(),
                share_basis_points: None,
            },
        ];
        let overlap = attribute_words(9, &links, WordAttributionPolicy::Overlap).unwrap();
        assert_eq!(overlap[&a], 9);
        assert_eq!(overlap[&b], 9);
        let equal = attribute_words(9, &links, WordAttributionPolicy::EqualShare).unwrap();
        assert_eq!(equal.values().sum::<u64>(), 9);
        assert!(equal.values().all(|n| *n == 4 || *n == 5));
    }
    #[test]
    fn datetime_custom_values_are_type_checked() {
        let d = CustomFieldDefinition {
            id: FieldId::new(),
            name: "When".into(),
            field_type: FieldType::DateTime,
            required: false,
            default_value: None,
            enum_values: vec![],
            reference_kind: None,
        };
        assert!(
            d.validate(&FieldValue::DateTime("2026-11-20T13:45:00Z".into()))
                .is_ok()
        );
        assert!(
            d.validate(&FieldValue::DateTime("tomorrow".into()))
                .is_err()
        );
    }

    #[test]
    fn templates_allow_separate_front_and_back_matter_positions_and_reject_cycles() {
        let mut template = BuiltinTemplate::BlankNonfiction.definition();
        template.content_units = vec![
            ProjectTemplateContentUnit {
                key: "front".into(),
                parent_key: None,
                kind: ContentUnitKind::FrontMatter,
                title: "Preface".into(),
                position: 0,
                target_words: None,
            },
            ProjectTemplateContentUnit {
                key: "back".into(),
                parent_key: None,
                kind: ContentUnitKind::BackMatter,
                title: "Bibliography".into(),
                position: 0,
                target_words: None,
            },
        ];
        assert!(template.validate().is_ok());
        template.nodes = vec![
            ProjectTemplateNode {
                key: "one".into(),
                parent_key: Some("two".into()),
                kind: NodeKind::Section,
                title: "One".into(),
                position: 0,
            },
            ProjectTemplateNode {
                key: "two".into(),
                parent_key: Some("one".into()),
                kind: NodeKind::Section,
                title: "Two".into(),
                position: 0,
            },
        ];
        assert!(template.validate().unwrap_err().contains("cycle"));
    }
}

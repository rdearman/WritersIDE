use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SourceType {
    Scrivener,
    Spreadsheet,
    Docx,
    Rtf,
    Odt,
    Epub,
    Unknown(String),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InventoryItem {
    pub source_document_id: String,
    pub item_type: String,
    pub parent_source_id: Option<String>,
    pub source_sheet: Option<String>,
    pub source_row: Option<u32>,
    pub fields: BTreeMap<String, String>,
    pub unsupported: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportInventory {
    pub source_type: SourceType,
    pub source_identifier: String,
    pub source_fingerprint: String,
    pub items: Vec<InventoryItem>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldMapping {
    pub source_field: String,
    pub destination_entity: String,
    pub destination_field: String,
    pub conversion: Option<String>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MappingPlan {
    pub fields: Vec<FieldMapping>,
    pub unresolved: Vec<String>,
    pub decisions: BTreeMap<String, String>,
}
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ImportError {
    #[error("duplicate source document identifier: {0}")]
    DuplicateSourceId(String),
    #[error("missing parent source item: {0}")]
    MissingParent(String),
    #[error("mapping refers to absent source field: {0}")]
    MissingField(String),
    #[error("unsupported manuscript format: {0}")]
    UnsupportedFormat(String),
    #[error("cannot read manuscript source: {0}")]
    Io(String),
    #[error("invalid manuscript archive: {0}")]
    Archive(String),
    #[error("invalid Scrivener import mapping: {0}")]
    InvalidMapping(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManuscriptUnit {
    pub source_document_id: String,
    pub title: String,
    pub body_markdown: String,
    pub unsupported: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManuscriptStaging {
    pub source_type: SourceType,
    pub source_path: String,
    pub source_fingerprint: String,
    pub title: String,
    pub units: Vec<ManuscriptUnit>,
    pub warnings: Vec<String>,
    pub unsupported: Vec<String>,
}

/// Read-only inventory of a Scrivener package. This records what the detected
/// manifest says; a version string alone is not a compatibility guarantee.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerProjectInventory {
    pub source_path: String,
    pub source_fingerprint: String,
    pub title: String,
    pub format_version: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    pub items: Vec<ScrivenerBinderItem>,
    pub warnings: Vec<String>,
    pub unsupported: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerBinderItem {
    pub source_document_id: String,
    pub parent_source_id: Option<String>,
    pub position: u32,
    pub title: String,
    pub type_code: Option<String>,
    pub area: String,
    pub include_in_compile: Option<bool>,
    pub synopsis: Option<String>,
    pub notes_markdown: Option<String>,
    pub content_markdown: Option<String>,
    pub fields: BTreeMap<String, String>,
    pub unsupported: Vec<String>,
}

/// Explicit destination choice shown in a Scrivener import preview. `Unmapped`
/// is intentional: folder names and binder roles are not sufficient evidence
/// to invent chapter/scene boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ScrivenerDestinationKind {
    Unmapped,
    Section,
    Chapter,
    Scene,
    Research,
    Skip,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ScrivenerImportMode {
    Editable,
    #[default]
    Both,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerMappingDecision {
    pub source_document_id: String,
    pub destination_kind: ScrivenerDestinationKind,
    pub title_override: Option<String>,
    pub include_in_import: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerImportPreview {
    pub total_items: usize,
    pub manuscript_items: usize,
    pub research_items: usize,
    pub skipped_items: usize,
    pub unresolved_source_ids: Vec<String>,
    pub unsupported: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerImportPlan {
    pub decisions: Vec<ScrivenerMappingDecision>,
}

impl ScrivenerImportPlan {
    pub fn proposed(inventory: &ScrivenerProjectInventory) -> Self {
        let decisions = inventory
            .items
            .iter()
            .map(|item| {
                let destination_kind = match item.area.as_str() {
                    "research" => ScrivenerDestinationKind::Research,
                    "trash" => ScrivenerDestinationKind::Skip,
                    _ if item.include_in_compile == Some(false) => ScrivenerDestinationKind::Skip,
                    // Binder folder roles vary by project; make the author
                    // choose section vs chapter in the preview.
                    _ if item.type_code.as_deref().is_some_and(|value| {
                        value.eq_ignore_ascii_case("folder") || value.eq_ignore_ascii_case("draft")
                    }) =>
                    {
                        ScrivenerDestinationKind::Unmapped
                    }
                    _ if item.content_markdown.is_some() => ScrivenerDestinationKind::Scene,
                    _ => ScrivenerDestinationKind::Unmapped,
                };
                ScrivenerMappingDecision {
                    source_document_id: item.source_document_id.clone(),
                    destination_kind,
                    title_override: None,
                    include_in_import: destination_kind != ScrivenerDestinationKind::Skip,
                }
            })
            .collect();
        Self { decisions }
    }

    pub fn preview(
        &self,
        inventory: &ScrivenerProjectInventory,
    ) -> Result<ScrivenerImportPreview, ImportError> {
        inventory.validate()?;
        let mut source_ids = BTreeSet::new();
        let mut preview = ScrivenerImportPreview {
            total_items: inventory.items.len(),
            manuscript_items: 0,
            research_items: 0,
            skipped_items: 0,
            unresolved_source_ids: Vec::new(),
            unsupported: inventory.unsupported.clone(),
        };
        for decision in &self.decisions {
            if !source_ids.insert(decision.source_document_id.as_str()) {
                return Err(ImportError::DuplicateSourceId(
                    decision.source_document_id.clone(),
                ));
            }
            let Some(item) = inventory
                .items
                .iter()
                .find(|item| item.source_document_id == decision.source_document_id)
            else {
                return Err(ImportError::MissingParent(
                    decision.source_document_id.clone(),
                ));
            };
            if !decision.include_in_import
                || decision.destination_kind == ScrivenerDestinationKind::Skip
            {
                preview.skipped_items += 1;
                continue;
            }
            match decision.destination_kind {
                ScrivenerDestinationKind::Unmapped => {
                    preview
                        .unresolved_source_ids
                        .push(decision.source_document_id.clone());
                }
                ScrivenerDestinationKind::Section | ScrivenerDestinationKind::Chapter => {
                    preview.manuscript_items += 1;
                }
                ScrivenerDestinationKind::Scene => {
                    preview.manuscript_items += 1;
                    if item.content_markdown.is_none() {
                        preview.unsupported.push(format!(
                            "{} is mapped to a scene but has no readable prose file",
                            item.source_document_id
                        ));
                    }
                }
                ScrivenerDestinationKind::Research => preview.research_items += 1,
                ScrivenerDestinationKind::Skip => unreachable!(),
            }
        }
        for item in &inventory.items {
            if !source_ids.contains(item.source_document_id.as_str()) {
                preview
                    .unresolved_source_ids
                    .push(item.source_document_id.clone());
            }
            preview.unsupported.extend(item.unsupported.iter().cloned());
        }
        preview.unresolved_source_ids.sort();
        preview.unresolved_source_ids.dedup();
        preview.unsupported.sort();
        preview.unsupported.dedup();
        Ok(preview)
    }

    pub fn validate_for_import(
        &self,
        inventory: &ScrivenerProjectInventory,
    ) -> Result<(), ImportError> {
        let preview = self.preview(inventory)?;
        if !preview.unresolved_source_ids.is_empty() {
            return Err(ImportError::InvalidMapping(format!(
                "unmapped binder items: {}",
                preview.unresolved_source_ids.join(", ")
            )));
        }
        let decisions = self
            .decisions
            .iter()
            .map(|decision| (decision.source_document_id.as_str(), decision))
            .collect::<BTreeMap<_, _>>();
        for item in &inventory.items {
            let decision = decisions[item.source_document_id.as_str()];
            if !decision.include_in_import
                || decision.destination_kind == ScrivenerDestinationKind::Skip
            {
                continue;
            }
            if let Some(parent_id) = item.parent_source_id.as_deref() {
                let parent = decisions.get(parent_id).ok_or_else(|| {
                    ImportError::InvalidMapping(format!("missing parent mapping: {parent_id}"))
                })?;
                if !parent.include_in_import
                    || parent.destination_kind == ScrivenerDestinationKind::Skip
                {
                    return Err(ImportError::InvalidMapping(format!(
                        "selected item {} is beneath skipped parent {parent_id}",
                        item.source_document_id
                    )));
                }
                let valid_parent = match decision.destination_kind {
                    ScrivenerDestinationKind::Section | ScrivenerDestinationKind::Chapter => {
                        parent.destination_kind == ScrivenerDestinationKind::Section
                    }
                    ScrivenerDestinationKind::Scene => {
                        parent.destination_kind == ScrivenerDestinationKind::Chapter
                    }
                    ScrivenerDestinationKind::Research => {
                        parent.destination_kind == ScrivenerDestinationKind::Research
                            || parent.destination_kind == ScrivenerDestinationKind::Section
                            || parent.destination_kind == ScrivenerDestinationKind::Chapter
                    }
                    ScrivenerDestinationKind::Unmapped | ScrivenerDestinationKind::Skip => true,
                };
                if !valid_parent {
                    return Err(ImportError::InvalidMapping(format!(
                        "binder item {} has an incompatible mapped parent {parent_id}",
                        item.source_document_id
                    )));
                }
            } else if decision.destination_kind == ScrivenerDestinationKind::Scene {
                return Err(ImportError::InvalidMapping(format!(
                    "scene {} needs an explicitly mapped chapter parent",
                    item.source_document_id
                )));
            }
        }
        Ok(())
    }
}

impl ScrivenerProjectInventory {
    pub fn validate(&self) -> Result<(), ImportError> {
        let mut ids = BTreeSet::new();
        for item in &self.items {
            if !ids.insert(item.source_document_id.as_str()) {
                return Err(ImportError::DuplicateSourceId(
                    item.source_document_id.clone(),
                ));
            }
        }
        for item in &self.items {
            if let Some(parent) = &item.parent_source_id
                && !ids.contains(parent.as_str())
            {
                return Err(ImportError::MissingParent(parent.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MergeConflict {
    pub base_start_line: usize,
    pub base_end_line: usize,
    pub base: String,
    pub local: String,
    pub external: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThreeWayMerge {
    /// `None` means overlapping edits require author review.
    pub merged_text: Option<String>,
    pub conflicts: Vec<MergeConflict>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MergeConflictChoice {
    Base,
    Local,
    External,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DocxReviewItem {
    pub kind: String,
    /// Zero-based source paragraph. `None` means Word supplied no usable range anchor.
    pub paragraph_index: Option<usize>,
    pub comment_id: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub text: String,
}

/// Inventories Word insertions, deletions, and comments without applying them.
pub fn inspect_docx_review(path: impl AsRef<Path>) -> Result<Vec<DocxReviewItem>, ImportError> {
    let bytes = std::fs::read(path).map_err(|error| ImportError::Io(error.to_string()))?;
    let document = read_zip_text(&bytes, "word/document.xml")?;
    let comment_paragraphs = docx_comment_paragraphs(&document)?;
    let mut items = Vec::new();
    for (tag, kind, text_tag) in [
        ("w:ins", "insertion", "w:t"),
        ("w:del", "deletion", "w:delText"),
    ] {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        let mut rest = document.as_str();
        while let Some(start) = rest.find(&open) {
            rest = &rest[start..];
            let Some(end) = rest.find(&close) else {
                break;
            };
            let block = &rest[..end];
            let prefix = &document[..document.len() - rest.len()];
            items.push(DocxReviewItem {
                kind: kind.into(),
                paragraph_index: Some(prefix.matches("<w:p").count().saturating_sub(1)),
                comment_id: None,
                author: xml_attribute(block, "w:author"),
                date: xml_attribute(block, "w:date"),
                text: paragraph_xml_to_text(block, text_tag),
            });
            rest = &rest[end + close.len()..];
        }
    }
    if let Ok(comments) = read_zip_text(&bytes, "word/comments.xml") {
        let mut rest = comments.as_str();
        while let Some(start) = rest.find("<w:comment ") {
            rest = &rest[start..];
            let Some(end) = rest.find("</w:comment>") else {
                break;
            };
            let block = &rest[..end];
            let comment_id = xml_attribute(block, "w:id");
            items.push(DocxReviewItem {
                kind: "comment".into(),
                paragraph_index: comment_id
                    .as_ref()
                    .and_then(|id| comment_paragraphs.get(id))
                    .copied(),
                comment_id,
                author: xml_attribute(block, "w:author"),
                date: xml_attribute(block, "w:date"),
                text: paragraph_xml_to_text(block, "w:t"),
            });
            rest = &rest[end + "</w:comment>".len()..];
        }
    }
    items.sort_by_key(|item| (item.paragraph_index, item.kind.clone()));
    Ok(items)
}

/// Maps Word comment range/reference markers to the first source paragraph they touch.
/// A comment that has no marker in the main document remains unanchored (`None`).
fn docx_comment_paragraphs(xml: &str) -> Result<BTreeMap<String, usize>, ImportError> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut paragraph = None;
    let mut next_paragraph = 0usize;
    let mut anchors = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) if event.local_name().as_ref() == b"p" => {
                paragraph = Some(next_paragraph);
                next_paragraph += 1;
            }
            Ok(Event::End(event)) if event.local_name().as_ref() == b"p" => {
                paragraph = None;
            }
            Ok(Event::Start(event) | Event::Empty(event))
                if matches!(
                    event.local_name().as_ref(),
                    b"commentRangeStart" | b"commentRangeEnd" | b"commentReference"
                ) =>
            {
                if let Some(index) = paragraph
                    && let Some(id) = xml_event_attribute(&event, "id", reader.decoder())?
                {
                    anchors.entry(id).or_insert(index);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ImportError::Archive(error.to_string())),
        }
    }
    Ok(anchors)
}

fn xml_event_attribute(
    event: &quick_xml::events::BytesStart<'_>,
    name: &str,
    decoder: quick_xml::encoding::Decoder,
) -> Result<Option<String>, ImportError> {
    for attribute in event.attributes().flatten() {
        if attribute.key.local_name().as_ref() == name.as_bytes() {
            return attribute
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder)
                .map(|value| Some(value.into_owned()))
                .map_err(|error| ImportError::Archive(error.to_string()));
        }
    }
    Ok(None)
}

/// Produces the accepted or rejected prose view of a DOCX without modifying the source archive.
/// Formatting and comment ranges are not applied to canonical Markdown; callers must present the
/// returned text as a review candidate and merge it through the normal conflict-aware workflow.
pub fn docx_review_text(
    path: impl AsRef<Path>,
    accept_changes: bool,
) -> Result<String, ImportError> {
    let bytes = std::fs::read(path).map_err(|error| ImportError::Io(error.to_string()))?;
    let xml = read_zip_text(&bytes, "word/document.xml")?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    let mut paragraphs = Vec::<String>::new();
    let mut in_paragraph = false;
    let mut inserted = 0usize;
    let mut deleted = 0usize;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event)) => match event.local_name().as_ref() {
                b"p" => {
                    paragraphs.push(String::new());
                    in_paragraph = true;
                }
                b"ins" => inserted += 1,
                b"del" => deleted += 1,
                _ => {}
            },
            Ok(quick_xml::events::Event::Empty(event)) if in_paragraph => {
                let paragraph = paragraphs.last_mut().expect("paragraph has been started");
                match event.local_name().as_ref() {
                    b"tab" => paragraph.push('\t'),
                    b"br" | b"cr" => paragraph.push('\n'),
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Text(event))
                if in_paragraph
                    && (if accept_changes {
                        deleted == 0
                    } else {
                        inserted == 0
                    }) =>
            {
                let decoded = event
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                let text = quick_xml::escape::unescape(&decoded)
                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                paragraphs
                    .last_mut()
                    .expect("paragraph has been started")
                    .push_str(&text);
            }
            Ok(quick_xml::events::Event::End(event)) => match event.local_name().as_ref() {
                b"p" => in_paragraph = false,
                b"ins" => inserted = inserted.saturating_sub(1),
                b"del" => deleted = deleted.saturating_sub(1),
                _ => {}
            },
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ImportError::Archive(error.to_string())),
        }
    }
    Ok(paragraphs.join("\n\n"))
}

fn xml_attribute(xml: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let start = xml.find(&key)? + key.len();
    let end = xml[start..].find('\"')? + start;
    Some(decode_xml(&xml[start..end]))
}

/// Line-oriented three-way merge. It combines non-overlapping changes and
/// returns exact conflict regions instead of inserting conflict markers into prose.
pub fn three_way_merge(base: &str, local: &str, external: &str) -> ThreeWayMerge {
    if local == external {
        return ThreeWayMerge {
            merged_text: Some(local.into()),
            conflicts: Vec::new(),
        };
    }
    if local == base {
        return ThreeWayMerge {
            merged_text: Some(external.into()),
            conflicts: Vec::new(),
        };
    }
    if external == base {
        return ThreeWayMerge {
            merged_text: Some(local.into()),
            conflicts: Vec::new(),
        };
    }
    let base_lines = base.split_inclusive('\n').collect::<Vec<_>>();
    let local_edits = line_edits(
        &base_lines,
        &local.split_inclusive('\n').collect::<Vec<_>>(),
    );
    let external_edits = line_edits(
        &base_lines,
        &external.split_inclusive('\n').collect::<Vec<_>>(),
    );
    let mut edits = local_edits
        .iter()
        .map(|edit| (edit.clone(), false))
        .chain(external_edits.iter().map(|edit| (edit.clone(), true)))
        .collect::<Vec<_>>();
    edits.sort_by_key(|(edit, is_external)| (edit.start, edit.end, *is_external));
    let mut unique: Vec<(LineEdit, bool)> = Vec::new();
    let mut conflicts = Vec::new();
    for (edit, is_external) in edits {
        let Some((previous, previous_external)) = unique.last() else {
            unique.push((edit, is_external));
            continue;
        };
        let overlaps = edit.start < previous.end
            || (edit.start == previous.start && edit.end == previous.end)
            || (edit.start == edit.end
                && edit.start >= previous.start
                && edit.start <= previous.end)
            || (previous.start == previous.end
                && previous.start >= edit.start
                && previous.start <= edit.end);
        if !overlaps {
            unique.push((edit, is_external));
            continue;
        }
        if edit == *previous {
            if *previous_external && !is_external { /* retain first */ }
            continue;
        }
        let start = edit.start.min(previous.start);
        let end = edit.end.max(previous.end);
        let side = |items: &[LineEdit]| -> String {
            let mut value = String::new();
            let mut cursor = start;
            for item in items
                .iter()
                .filter(|item| item.start >= start && item.end <= end)
            {
                value.push_str(&base_lines[cursor..item.start].concat());
                value.push_str(&item.replacement.iter().copied().collect::<String>());
                cursor = item.end;
            }
            value.push_str(&base_lines[cursor..end].concat());
            value
        };
        if !conflicts.iter().any(|conflict: &MergeConflict| {
            conflict.base_start_line == start && conflict.base_end_line == end
        }) {
            conflicts.push(MergeConflict {
                base_start_line: start,
                base_end_line: end,
                base: base_lines[start..end].concat(),
                local: side(&local_edits),
                external: side(&external_edits),
            });
        }
        // Merge conflicts are kept out of the proposed output. Continue to collect
        // disjoint conflicts, but don't use conflicting edits in the auto-merge.
        let _ = (previous_external, is_external);
    }
    if !conflicts.is_empty() {
        return ThreeWayMerge {
            merged_text: None,
            conflicts,
        };
    }
    unique.sort_by_key(|(edit, _)| (edit.start, edit.end));
    let mut output = base_lines.clone();
    for (edit, _) in unique.into_iter().rev() {
        output.splice(edit.start..edit.end, edit.replacement);
    }
    ThreeWayMerge {
        merged_text: Some(output.concat()),
        conflicts,
    }
}

/// Resolves each reported overlapping range in order while retaining automatic
/// disjoint edits from both versions. `choices` must cover every conflict.
pub fn resolve_three_way_merge(
    base: &str,
    local: &str,
    external: &str,
    choices: &[MergeConflictChoice],
) -> Result<String, String> {
    let result = three_way_merge(base, local, external);
    if result.conflicts.is_empty() {
        if !choices.is_empty() {
            return Err("merge has no conflicts but resolution choices were provided".into());
        }
        return result
            .merged_text
            .ok_or_else(|| "automatic merge did not produce text".to_owned());
    }
    if choices.len() != result.conflicts.len() {
        return Err(format!(
            "expected {} conflict choices, received {}",
            result.conflicts.len(),
            choices.len()
        ));
    }
    let base_lines = base.split_inclusive('\n').collect::<Vec<_>>();
    let local_lines = local.split_inclusive('\n').collect::<Vec<_>>();
    let external_lines = external.split_inclusive('\n').collect::<Vec<_>>();
    let edits = line_edits(&base_lines, &local_lines)
        .into_iter()
        .chain(line_edits(&base_lines, &external_lines))
        .collect::<Vec<_>>();
    let mut operations: Vec<(usize, usize, Vec<&str>)> = Vec::new();
    for edit in edits {
        let in_conflict = result.conflicts.iter().any(|conflict| {
            if conflict.base_start_line == conflict.base_end_line {
                edit.start <= conflict.base_start_line && edit.end >= conflict.base_start_line
            } else if edit.start == edit.end {
                edit.start >= conflict.base_start_line && edit.start <= conflict.base_end_line
            } else {
                edit.start < conflict.base_end_line && edit.end > conflict.base_start_line
            }
        });
        if !in_conflict {
            let operation = (edit.start, edit.end, edit.replacement);
            if !operations.contains(&operation) {
                operations.push(operation);
            }
        }
    }
    for (conflict, choice) in result.conflicts.iter().zip(choices) {
        let replacement = match choice {
            MergeConflictChoice::Base => conflict.base.as_str(),
            MergeConflictChoice::Local => conflict.local.as_str(),
            MergeConflictChoice::External => conflict.external.as_str(),
        };
        operations.push((
            conflict.base_start_line,
            conflict.base_end_line,
            replacement.split_inclusive('\n').collect(),
        ));
    }
    operations.sort_by_key(|(start, end, _)| (*start, *end));
    let mut output = String::new();
    let mut cursor = 0;
    for (start, end, replacement) in operations {
        if start < cursor || end > base_lines.len() {
            return Err("merge conflict ranges overlap ambiguously".into());
        }
        output.push_str(&base_lines[cursor..start].concat());
        output.push_str(&replacement.concat());
        cursor = end;
    }
    output.push_str(&base_lines[cursor..].concat());
    Ok(output)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LineEdit<'a> {
    start: usize,
    end: usize,
    replacement: Vec<&'a str>,
}

fn line_edits<'a>(base: &[&'a str], changed: &[&'a str]) -> Vec<LineEdit<'a>> {
    // Bound memory use: pathological files fall back to one whole-document edit.
    if base
        .len()
        .saturating_add(1)
        .saturating_mul(changed.len().saturating_add(1))
        > 4_000_000
    {
        return vec![LineEdit {
            start: 0,
            end: base.len(),
            replacement: changed.to_vec(),
        }];
    }
    let width = changed.len() + 1;
    let mut lcs = vec![0usize; (base.len() + 1) * width];
    for i in (0..base.len()).rev() {
        for j in (0..changed.len()).rev() {
            lcs[i * width + j] = if base[i] == changed[j] {
                1 + lcs[(i + 1) * width + j + 1]
            } else {
                lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
            };
        }
    }
    let mut edits = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut pending: Option<LineEdit<'a>> = None;
    while i < base.len() || j < changed.len() {
        if i < base.len() && j < changed.len() && base[i] == changed[j] {
            if let Some(edit) = pending.take() {
                edits.push(edit);
            }
            i += 1;
            j += 1;
        } else if j < changed.len()
            && (i == base.len() || lcs[i * width + j + 1] >= lcs[(i + 1) * width + j])
        {
            let edit = pending.get_or_insert_with(|| LineEdit {
                start: i,
                end: i,
                replacement: Vec::new(),
            });
            edit.replacement.push(changed[j]);
            j += 1;
        } else {
            let edit = pending.get_or_insert_with(|| LineEdit {
                start: i,
                end: i,
                replacement: Vec::new(),
            });
            edit.end += 1;
            i += 1;
        }
    }
    if let Some(edit) = pending {
        edits.push(edit);
    }
    edits
}

/// Read-only inventory of DOCX, RTF, ODT, or EPUB content. The source bytes
/// are fingerprinted and never rewritten; formatting losses are reported.
pub fn inspect_manuscript(path: impl AsRef<Path>) -> Result<ManuscriptStaging, ImportError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|error| ImportError::Io(error.to_string()))?;
    let fingerprint = format!("{:x}", Sha256::digest(&bytes));
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let source_type = match extension.as_str() {
        "docx" => SourceType::Docx,
        "rtf" => SourceType::Rtf,
        "odt" => SourceType::Odt,
        "epub" => SourceType::Epub,
        _ => return Err(ImportError::UnsupportedFormat(extension)),
    };
    let title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Imported Manuscript")
        .to_owned();
    let mut staging = ManuscriptStaging {
        source_type: source_type.clone(),
        source_path: path.to_string_lossy().into_owned(),
        source_fingerprint: fingerprint.clone(),
        title,
        units: Vec::new(),
        warnings: Vec::new(),
        unsupported: Vec::new(),
    };
    match source_type {
        SourceType::Docx => {
            let xml = read_zip_text(&bytes, "word/document.xml")?;
            let blocks = extract_docx_blocks(&xml)?;
            split_blocks_into_units(&mut staging, blocks, "docx");
            if xml.contains("<w:tbl") {
                staging
                    .unsupported
                    .push("Tables are flattened to paragraph text".into());
            }
            if xml.contains("<w:drawing") || xml.contains("<w:pict") {
                staging
                    .unsupported
                    .push("Embedded images and drawings are not imported".into());
            }
            for (tag, description) in [
                ("w:u", "underline"),
                ("w:color", "font color"),
                ("w:highlight", "highlight"),
                ("w:rFonts", "run font"),
                ("w:caps", "capitalization"),
                ("w:smallCaps", "small-capitalization"),
                ("w:vanish", "hidden text"),
            ] {
                if xml.contains(&format!("<{tag}")) {
                    staging
                        .unsupported
                        .push(format!("DOCX {description} formatting is not preserved"));
                }
            }
            if xml.contains("<w:hyperlink") {
                staging
                    .unsupported
                    .push("DOCX hyperlink destinations are flattened to their anchor text".into());
            }
            for feature in [
                "<w:ins",
                "<w:del",
                "<w:commentRangeStart",
                "<w:footnoteReference",
                "<w:endnoteReference",
            ] {
                if xml.contains(feature) {
                    staging
                        .unsupported
                        .push(format!("DOCX feature requires review: {feature}"));
                }
            }
        }
        SourceType::Odt => {
            let xml = read_zip_text(&bytes, "content.xml")?;
            let blocks = extract_odt_blocks(&xml);
            split_blocks_into_units(&mut staging, blocks, "odt");
            if xml.contains("<draw:frame") {
                staging
                    .unsupported
                    .push("Embedded images and drawings are not imported".into());
            }
            if xml.contains("<table:table") {
                staging
                    .unsupported
                    .push("Tables are flattened to text".into());
            }
            if xml.contains("<text:span") || xml.contains("<text:a ") {
                staging.unsupported.push(
                    "Inline ODT styling and hyperlink semantics are flattened to plain text".into(),
                );
            }
        }
        SourceType::Rtf => {
            let source = String::from_utf8_lossy(&bytes);
            let text = extract_rtf_text(&source);
            staging.units.push(ManuscriptUnit {
                source_document_id: format!("rtf:{}:0", &fingerprint[..16]),
                title: staging.title.clone(),
                body_markdown: text,
                unsupported: vec!["RTF character and paragraph styling is not preserved".into()],
            });
            staging.unsupported.push(
                "RTF formatting, embedded objects, comments, and tracked changes require review"
                    .into(),
            );
        }
        SourceType::Epub => {
            let mut document = epub::doc::EpubDoc::new(path)
                .map_err(|error| ImportError::Archive(error.to_string()))?;
            if let Some(titles) = document.metadata.get("title")
                && let Some(title) = titles.first()
            {
                staging.title = title.clone();
            }
            let spine_ids = document.spine.clone();
            for (index, id) in spine_ids.iter().enumerate() {
                let chapter = document
                    .get_resource_str(id)
                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                let body = html_to_text(&chapter);
                if body.trim().is_empty() {
                    continue;
                }
                let chapter_title = document
                    .toc
                    .iter()
                    .find(|point| {
                        point.content
                            == document
                                .resources
                                .get(id)
                                .map(|(path, _)| path.to_string_lossy().into_owned())
                                .unwrap_or_default()
                    })
                    .map(|point| point.label.clone())
                    .unwrap_or_else(|| format!("Chapter {}", index + 1));
                staging.units.push(ManuscriptUnit {
                    source_document_id: format!("epub:{}:{index}", &fingerprint[..16]),
                    title: chapter_title,
                    body_markdown: body,
                    unsupported: vec![
                        "EPUB layout, CSS, embedded fonts, and page breaks are not preserved"
                            .into(),
                    ],
                });
            }
            staging.unsupported.push("EPUB styling, navigation landmarks, embedded media, and page layout require review".into());
        }
        _ => unreachable!(),
    }
    if staging.units.is_empty() {
        staging
            .warnings
            .push("No readable prose units were found".into());
    }
    staging.unsupported.sort();
    staging.unsupported.dedup();
    Ok(staging)
}

#[derive(Clone, Debug, Default)]
struct ScrivenerXmlNode {
    name: String,
    attributes: BTreeMap<String, String>,
    text: String,
    children: Vec<ScrivenerXmlNode>,
}

/// Scans a Scrivener `.scriv` directory or its `.scrivx` manifest without
/// writing to the source. The result is an inventory/preview; callers must not
/// advertise format compatibility until it has passed real-project fixtures.
pub fn inspect_scrivener_project(
    path: impl AsRef<Path>,
) -> Result<ScrivenerProjectInventory, ImportError> {
    let input = path.as_ref();
    let (project_root, manifest_path) = locate_scrivener_manifest(input)?;
    let fingerprint = fingerprint_source_tree(&project_root)?;
    let xml =
        fs::read_to_string(&manifest_path).map_err(|error| ImportError::Io(error.to_string()))?;
    let root = parse_scrivener_xml(&xml)?;
    let version = root.attributes.get("Version").cloned();
    let creator = root.attributes.get("Creator").cloned();
    let mut inventory = ScrivenerProjectInventory {
        source_path: project_root.to_string_lossy().into_owned(),
        source_fingerprint: fingerprint,
        title: project_root
            .file_stem()
            .or_else(|| project_root.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Scrivener Project")
            .to_owned(),
        format_version: version.clone(),
        creator,
        items: Vec::new(),
        warnings: Vec::new(),
        unsupported: Vec::new(),
    };

    if version.as_deref() != Some("2.0") {
        inventory.unsupported.push(format!(
            "Scrivener manifest revision {} has no inspected content adapter",
            version.as_deref().unwrap_or("(missing)")
        ));
    } else {
        inventory.warnings.push(format!(
            "Manifest revision 2.0 detected{}; this manifest revision does not establish Scrivener application version or platform compatibility",
            inventory
                .creator
                .as_deref()
                .map(|creator| format!(" (Creator={creator})"))
                .unwrap_or_default()
        ));
    }

    let binder = find_scrivener_node(&root, "Binder")
        .ok_or_else(|| ImportError::Archive("Scrivener manifest has no Binder element".into()))?;
    let top_level = scrivener_child_items(binder);
    let mut counts = BTreeMap::<Option<String>, u32>::new();
    for node in top_level {
        let parent_area = "unknown".to_owned();
        collect_scrivener_item(
            node,
            None,
            parent_area,
            &project_root,
            version.as_deref() == Some("2.0"),
            &mut counts,
            &mut inventory.items,
        )?;
    }

    inventory.validate()?;
    if inventory.items.is_empty() {
        inventory
            .warnings
            .push("Binder manifest contains no readable BinderItem records".into());
    }
    inventory.unsupported.sort();
    inventory.unsupported.dedup();
    Ok(inventory)
}

fn locate_scrivener_manifest(input: &Path) -> Result<(PathBuf, PathBuf), ImportError> {
    if input.is_file()
        && input
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("scrivx"))
    {
        let root = input
            .parent()
            .ok_or_else(|| ImportError::Io("manifest has no parent directory".into()))?;
        return Ok((root.to_path_buf(), input.to_path_buf()));
    }
    if !input.is_dir() {
        return Err(ImportError::UnsupportedFormat(
            input
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("unknown")
                .to_owned(),
        ));
    }
    let mut manifests = fs::read_dir(input)
        .map_err(|error| ImportError::Io(error.to_string()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let is_manifest = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("scrivx"));
            (is_manifest && entry.file_type().ok()?.is_file()).then_some(path)
        })
        .collect::<Vec<_>>();
    manifests.sort();
    match manifests.as_slice() {
        [manifest] => Ok((input.to_path_buf(), manifest.clone())),
        [] => Err(ImportError::Archive(
            "Scrivener project directory contains no .scrivx manifest".into(),
        )),
        _ => Err(ImportError::Archive(
            "Scrivener project directory contains multiple .scrivx manifests".into(),
        )),
    }
}

fn fingerprint_source_tree(root: &Path) -> Result<String, ImportError> {
    fn collect(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), ImportError> {
        let entries =
            fs::read_dir(directory).map_err(|error| ImportError::Io(error.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|error| ImportError::Io(error.to_string()))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| ImportError::Io(error.to_string()))?;
            if kind.is_symlink() {
                return Err(ImportError::Archive(format!(
                    "Scrivener source contains a symlink that cannot be fingerprinted safely: {}",
                    path.strip_prefix(root).unwrap_or(&path).display()
                )));
            }
            if kind.is_dir() {
                collect(root, &path, files)?;
            } else if kind.is_file() {
                files.push(path);
            }
        }
        Ok(())
    }

    let mut paths = Vec::new();
    collect(root, root, &mut paths)?;
    paths.sort_by(|left, right| {
        left.strip_prefix(root)
            .unwrap_or(left)
            .cmp(right.strip_prefix(root).unwrap_or(right))
    });
    let mut hasher = Sha256::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| ImportError::Io(error.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let contents = fs::read(&path).map_err(|error| ImportError::Io(error.to_string()))?;
        hasher.update((relative.len() as u64).to_be_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((contents.len() as u64).to_be_bytes());
        hasher.update(contents);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn parse_scrivener_xml(xml: &str) -> Result<ScrivenerXmlNode, ImportError> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<ScrivenerXmlNode>::new();
    let mut root = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let mut node = ScrivenerXmlNode {
                    name: String::from_utf8_lossy(event.local_name().as_ref()).into_owned(),
                    ..ScrivenerXmlNode::default()
                };
                for attribute in event.attributes().flatten() {
                    let key =
                        String::from_utf8_lossy(attribute.key.local_name().as_ref()).into_owned();
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|error| ImportError::Archive(error.to_string()))?;
                    node.attributes.insert(key, value.into_owned());
                }
                stack.push(node);
            }
            Ok(Event::Empty(event)) => {
                let mut node = ScrivenerXmlNode {
                    name: String::from_utf8_lossy(event.local_name().as_ref()).into_owned(),
                    ..ScrivenerXmlNode::default()
                };
                for attribute in event.attributes().flatten() {
                    let key =
                        String::from_utf8_lossy(attribute.key.local_name().as_ref()).into_owned();
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|error| ImportError::Archive(error.to_string()))?;
                    node.attributes.insert(key, value.into_owned());
                }
                attach_scrivener_node(node, &mut stack, &mut root);
            }
            Ok(Event::Text(event)) => {
                if let Some(node) = stack.last_mut() {
                    let decoded = event
                        .xml_content(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|error| ImportError::Archive(error.to_string()))?;
                    node.text.push_str(
                        &quick_xml::escape::unescape(&decoded)
                            .map_err(|error| ImportError::Archive(error.to_string()))?,
                    );
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(
                        &event
                            .decode()
                            .map_err(|error| ImportError::Archive(error.to_string()))?,
                    );
                }
            }
            Ok(Event::End(_)) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| ImportError::Archive("unbalanced XML end element".into()))?;
                attach_scrivener_node(node, &mut stack, &mut root);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ImportError::Archive(error.to_string())),
        }
    }
    if !stack.is_empty() {
        return Err(ImportError::Archive(
            "Scrivener manifest ended with unclosed XML elements".into(),
        ));
    }
    root.ok_or_else(|| ImportError::Archive("Scrivener manifest is empty".into()))
}

fn attach_scrivener_node(
    node: ScrivenerXmlNode,
    stack: &mut [ScrivenerXmlNode],
    root: &mut Option<ScrivenerXmlNode>,
) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.is_none() {
        *root = Some(node);
    }
}

fn find_scrivener_node<'a>(node: &'a ScrivenerXmlNode, name: &str) -> Option<&'a ScrivenerXmlNode> {
    if node.name.eq_ignore_ascii_case(name) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_scrivener_node(child, name))
}

fn scrivener_child_items(node: &ScrivenerXmlNode) -> Vec<&ScrivenerXmlNode> {
    let container = node
        .children
        .iter()
        .find(|child| child.name.eq_ignore_ascii_case("Children"));
    container.map_or_else(
        || {
            node.children
                .iter()
                .filter(|child| child.name.eq_ignore_ascii_case("BinderItem"))
                .collect()
        },
        |children| {
            children
                .children
                .iter()
                .filter(|child| child.name.eq_ignore_ascii_case("BinderItem"))
                .collect()
        },
    )
}

fn scrivener_field_values(node: &ScrivenerXmlNode, values: &mut BTreeMap<String, String>) {
    for child in &node.children {
        if child.children.is_empty() && !child.text.trim().is_empty() {
            values.insert(child.name.clone(), child.text.trim().to_owned());
        } else {
            scrivener_field_values(child, values);
        }
    }
}

fn scrivener_named_value(node: &ScrivenerXmlNode, name: &str) -> Option<String> {
    node.children
        .iter()
        .find(|child| child.name.eq_ignore_ascii_case(name))
        .map(|child| child.text.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn classify_scrivener_area(title: &str, type_code: Option<&str>, inherited: &str) -> String {
    let label = format!(
        "{} {}",
        title.to_ascii_lowercase(),
        type_code.unwrap_or_default()
    )
    .to_ascii_lowercase();
    if label.contains("research") {
        "research".into()
    } else if label.contains("trash") || label.contains("recycle") {
        "trash".into()
    } else if label.contains("draft") || label.contains("manuscript") {
        "manuscript".into()
    } else {
        inherited.to_owned()
    }
}

fn collect_scrivener_item(
    node: &ScrivenerXmlNode,
    parent_source_id: Option<String>,
    parent_area: String,
    project_root: &Path,
    read_candidate_content: bool,
    counts: &mut BTreeMap<Option<String>, u32>,
    output: &mut Vec<ScrivenerBinderItem>,
) -> Result<(), ImportError> {
    let source_document_id = node
        .attributes
        .get("UUID")
        .cloned()
        .or_else(|| node.attributes.get("ID").cloned())
        .or_else(|| scrivener_named_value(node, "UUID"))
        .ok_or_else(|| ImportError::Archive("BinderItem is missing its UUID/ID".into()))?;
    let position = counts.entry(parent_source_id.clone()).or_default();
    let this_position = *position;
    *position = position.saturating_add(1);
    let title = scrivener_named_value(node, "Title").unwrap_or_default();
    let type_code = node
        .attributes
        .get("Type")
        .cloned()
        .or_else(|| scrivener_named_value(node, "Type"));
    let area = classify_scrivener_area(&title, type_code.as_deref(), &parent_area);
    let mut fields = BTreeMap::new();
    scrivener_field_values(node, &mut fields);
    let include_in_compile = fields
        .iter()
        .find(|(key, _)| {
            key.eq_ignore_ascii_case("IncludeInCompile")
                || key.eq_ignore_ascii_case("IncludeInCompilation")
        })
        .map(|(_, value)| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "yes" | "true" | "1" | "include"
            )
        });
    let mut unsupported = Vec::new();
    let mut synopsis = None;
    let mut notes_markdown = None;
    let mut content_markdown = None;
    if read_candidate_content {
        if let Some(data_dir) =
            find_case_insensitive_child(&project_root.join("Files"), "Data", true)?
            && let Some(item_dir) =
                find_case_insensitive_child(&data_dir, &source_document_id, true)?
        {
            if let Some(file) = find_case_insensitive_child(&item_dir, "content.rtf", false)? {
                let bytes = fs::read(&file).map_err(|error| ImportError::Io(error.to_string()))?;
                let rtf = String::from_utf8_lossy(&bytes);
                content_markdown = Some(extract_rtf_markdown(&rtf));
                unsupported.push(
                    "Scrivener RTF font, colour, layout and embedded-object fidelity needs review"
                        .into(),
                );
            }
            if let Some(file) = find_case_insensitive_child(&item_dir, "synopsis.txt", false)? {
                synopsis = Some(
                    fs::read_to_string(file).map_err(|error| ImportError::Io(error.to_string()))?,
                );
            }
            if let Some(file) = find_case_insensitive_child(&item_dir, "notes.rtf", false)? {
                let bytes = fs::read(file).map_err(|error| ImportError::Io(error.to_string()))?;
                notes_markdown = Some(extract_rtf_markdown(&String::from_utf8_lossy(&bytes)));
            }
        }
        if content_markdown.is_none()
            && area == "manuscript"
            && !scrivener_child_items(node).is_empty()
        {
            unsupported.push(
                "Container item has no readable content.rtf; folder-group prose may be stored separately".into(),
            );
        }
    }
    output.push(ScrivenerBinderItem {
        source_document_id: source_document_id.clone(),
        parent_source_id: parent_source_id.clone(),
        position: this_position,
        title,
        type_code,
        area: area.clone(),
        include_in_compile,
        synopsis,
        notes_markdown,
        content_markdown,
        fields,
        unsupported,
    });
    for child in scrivener_child_items(node) {
        collect_scrivener_item(
            child,
            Some(source_document_id.clone()),
            area.clone(),
            project_root,
            read_candidate_content,
            counts,
            output,
        )?;
    }
    Ok(())
}

fn find_case_insensitive_child(
    directory: &Path,
    name: &str,
    want_directory: bool,
) -> Result<Option<PathBuf>, ImportError> {
    if !directory.is_dir() {
        return Ok(None);
    }
    let entries = fs::read_dir(directory).map_err(|error| ImportError::Io(error.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|error| ImportError::Io(error.to_string()))?;
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(name)
        {
            let kind = entry
                .file_type()
                .map_err(|error| ImportError::Io(error.to_string()))?;
            if kind.is_symlink() {
                return Err(ImportError::Archive(format!(
                    "Scrivener content path is a symlink: {}",
                    entry.path().display()
                )));
            }
            if (want_directory && kind.is_dir()) || (!want_directory && kind.is_file()) {
                return Ok(Some(entry.path()));
            }
        }
    }
    Ok(None)
}

fn read_zip_text(bytes: &[u8], member: &str) -> Result<String, ImportError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| ImportError::Archive(error.to_string()))?;
    let mut file = archive
        .by_name(member)
        .map_err(|error| ImportError::Archive(error.to_string()))?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text)
        .map_err(|error| ImportError::Archive(error.to_string()))?;
    Ok(text)
}

fn docx_toggle_enabled(
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) -> Result<bool, ImportError> {
    for attribute in event.attributes().flatten() {
        if attribute.key.local_name().as_ref() == b"val" {
            let value = attribute
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder)
                .map_err(|error| ImportError::Archive(error.to_string()))?;
            return Ok(!matches!(value.as_ref(), "0" | "false" | "off"));
        }
    }
    Ok(true)
}

fn docx_inline_markdown(text: &str, bold: bool, italic: bool, strike: bool) -> String {
    if !(bold || italic || strike) {
        return text.into();
    }
    let escaped = text
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('~', "\\~")
        .replace('`', "\\`");
    let styled = match (bold, italic) {
        (true, true) => format!("***{escaped}***"),
        (true, false) => format!("**{escaped}**"),
        (false, true) => format!("*{escaped}*"),
        (false, false) => escaped,
    };
    if strike {
        format!("~~{styled}~~")
    } else {
        styled
    }
}

fn flush_docx_run(
    paragraph: &mut String,
    run_text: &mut String,
    bold: bool,
    italic: bool,
    strike: bool,
) {
    paragraph.push_str(&docx_inline_markdown(run_text, bold, italic, strike));
    run_text.clear();
}

fn extract_docx_blocks(xml: &str) -> Result<Vec<(String, bool)>, ImportError> {
    let mut blocks = Vec::new();
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut in_paragraph = false;
    let mut is_heading = false;
    let mut paragraph = String::new();
    let mut run_text = String::new();
    let (mut bold, mut italic, mut strike) = (false, false, false);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event)) => match event.local_name().as_ref() {
                b"r" => {
                    flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                    (bold, italic, strike) = (false, false, false);
                }
                b"p" => {
                    in_paragraph = true;
                    is_heading = false;
                    paragraph.clear();
                    run_text.clear();
                }
                b"pStyle" if in_paragraph => {
                    for attribute in event.attributes().flatten() {
                        if attribute.key.local_name().as_ref() == b"val" {
                            let style = attribute
                                .decoded_and_normalized_value(
                                    quick_xml::XmlVersion::Implicit1_0,
                                    reader.decoder(),
                                )
                                .map_err(|error| ImportError::Archive(error.to_string()))?;
                            is_heading = style.to_ascii_lowercase().contains("heading")
                                || style.eq_ignore_ascii_case("title");
                        }
                    }
                }
                b"b" if in_paragraph => bold = docx_toggle_enabled(&event, reader.decoder())?,
                b"i" if in_paragraph => italic = docx_toggle_enabled(&event, reader.decoder())?,
                b"strike" | b"dstrike" if in_paragraph => {
                    strike = docx_toggle_enabled(&event, reader.decoder())?
                }
                b"tab" if in_paragraph => {
                    flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                    paragraph.push('\t');
                }
                b"br" | b"cr" if in_paragraph => {
                    flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                    paragraph.push('\n');
                }
                _ => {}
            },
            Ok(quick_xml::events::Event::Empty(event)) if in_paragraph => {
                match event.local_name().as_ref() {
                    b"b" => bold = docx_toggle_enabled(&event, reader.decoder())?,
                    b"i" => italic = docx_toggle_enabled(&event, reader.decoder())?,
                    b"strike" | b"dstrike" => {
                        strike = docx_toggle_enabled(&event, reader.decoder())?
                    }
                    b"tab" => {
                        flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                        paragraph.push('\t');
                    }
                    b"br" | b"cr" => {
                        flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                        paragraph.push('\n');
                    }
                    b"pStyle" => {
                        for attribute in event.attributes().flatten() {
                            if attribute.key.local_name().as_ref() == b"val" {
                                let style = attribute
                                    .decoded_and_normalized_value(
                                        quick_xml::XmlVersion::Implicit1_0,
                                        reader.decoder(),
                                    )
                                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                                is_heading = style.to_ascii_lowercase().contains("heading")
                                    || style.eq_ignore_ascii_case("title");
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Text(event)) if in_paragraph => {
                let decoded = event
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                run_text.push_str(&decoded);
            }
            Ok(quick_xml::events::Event::GeneralRef(event)) if in_paragraph => {
                let reference = event
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                if let Some(character) = event
                    .resolve_char_ref()
                    .map_err(|error| ImportError::Archive(error.to_string()))?
                {
                    run_text.push(character);
                } else {
                    run_text.push_str(match reference.as_ref() {
                        "amp" => "&",
                        "lt" => "<",
                        "gt" => ">",
                        "quot" => "\"",
                        "apos" => "'",
                        unknown => {
                            return Err(ImportError::Archive(format!(
                                "unsupported XML entity reference: &{unknown};"
                            )));
                        }
                    });
                }
            }
            Ok(quick_xml::events::Event::End(event)) if event.local_name().as_ref() == b"r" => {
                flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                (bold, italic, strike) = (false, false, false);
            }
            Ok(quick_xml::events::Event::End(event)) if event.local_name().as_ref() == b"p" => {
                flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                if !paragraph.trim().is_empty() {
                    blocks.push((paragraph.clone(), is_heading));
                }
                in_paragraph = false;
                bold = false;
                italic = false;
                strike = false;
            }
            Ok(quick_xml::events::Event::Eof) if in_paragraph => {
                return Err(ImportError::Archive(
                    "DOCX document ends inside a paragraph".into(),
                ));
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ImportError::Archive(error.to_string())),
        }
    }
    Ok(blocks)
}

fn extract_odt_blocks(xml: &str) -> Vec<(String, bool)> {
    let mut blocks = Vec::new();
    let mut rest = xml;
    loop {
        let paragraph = rest.find("<text:p").map(|index| (index, "text:p", false));
        let heading = rest.find("<text:h").map(|index| (index, "text:h", true));
        let next = match (paragraph, heading) {
            (Some(a), Some(b)) => {
                if a.0 < b.0 {
                    Some(a)
                } else {
                    Some(b)
                }
            }
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some((start, tag, is_heading)) = next else {
            break;
        };
        rest = &rest[start..];
        let Some(open_end) = rest.find('>') else {
            break;
        };
        let closing = format!("</{tag}>");
        let Some(end) = rest[open_end + 1..].find(&closing) else {
            break;
        };
        let text = xml_markup_to_text(&rest[open_end + 1..open_end + 1 + end]);
        if !text.trim().is_empty() {
            blocks.push((text, is_heading));
        }
        rest = &rest[open_end + 1 + end + closing.len()..];
    }
    blocks
}

fn paragraph_xml_to_text(xml: &str, text_tag: &str) -> String {
    let mut result = String::new();
    let open = format!("<{text_tag}");
    let close = format!("</{text_tag}>");
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        rest = &rest[start..];
        let Some(content_start) = rest.find('>').map(|index| index + 1) else {
            break;
        };
        let Some(end) = rest[content_start..].find(&close) else {
            break;
        };
        result.push_str(&decode_xml(&rest[content_start..content_start + end]));
        rest = &rest[content_start + end + close.len()..];
    }
    result
}

fn decode_xml(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn xml_markup_to_text(xml: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in xml.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    decode_xml(&text)
}

fn split_blocks_into_units(
    staging: &mut ManuscriptStaging,
    blocks: Vec<(String, bool)>,
    prefix: &str,
) {
    let mut title = staging.title.clone();
    let mut body = Vec::new();
    let mut index = 0;
    for (paragraph, heading) in blocks {
        if heading && !body.is_empty() {
            staging.units.push(ManuscriptUnit {
                source_document_id: format!(
                    "{prefix}:{}:{index}",
                    &staging.source_fingerprint[..16]
                ),
                title: title.clone(),
                body_markdown: body.join("\n\n"),
                unsupported: Vec::new(),
            });
            index += 1;
            body.clear();
        }
        if heading {
            title = paragraph;
        } else {
            body.push(paragraph);
        }
    }
    if !body.is_empty() || staging.units.is_empty() {
        staging.units.push(ManuscriptUnit {
            source_document_id: format!("{prefix}:{}:{index}", &staging.source_fingerprint[..16]),
            title,
            body_markdown: body.join("\n\n"),
            unsupported: Vec::new(),
        });
    }
}

fn html_to_text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => {
                in_tag = true;
                if !text.ends_with('\n') {
                    text.push('\n');
                }
            }
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    decode_xml(&text)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn extract_rtf_text(source: &str) -> String {
    let mut output = String::new();
    let mut chars = source.chars().peekable();
    let mut skip_depth = 0usize;
    let mut groups = Vec::new();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                groups.push(skip_depth);
                if chars.peek() == Some(&'\\') {
                    let mut probe = chars.clone();
                    probe.next();
                    if probe.peek() == Some(&'*') {
                        skip_depth += 1;
                    }
                }
            }
            '}' => {
                skip_depth = groups.pop().unwrap_or(0);
            }
            '\\' => {
                let Some(next) = chars.next() else {
                    break;
                };
                if matches!(next, '\n' | '\r') {
                    if skip_depth == 0 {
                        output.push('\n');
                    }
                    if next == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    continue;
                }
                if next == '\\' || next == '{' || next == '}' {
                    if skip_depth == 0 {
                        output.push(next);
                    }
                    continue;
                }
                if next == '\'' {
                    let a = chars.next().and_then(|c| c.to_digit(16));
                    let b = chars.next().and_then(|c| c.to_digit(16));
                    if skip_depth == 0
                        && let (Some(a), Some(b)) = (a, b)
                    {
                        output.push(char::from_u32(a * 16 + b).unwrap_or('�'));
                    }
                    continue;
                }
                if !next.is_ascii_alphabetic() {
                    if next == '~' && skip_depth == 0 {
                        output.push(' ');
                    }
                    continue;
                }
                let mut word = String::from(next);
                while chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    word.push(chars.next().unwrap());
                }
                let mut number = String::new();
                if chars
                    .peek()
                    .is_some_and(|c| *c == '-' || c.is_ascii_digit())
                {
                    if chars.peek() == Some(&'-') {
                        number.push(chars.next().unwrap());
                    }
                    while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                        number.push(chars.next().unwrap());
                    }
                }
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
                if matches!(
                    word.as_str(),
                    "fonttbl"
                        | "colortbl"
                        | "stylesheet"
                        | "info"
                        | "pict"
                        | "object"
                        | "header"
                        | "footer"
                        | "xmlnstbl"
                        | "listtable"
                        | "listoverridetable"
                ) {
                    skip_depth += 1;
                }
                if skip_depth == 0 {
                    match word.as_str() {
                        "par" | "line" => output.push('\n'),
                        "tab" => output.push('\t'),
                        "u" => {
                            if let Ok(code) = number.parse::<i32>() {
                                output.push(char::from_u32(code as u16 as u32).unwrap_or('�'));
                                let _ = chars.next();
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ if skip_depth == 0 && !ch.is_control() => output.push(ch),
            _ => {}
        }
    }
    output
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Converts the subset of RTF inline formatting supported by the importer to
/// Markdown. Unsupported document-level formatting is reported by the caller.
/// This intentionally preserves bold, italic and strike runs without trying
/// to reproduce page layout or embedded objects.
fn extract_rtf_markdown(source: &str) -> String {
    #[derive(Clone, Copy, Default, Eq, PartialEq)]
    struct Style {
        bold: bool,
        italic: bool,
        strike: bool,
        skip: bool,
    }

    let mut chars = source.chars().peekable();
    let mut style = Style::default();
    let mut stack = Vec::new();
    let mut runs = Vec::<(char, Style)>::new();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                stack.push(style);
                if chars.peek() == Some(&'\\') {
                    let mut probe = chars.clone();
                    probe.next();
                    if probe.peek() == Some(&'*') {
                        style.skip = true;
                    }
                }
            }
            '}' => style = stack.pop().unwrap_or_default(),
            '\\' => {
                let Some(next) = chars.next() else { break };
                if matches!(next, '\n' | '\r') {
                    if !style.skip {
                        runs.push(('\n', Style::default()));
                    }
                    if next == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    continue;
                }
                if matches!(next, '\\' | '{' | '}') {
                    if !style.skip {
                        runs.push((next, style));
                    }
                    continue;
                }
                if next == '\'' {
                    let a = chars.next().and_then(|c| c.to_digit(16));
                    let b = chars.next().and_then(|c| c.to_digit(16));
                    if !style.skip
                        && let (Some(a), Some(b)) = (a, b)
                    {
                        runs.push((char::from_u32(a * 16 + b).unwrap_or('�'), style));
                    }
                    continue;
                }
                if !next.is_ascii_alphabetic() {
                    if !style.skip && next == '~' {
                        runs.push((' ', style));
                    }
                    continue;
                }
                let mut word = String::from(next);
                while chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    word.push(chars.next().unwrap());
                }
                let mut number = String::new();
                if chars
                    .peek()
                    .is_some_and(|c| *c == '-' || c.is_ascii_digit())
                {
                    if chars.peek() == Some(&'-') {
                        number.push(chars.next().unwrap());
                    }
                    while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                        number.push(chars.next().unwrap());
                    }
                }
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
                match word.as_str() {
                    "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" | "object"
                    | "header" | "footer" | "xmlnstbl" | "listtable" | "listoverridetable" => {
                        style.skip = true
                    }
                    "b" => style.bold = number != "0",
                    "i" => style.italic = number != "0",
                    "strike" | "strikethrough" => style.strike = number != "0",
                    "par" | "line" if !style.skip => runs.push(('\n', Style::default())),
                    "tab" if !style.skip => runs.push(('\t', Style::default())),
                    "u" if !style.skip => {
                        if let Ok(code) = number.parse::<i32>() {
                            runs.push((char::from_u32(code as u16 as u32).unwrap_or('�'), style));
                            // RTF Unicode escapes may be followed by one ANSI fallback char.
                            let _ = chars.next();
                        }
                    }
                    _ => {}
                }
            }
            _ if !style.skip && !ch.is_control() => runs.push((ch, style)),
            _ => {}
        }
    }

    let mut output = String::new();
    let mut current = Style::default();
    let mut text = String::new();
    let flush = |output: &mut String, text: &mut String, style: Style| {
        if text.is_empty() {
            return;
        }
        let escaped = text
            .replace('\\', "\\\\")
            .replace('*', "\\*")
            .replace('_', "\\_");
        let value = match (style.bold, style.italic, style.strike) {
            (true, true, true) => format!("~~***{escaped}***~~"),
            (true, true, false) => format!("***{escaped}***"),
            (true, false, true) => format!("~~**{escaped}**~~"),
            (false, true, true) => format!("~~*{escaped}*~~"),
            (true, false, false) => format!("**{escaped}**"),
            (false, true, false) => format!("*{escaped}*"),
            (false, false, true) => format!("~~{escaped}~~"),
            _ => escaped,
        };
        output.push_str(&value);
        text.clear();
    };
    for (ch, next_style) in runs {
        if ch == '\n' {
            flush(&mut output, &mut text, current);
            while output.ends_with(' ') {
                output.pop();
            }
            if !output.ends_with("\n\n") {
                output.push_str("\n\n");
            }
            current = Style::default();
        } else if next_style != current {
            flush(&mut output, &mut text, current);
            current = next_style;
            text.push(ch);
        } else {
            text.push(ch);
        }
    }
    flush(&mut output, &mut text, current);
    output.trim().to_owned()
}

impl ImportInventory {
    /// Validates identity and hierarchy before any destination project is mutated.
    pub fn validate(&self) -> Result<(), ImportError> {
        let mut ids = BTreeSet::new();
        for item in &self.items {
            if !ids.insert(&item.source_document_id) {
                return Err(ImportError::DuplicateSourceId(
                    item.source_document_id.clone(),
                ));
            }
        }
        for item in &self.items {
            if let Some(parent) = &item.parent_source_id
                && !ids.contains(parent)
            {
                return Err(ImportError::MissingParent(parent.clone()));
            }
        }
        Ok(())
    }
    pub fn apply_mapping_plan(&self, plan: &MappingPlan) -> Result<usize, ImportError> {
        self.validate()?;
        let known: BTreeSet<&str> = self
            .items
            .iter()
            .flat_map(|i| i.fields.keys().map(String::as_str))
            .collect();
        for m in &plan.fields {
            if !known.contains(m.source_field.as_str()) {
                return Err(ImportError::MissingField(m.source_field.clone()));
            }
        }
        Ok(plan.fields.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    fn synthetic_fixture() -> ImportInventory {
        let bytes = b"synthetic source inventory fixture";
        ImportInventory {
            source_type: SourceType::Spreadsheet,
            source_identifier: "fixture.ods".into(),
            source_fingerprint: format!("{:x}", Sha256::digest(bytes)),
            items: vec![
                InventoryItem {
                    source_document_id: "Arc:row:2".into(),
                    item_type: "scene".into(),
                    parent_source_id: None,
                    source_sheet: Some("Arc".into()),
                    source_row: Some(2),
                    fields: BTreeMap::from([
                        ("Title".into(), "Opening".into()),
                        ("POV".into(), "Mara".into()),
                    ]),
                    unsupported: vec![],
                },
                InventoryItem {
                    source_document_id: "Arc:row:3".into(),
                    item_type: "scene".into(),
                    parent_source_id: Some("Arc:row:2".into()),
                    source_sheet: Some("Arc".into()),
                    source_row: Some(3),
                    fields: BTreeMap::from([("Title".into(), "Consequence".into())]),
                    unsupported: vec!["embedded chart styling".into()],
                },
            ],
            warnings: vec![],
        }
    }
    #[test]
    fn synthetic_inventory_and_mapping_are_validated() {
        let inv = synthetic_fixture();
        inv.validate().unwrap();
        let plan = MappingPlan {
            fields: vec![FieldMapping {
                source_field: "Title".into(),
                destination_entity: "scene".into(),
                destination_field: "title".into(),
                conversion: None,
            }],
            unresolved: vec!["POV".into()],
            decisions: BTreeMap::new(),
        };
        assert_eq!(inv.apply_mapping_plan(&plan).unwrap(), 1);
    }
    #[test]
    fn duplicate_source_items_are_rejected() {
        let mut inv = synthetic_fixture();
        inv.items[1].source_document_id = inv.items[0].source_document_id.clone();
        assert!(matches!(
            inv.validate(),
            Err(ImportError::DuplicateSourceId(_))
        ));
    }

    fn archive(path: &Path, member: &str, contents: &str) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(member, zip::write::FileOptions::default())
            .unwrap();
        zip.write_all(contents.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    fn archive_with_members(path: &Path, members: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (member, contents) in members {
            zip.start_file(*member, zip::write::FileOptions::default())
                .unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    fn synthetic_epub(path: &Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "mimetype",
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(b"application/epub+zip").unwrap();
        for (member, body) in [
            (
                "META-INF/container.xml",
                "<container><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>",
            ),
            (
                "OEBPS/content.opf",
                "<package xmlns:dc=\"http://purl.org/dc/elements/1.1/\" version=\"2.0\"><metadata><dc:title>Fixture Book</dc:title><dc:language>en</dc:language></metadata><manifest><item id=\"chapter\" href=\"chapter.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"chapter\"/></spine></package>",
            ),
            (
                "OEBPS/chapter.xhtml",
                "<html><body><h1>Opening</h1><p>EPUB prose.</p></body></html>",
            ),
        ] {
            zip.start_file(member, zip::write::FileOptions::default())
                .unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn docx_and_odt_staging_extract_prose_and_leave_source_unchanged() {
        let directory = tempfile::tempdir().unwrap();
        let docx = directory.path().join("draft.docx");
        archive(
            &docx,
            "word/document.xml",
            "<w:document><w:body><w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>Chapter One</w:t></w:r></w:p><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Élodie &amp; Sam.</w:t><w:tab/><w:t>Still here.</w:t><w:br/><w:t>New line.</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>italic text</w:t></w:r><w:r><w:rPr><w:strike/></w:rPr><w:t>struck text</w:t></w:r><w:r><w:rPr><w:u w:val=\"single\"/></w:rPr><w:t>underlined text</w:t></w:r></w:p></w:body></w:document>",
        );
        let source_before = std::fs::read(&docx).unwrap();
        let staged = inspect_manuscript(&docx).unwrap();
        assert_eq!(staged.source_type, SourceType::Docx);
        assert_eq!(staged.units.len(), 1);
        assert_eq!(staged.units[0].title, "Chapter One");
        assert!(
            staged.units[0].body_markdown.contains("**Élodie & Sam.**"),
            "{}",
            staged.units[0].body_markdown
        );
        assert!(
            staged.units[0]
                .body_markdown
                .contains("\t**Still here.**\n**New line.**")
        );
        assert!(staged.units[0].body_markdown.contains("*italic text*"));
        assert!(staged.units[0].body_markdown.contains("~~struck text~~"));
        assert!(
            staged
                .unsupported
                .iter()
                .any(|item| item.contains("underline formatting"))
        );
        assert_eq!(
            staged.source_fingerprint,
            format!("{:x}", Sha256::digest(&source_before))
        );
        assert_eq!(std::fs::read(&docx).unwrap(), source_before);

        let odt = directory.path().join("notes.odt");
        archive(
            &odt,
            "content.xml",
            "<office:document><text:h>Part One</text:h><text:p>First &amp; foremost.</text:p></office:document>",
        );
        let staged = inspect_manuscript(&odt).unwrap();
        assert_eq!(staged.units.len(), 1);
        assert_eq!(staged.units[0].title, "Part One");
        assert!(staged.units[0].body_markdown.contains("First & foremost."));
    }

    #[test]
    fn malformed_docx_xml_is_reported_instead_of_partially_imported() {
        let directory = tempfile::tempdir().unwrap();
        let docx = directory.path().join("malformed.docx");
        archive(
            &docx,
            "word/document.xml",
            "<w:document><w:p><w:t>unfinished",
        );
        assert!(matches!(
            inspect_manuscript(&docx),
            Err(ImportError::Archive(_))
        ));
    }

    #[test]
    fn rtf_staging_decodes_basic_controls_and_reports_formatting_loss() {
        let directory = tempfile::tempdir().unwrap();
        let rtf = directory.path().join("draft.rtf");
        std::fs::write(
            &rtf,
            r"{\rtf1\ansi{\fonttbl{\f0 Times;}}Hello\par \u8212? world\par {\b bold} text}",
        )
        .unwrap();
        let staged = inspect_manuscript(&rtf).unwrap();
        assert!(staged.units[0].body_markdown.contains("Hello"));
        assert!(staged.units[0].body_markdown.contains("— world"));
        assert!(!staged.units[0].body_markdown.contains("Times"));
        assert!(!staged.unsupported.is_empty());
    }

    #[test]
    fn scrivener_rtf_physical_line_boundary_does_not_join_adjacent_words() {
        let source = r#"{\rtf1\ansi
Test\

This is an addition to the old file.}"#;
        let markdown = extract_rtf_markdown(source);
        assert_eq!(markdown, "Test\n\nThis is an addition to the old file.");
        assert_eq!(extract_rtf_text(source), markdown);
    }

    #[test]
    fn synthetic_scrivener_layout_inventory_preserves_hierarchy_and_source() {
        // Synthetic parser fixture only. This exercises the inventory contract
        // and is not evidence of compatibility with real Scrivener projects.
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Synthetic.scriv");
        let manifest_dir = project.join("Files");
        let data_dir = manifest_dir.join("Data");
        let document_dir = data_dir.join("scene-uuid");
        std::fs::create_dir_all(&document_dir).unwrap();
        let manifest = r#"<?xml version="1.0" encoding="UTF-8"?>
            <ScrivenerProject Version="2.0"><Binder>
              <BinderItem UUID="chapter-uuid" Type="Folder"><Title>Chapter One</Title><Children>
                <BinderItem UUID="scene-uuid" Type="Text"><Title>Opening</Title><IncludeInCompile>Yes</IncludeInCompile><Children/></BinderItem>
              </Children></BinderItem>
              <BinderItem UUID="research-uuid" Type="Folder"><Title>Research</Title><Children/></BinderItem>
            </Binder></ScrivenerProject>"#;
        std::fs::write(project.join("Synthetic.scrivx"), manifest).unwrap();
        std::fs::write(
            document_dir.join("content.rtf"),
            r"{\rtf1\ansi Hello {\i italic} world\par \u8212? continuation}",
        )
        .unwrap();
        std::fs::write(document_dir.join("synopsis.txt"), "A test synopsis").unwrap();
        let before_manifest = std::fs::read(project.join("Synthetic.scrivx")).unwrap();
        let before_content = std::fs::read(document_dir.join("content.rtf")).unwrap();

        let inventory = inspect_scrivener_project(&project).unwrap();
        inventory.validate().unwrap();
        assert_eq!(inventory.items.len(), 3);
        assert_eq!(
            inventory.items[1].parent_source_id.as_deref(),
            Some("chapter-uuid")
        );
        assert_eq!(inventory.items[1].position, 0);
        assert_eq!(inventory.items[1].include_in_compile, Some(true));
        assert_eq!(inventory.items[1].synopsis, Some("A test synopsis".into()));
        let prose = inventory.items[1].content_markdown.as_deref().unwrap();
        assert!(prose.contains("*italic*"), "{prose}");
        assert!(prose.contains("— continuation"), "{prose}");
        assert_eq!(inventory.items[2].area, "research");
        assert_eq!(
            std::fs::read(project.join("Synthetic.scrivx")).unwrap(),
            before_manifest
        );
        assert_eq!(
            std::fs::read(document_dir.join("content.rtf")).unwrap(),
            before_content
        );
        assert!(
            inventory
                .warnings
                .iter()
                .any(|warning| warning.contains("does not establish Scrivener application version"))
        );

        let mut plan = ScrivenerImportPlan::proposed(&inventory);
        let preview = plan.preview(&inventory).unwrap();
        assert_eq!(preview.total_items, 3);
        assert_eq!(preview.research_items, 1);
        assert!(
            preview
                .unresolved_source_ids
                .contains(&"chapter-uuid".into())
        );
        assert!(!preview.unresolved_source_ids.contains(&"scene-uuid".into()));
        plan.decisions
            .iter_mut()
            .find(|decision| decision.source_document_id == "chapter-uuid")
            .unwrap()
            .destination_kind = ScrivenerDestinationKind::Chapter;
        let preview = plan.preview(&inventory).unwrap();
        assert!(preview.unresolved_source_ids.is_empty());
        plan.validate_for_import(&inventory).unwrap();
    }

    #[test]
    fn scrivener_inventory_rejects_malformed_and_duplicate_binder_items() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Broken.scriv");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("Broken.scrivx"), "<broken").unwrap();
        assert!(matches!(
            inspect_scrivener_project(&project),
            Err(ImportError::Archive(_))
        ));

        let duplicate = ScrivenerProjectInventory {
            source_path: String::new(),
            source_fingerprint: String::new(),
            title: String::new(),
            format_version: None,
            creator: None,
            items: vec![
                ScrivenerBinderItem {
                    source_document_id: "same".into(),
                    parent_source_id: None,
                    position: 0,
                    title: String::new(),
                    type_code: None,
                    area: "unknown".into(),
                    include_in_compile: None,
                    synopsis: None,
                    notes_markdown: None,
                    content_markdown: None,
                    fields: BTreeMap::new(),
                    unsupported: Vec::new(),
                },
                ScrivenerBinderItem {
                    source_document_id: "same".into(),
                    parent_source_id: None,
                    position: 1,
                    title: String::new(),
                    type_code: None,
                    area: "unknown".into(),
                    include_in_compile: None,
                    synopsis: None,
                    notes_markdown: None,
                    content_markdown: None,
                    fields: BTreeMap::new(),
                    unsupported: Vec::new(),
                },
            ],
            warnings: Vec::new(),
            unsupported: Vec::new(),
        };
        assert!(matches!(
            duplicate.validate(),
            Err(ImportError::DuplicateSourceId(_))
        ));
    }

    #[test]
    fn three_way_merge_combines_disjoint_edits_and_reports_overlapping_changes() {
        let base = "One.\nTwo.\nThree.\n";
        let local = "One revised.\nTwo.\nThree.\n";
        let external = "One.\nTwo.\nThree revised.\n";
        let merged = three_way_merge(base, local, external);
        assert!(merged.conflicts.is_empty());
        assert_eq!(
            merged.merged_text.as_deref(),
            Some("One revised.\nTwo.\nThree revised.\n")
        );

        let conflict = three_way_merge(
            base,
            "One local.\nTwo.\nThree.\n",
            "One external.\nTwo.\nThree.\n",
        );
        assert!(conflict.merged_text.is_none());
        assert_eq!(conflict.conflicts.len(), 1);
        assert_eq!(conflict.conflicts[0].base, "One.\n");
        assert_eq!(conflict.conflicts[0].local, "One local.\n");
        assert_eq!(conflict.conflicts[0].external, "One external.\n");
    }

    #[test]
    fn conflict_resolution_preserves_disjoint_changes_and_requires_one_choice_per_hunk() {
        let base = "One.\nTwo.\nThree.\nFour.\nFive.\nSix.\nSeven.\nEight.\n";
        let local = "One local.\nTwo.\nThree.\nFour.\nFive.\nSix.\nSeven.\nEight local.\n";
        let external = "One external.\nTwo.\nThree.\nFour.\nFive external.\nSix.\nSeven.\nEight.\n";
        let conflicts = three_way_merge(base, local, external).conflicts;
        assert_eq!(conflicts.len(), 1);
        assert!(resolve_three_way_merge(base, local, external, &[]).is_err());
        let merged =
            resolve_three_way_merge(base, local, external, &[MergeConflictChoice::External])
                .unwrap();
        assert_eq!(
            merged,
            "One external.\nTwo.\nThree.\nFour.\nFive external.\nSix.\nSeven.\nEight local.\n"
        );
        assert!(
            resolve_three_way_merge(
                base,
                local,
                external,
                &[MergeConflictChoice::Base, MergeConflictChoice::Local]
            )
            .is_err()
        );
    }

    #[test]
    fn docx_review_inventory_preserves_authors_and_comment_ids() {
        let directory = tempfile::tempdir().unwrap();
        let docx = directory.path().join("review.docx");
        archive_with_members(
            &docx,
            &[
                (
                    "word/document.xml",
                    "<w:document><w:body><w:p><w:r><w:t>Base </w:t></w:r><w:ins w:id=\"2\" w:author=\"Editor\" w:date=\"2026-01-01\"><w:r><w:t>change</w:t></w:r></w:ins><w:del w:id=\"3\" w:author=\"Editor\"><w:r><w:delText>old</w:delText></w:r></w:del></w:p><w:p><w:commentRangeStart w:id=\"7\"/><w:r><w:t>Second paragraph.</w:t></w:r><w:commentRangeEnd w:id=\"7\"/><w:r><w:commentReference w:id=\"7\"/></w:r></w:p></w:body></w:document>",
                ),
                (
                    "word/comments.xml",
                    "<w:comments><w:comment w:id=\"7\" w:author=\"Reviewer\" w:date=\"2026-01-02\"><w:p><w:r><w:t>Clarify this.</w:t></w:r></w:p></w:comment><w:comment w:id=\"8\" w:author=\"Reviewer\"><w:p><w:r><w:t>Unanchored note.</w:t></w:r></w:p></w:comment></w:comments>",
                ),
            ],
        );
        let items = inspect_docx_review(&docx).unwrap();
        assert_eq!(items.len(), 4);
        let insertion = items.iter().find(|item| item.kind == "insertion").unwrap();
        assert_eq!(insertion.text, "change");
        assert_eq!(insertion.author.as_deref(), Some("Editor"));
        let deletion = items.iter().find(|item| item.kind == "deletion").unwrap();
        assert_eq!(deletion.text, "old");
        let comment = items
            .iter()
            .find(|item| item.comment_id.as_deref() == Some("7"))
            .unwrap();
        assert_eq!(comment.comment_id.as_deref(), Some("7"));
        assert_eq!(comment.paragraph_index, Some(1));
        assert_eq!(comment.text, "Clarify this.");
        let unanchored = items
            .iter()
            .find(|item| item.comment_id.as_deref() == Some("8"))
            .unwrap();
        assert_eq!(unanchored.paragraph_index, None);
        assert_eq!(
            docx_review_text(&docx, true).unwrap(),
            "Base change\n\nSecond paragraph."
        );
        assert_eq!(
            docx_review_text(&docx, false).unwrap(),
            "Base old\n\nSecond paragraph."
        );
    }

    #[test]
    fn epub_staging_reads_spine_in_order() {
        let directory = tempfile::tempdir().unwrap();
        let epub = directory.path().join("fixture.epub");
        synthetic_epub(&epub);
        let staging = inspect_manuscript(&epub).unwrap();
        assert_eq!(staging.title, "Fixture Book");
        assert_eq!(staging.units.len(), 1);
        assert!(staging.units[0].body_markdown.contains("EPUB prose."));
    }
}

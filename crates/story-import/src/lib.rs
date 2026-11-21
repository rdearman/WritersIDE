use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
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
    pub paragraph_index: usize,
    pub comment_id: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub text: String,
}

/// Inventories Word insertions, deletions, and comments without applying them.
pub fn inspect_docx_review(path: impl AsRef<Path>) -> Result<Vec<DocxReviewItem>, ImportError> {
    let bytes = std::fs::read(path).map_err(|error| ImportError::Io(error.to_string()))?;
    let document = read_zip_text(&bytes, "word/document.xml")?;
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
                paragraph_index: prefix.matches("<w:p").count().saturating_sub(1),
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
            items.push(DocxReviewItem {
                kind: "comment".into(),
                paragraph_index: 0,
                comment_id: xml_attribute(block, "w:id"),
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
                    "<w:document><w:body><w:p><w:r><w:t>Base </w:t></w:r><w:ins w:id=\"2\" w:author=\"Editor\" w:date=\"2026-01-01\"><w:r><w:t>change</w:t></w:r></w:ins><w:del w:id=\"3\" w:author=\"Editor\"><w:r><w:delText>old</w:delText></w:r></w:del></w:p></w:body></w:document>",
                ),
                (
                    "word/comments.xml",
                    "<w:comments><w:comment w:id=\"7\" w:author=\"Reviewer\" w:date=\"2026-01-02\"><w:p><w:r><w:t>Clarify this.</w:t></w:r></w:p></w:comment></w:comments>",
                ),
            ],
        );
        let items = inspect_docx_review(&docx).unwrap();
        assert_eq!(items.len(), 3);
        let insertion = items.iter().find(|item| item.kind == "insertion").unwrap();
        assert_eq!(insertion.text, "change");
        assert_eq!(insertion.author.as_deref(), Some("Editor"));
        let deletion = items.iter().find(|item| item.kind == "deletion").unwrap();
        assert_eq!(deletion.text, "old");
        let comment = items.iter().find(|item| item.kind == "comment").unwrap();
        assert_eq!(comment.comment_id.as_deref(), Some("7"));
        assert_eq!(comment.text, "Clarify this.");
        assert_eq!(docx_review_text(&docx, true).unwrap(), "Base change");
        assert_eq!(docx_review_text(&docx, false).unwrap(), "Base old");
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

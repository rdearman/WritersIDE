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
    PlainText,
    Markdown,
    Org,
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
    #[error("text source is not valid UTF-8: {0}")]
    InvalidEncoding(String),
    #[error("invalid manuscript archive: {0}")]
    Archive(String),
    #[error("invalid Scrivener import mapping: {0}")]
    InvalidMapping(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManuscriptUnit {
    pub source_document_id: String,
    pub title: String,
    #[serde(default)]
    pub heading_level: Option<u8>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TextDocumentFormat {
    PlainText,
    Markdown,
    Org,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OrgImportMode {
    #[default]
    SingleDocument,
    HeadingHierarchy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextDocumentUnit {
    pub source_document_id: String,
    pub title: String,
    /// Org heading depth (1–6), or `None` for a single document/prose before
    /// the first heading. This is preview metadata; it does not change source.
    pub heading_level: Option<u8>,
    pub body_markdown: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextDocumentPreview {
    pub format: TextDocumentFormat,
    pub source_path: String,
    pub source_fingerprint: String,
    pub title: String,
    pub units: Vec<TextDocumentUnit>,
    pub warnings: Vec<String>,
    pub unsupported: Vec<String>,
}

/// Reads a text, Markdown, or Org-mode source without modifying it. Org
/// hierarchy mode groups prose under its source headings and preserves the
/// heading depth for a later destination-mapping step.
pub fn inspect_text_document(
    path: impl AsRef<Path>,
    org_mode: OrgImportMode,
) -> Result<TextDocumentPreview, ImportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| ImportError::Io(error.to_string()))?;
    let fingerprint = format!("{:x}", Sha256::digest(&bytes));
    let source = String::from_utf8(bytes)
        .map_err(|error| ImportError::InvalidEncoding(error.to_string()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let format = match extension.as_str() {
        "txt" | "text" => TextDocumentFormat::PlainText,
        "md" | "markdown" | "mdown" => TextDocumentFormat::Markdown,
        "org" => TextDocumentFormat::Org,
        _ => return Err(ImportError::UnsupportedFormat(extension)),
    };
    let title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Imported Text")
        .to_owned();
    let mut warnings = Vec::new();
    let mut unsupported = if format == TextDocumentFormat::Org {
        org_unsupported_constructs(&source)
    } else {
        Vec::new()
    };
    let units = if format == TextDocumentFormat::Org && org_mode == OrgImportMode::HeadingHierarchy
    {
        parse_org_hierarchy(&source, &fingerprint, &mut unsupported)
    } else {
        let body_markdown = if format == TextDocumentFormat::Org {
            source
                .lines()
                .map(org_line_to_markdown)
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            source.clone()
        };
        vec![TextDocumentUnit {
            source_document_id: format!("text:{fingerprint}:0"),
            title: title.clone(),
            heading_level: None,
            body_markdown,
        }]
    };
    if units.is_empty() {
        warnings.push("No text units were found in the source".into());
    }
    Ok(TextDocumentPreview {
        format,
        source_path: path.to_string_lossy().into_owned(),
        source_fingerprint: fingerprint,
        title,
        units,
        warnings,
        unsupported,
    })
}

fn parse_org_hierarchy(
    source: &str,
    fingerprint: &str,
    unsupported: &mut Vec<String>,
) -> Vec<TextDocumentUnit> {
    let mut units = Vec::new();
    let mut title: Option<String> = None;
    let mut level = None;
    let mut body = Vec::new();
    let mut index = 0usize;
    for line in source.lines() {
        if let Some((heading_level, heading)) = parse_org_heading(line) {
            if heading_level > 2 {
                unsupported.push(format!(
                    "Org heading level {heading_level} is retained as a scene under its nearest chapter"
                ));
            }
            if title.is_some() || body.iter().any(|part: &String| !part.is_empty()) {
                let unit_title = title.take().unwrap_or_else(|| "Preamble".into());
                units.push(TextDocumentUnit {
                    source_document_id: format!("org:{fingerprint}:{index}"),
                    title: unit_title,
                    heading_level: level,
                    body_markdown: body.join("\n"),
                });
                index += 1;
                body.clear();
            }
            title = Some(org_inline_to_markdown(heading.trim()));
            level = Some(heading_level);
        } else {
            if let Some(description) = org_unsupported_line(line) {
                unsupported.push(description);
            }
            body.push(org_line_to_markdown(line));
        }
    }
    if title.is_some() || !body.is_empty() {
        units.push(TextDocumentUnit {
            source_document_id: format!("org:{fingerprint}:{index}"),
            title: title.unwrap_or_else(|| "Preamble".into()),
            heading_level: level,
            body_markdown: body.join("\n"),
        });
    }
    if units.is_empty() && !source.is_empty() {
        units.push(TextDocumentUnit {
            source_document_id: format!("org:{fingerprint}:0"),
            title: "Preamble".into(),
            heading_level: None,
            body_markdown: source
                .lines()
                .map(org_line_to_markdown)
                .collect::<Vec<_>>()
                .join("\n"),
        });
    }
    unsupported.sort();
    unsupported.dedup();
    units
}

fn parse_org_heading(line: &str) -> Option<(u8, &str)> {
    let level = line.bytes().take_while(|byte| *byte == b'*').count();
    if level == 0
        || !line
            .as_bytes()
            .get(level)
            .is_some_and(u8::is_ascii_whitespace)
    {
        return None;
    }
    Some((
        level.min(u8::MAX as usize) as u8,
        line[level..].trim_start(),
    ))
}

fn org_unsupported_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("#+begin_") || trimmed.starts_with("#+BEGIN_") {
        Some(format!("Org block needs review: {trimmed}"))
    } else if trimmed.starts_with(':')
        && trimmed.ends_with(':')
        && trimmed.matches(':').count() >= 2
    {
        Some(format!("Org drawer/property line needs review: {trimmed}"))
    } else if trimmed.starts_with("#+") {
        Some(format!("Org directive needs review: {trimmed}"))
    } else {
        None
    }
}

fn org_unsupported_constructs(source: &str) -> Vec<String> {
    let mut unsupported = Vec::new();
    for line in source.lines() {
        if let Some(description) = org_unsupported_line(line) {
            unsupported.push(description);
        }
        if parse_org_heading(line).is_some()
            && line.split_whitespace().nth(1).is_some_and(|word| {
                matches!(word, "TODO" | "DONE" | "NEXT" | "WAITING" | "CANCELLED")
            })
        {
            unsupported.push(format!("Org task state needs review: {}", line.trim()));
        }
    }
    if source.contains("[[") {
        unsupported.push("Org links are converted to Markdown links; review their targets".into());
    }
    unsupported.sort();
    unsupported.dedup();
    unsupported
}

fn org_line_to_markdown(line: &str) -> String {
    if let Some((level, heading)) = parse_org_heading(line) {
        return format!(
            "{} {}",
            "#".repeat(level.min(6) as usize),
            org_inline_to_markdown(heading)
        );
    }
    org_inline_to_markdown(line)
}

fn org_inline_to_markdown(line: &str) -> String {
    // Keep URL path separators inside Org links untouched while converting the
    // link syntax itself. Emphasis on the same link-bearing line is left as Org
    // syntax rather than risking a lossy rewrite.
    if line.contains("[[") {
        return convert_org_links(line);
    }
    let mut out = String::with_capacity(line.len());
    let mut chars = line.char_indices().peekable();
    while let Some((start, character)) = chars.next() {
        let delimiter = match character {
            '*' => Some("**"),
            '/' => Some("*"),
            '~' | '=' => Some("`"),
            '+' => Some("~~"),
            _ => None,
        };
        if let Some(markdown) = delimiter {
            let close = line[start + character.len_utf8()..]
                .find(character)
                .map(|offset| start + character.len_utf8() + offset);
            if let Some(end) = close {
                let content_start = start + character.len_utf8();
                let content = &line[content_start..end];
                if !content.is_empty()
                    && !content.chars().next().is_some_and(char::is_whitespace)
                    && !content.chars().next_back().is_some_and(char::is_whitespace)
                    && !(character == '/'
                        && (line[..start].ends_with(':') || line[..start].contains("://")))
                {
                    out.push_str(markdown);
                    out.push_str(content);
                    out.push_str(markdown);
                    while chars.peek().is_some_and(|(offset, _)| *offset <= end) {
                        chars.next();
                    }
                    continue;
                }
            }
        }
        out.push(character);
    }
    convert_org_links(&out)
}

fn convert_org_links(source: &str) -> String {
    let mut output = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("[[") {
        output.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            output.push_str(&rest[start..]);
            return output;
        };
        let inner = &after[..end];
        let (target, label) = inner
            .split_once("][")
            .map_or((inner, inner), |(target, label)| (target, label));
        output.push('[');
        output.push_str(label);
        output.push_str("](");
        output.push_str(target);
        output.push(')');
        rest = &after[end + 2..];
    }
    output.push_str(rest);
    output
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DocxExtractedDocument {
    blocks: Vec<(String, bool)>,
    unsupported: Vec<String>,
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
    /// Project metadata recorded in `Settings/compile.xml`, when present.
    /// Values are kept as source metadata; callers decide whether an empty
    /// destination field may be populated from them.
    #[serde(default)]
    pub project_metadata: BTreeMap<String, String>,
    /// Scrivener compile settings are retained for review and later mapping.
    /// They are not silently treated as a WIDE publication profile.
    #[serde(default)]
    pub compile_settings: Option<ScrivenerCompileSettings>,
    /// Source-defined section type names keyed by stable Scrivener type ID.
    #[serde(default)]
    pub section_types: BTreeMap<String, String>,
    /// Scrivener's default section type by binder item category (`folders`,
    /// `containers`, or `files`). Explicit per-item SectionType metadata takes
    /// precedence over these defaults when resolving a compile layout.
    #[serde(default)]
    pub level_types: BTreeMap<String, String>,
    /// Project-defined status labels keyed by their source IDs. Binder items
    /// may store only the numeric ID, so retain this lookup to translate
    /// source statuses without losing their original field values.
    #[serde(default)]
    pub status_labels: BTreeMap<String, String>,
    pub items: Vec<ScrivenerBinderItem>,
    pub warnings: Vec<String>,
    pub unsupported: Vec<String>,
}

impl ScrivenerProjectInventory {
    /// Resolve the source section type used for compilation. Scrivener stores
    /// explicit overrides on individual binder items, while its LevelTypes
    /// table supplies defaults by binder category.
    pub fn effective_section_type_id<'a>(
        &'a self,
        item: &'a ScrivenerBinderItem,
    ) -> Option<&'a str> {
        if let Some(explicit) = item.section_type_id.as_deref() {
            return Some(explicit);
        }
        let category = match item.type_code.as_deref()?.to_ascii_lowercase().as_str() {
            "folder" | "draftfolder" | "researchfolder" | "trashfolder" => "folders",
            "container" | "group" => "containers",
            "text" | "file" | "document" => "files",
            _ => return None,
        };
        self.level_types.get(category).map(String::as_str)
    }

    /// Return a non-empty source metadata value by case-insensitive field name.
    pub fn metadata_value(&self, names: &[&str]) -> Option<&str> {
        self.project_metadata
            .iter()
            .find(|(key, value)| {
                !value.trim().is_empty() && names.iter().any(|name| key.eq_ignore_ascii_case(name))
            })
            .map(|(_, value)| value.trim())
    }

    /// The compile title recorded in Scrivener metadata, if the author set it.
    pub fn project_title(&self) -> Option<&str> {
        self.metadata_value(&["ProjectTitle"])
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerCompileSettings {
    #[serde(default)]
    pub content_scope: Option<String>,
    #[serde(default)]
    pub formats: Vec<ScrivenerCompileFormatSettings>,
    /// Every project-local `.scrformat` definition, keyed by its own source
    /// format ID. Some Scrivener projects refer to a built-in format ID in
    /// compile.xml while retaining a separate custom format file; keep both
    /// rather than assuming the identifiers match.
    #[serde(default)]
    pub available_layouts: BTreeMap<String, BTreeMap<String, ScrivenerCompileLayout>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerCompileFormatSettings {
    pub source_format_id: String,
    #[serde(default)]
    pub section_layouts: BTreeMap<String, String>,
    #[serde(default)]
    pub default_front_matter_id: Option<String>,
    #[serde(default)]
    pub add_default_front_matter: Option<bool>,
    #[serde(default)]
    pub default_back_matter_id: Option<String>,
    #[serde(default)]
    pub add_default_back_matter: Option<bool>,
    #[serde(default)]
    pub layouts: BTreeMap<String, ScrivenerCompileLayout>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerCompileLayout {
    pub source_layout_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub include_titles: Option<bool>,
    #[serde(default)]
    pub include_text: Option<bool>,
    #[serde(default)]
    pub title_prefix: Option<String>,
    #[serde(default)]
    pub title_suffix: Option<String>,
    #[serde(default)]
    pub text_prefix: Option<String>,
    #[serde(default)]
    pub text_suffix: Option<String>,
    #[serde(default)]
    pub between_separator_kind: Option<String>,
    #[serde(default)]
    pub between_separator_text: Option<String>,
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
    /// Binder item's own SectionType value. It is deliberately separate from
    /// recursively collected custom metadata and from its display title.
    #[serde(default)]
    pub section_type_id: Option<String>,
    pub synopsis: Option<String>,
    pub notes_markdown: Option<String>,
    pub content_markdown: Option<String>,
    #[serde(default)]
    pub embedded_assets: Vec<ScrivenerEmbeddedAsset>,
    pub fields: BTreeMap<String, String>,
    pub unsupported: Vec<String>,
}

/// An image extracted from a Scrivener RTF text body. Binary payloads are
/// available to an in-process import, while inventory JSON exposes only a
/// stable placeholder, digest and media type so preview reports stay compact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScrivenerEmbeddedAsset {
    pub placeholder: String,
    pub media_type: String,
    pub extension: String,
    pub sha256: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

/// A destination role for a Scrivener binder item. The proposed import plan
/// preserves ordinary manuscript folders as sections or chapters where their
/// parent/child shape establishes a safe outline role; `Unmapped` remains
/// available for genuinely conflicting source metadata that needs review.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ScrivenerDestinationKind {
    Unmapped,
    Section,
    Chapter,
    Scene,
    FrontMatter,
    BackMatter,
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

fn scrivener_item_is_folder(item: &ScrivenerBinderItem) -> bool {
    item.type_code.as_deref().is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "folder" | "draftfolder" | "researchfolder" | "trashfolder" | "group"
        )
    })
}

impl ScrivenerImportPlan {
    pub fn proposed(inventory: &ScrivenerProjectInventory) -> Self {
        let mut matter_roles = BTreeMap::<&str, ScrivenerDestinationKind>::new();
        if let Some(settings) = &inventory.compile_settings {
            for format in &settings.formats {
                for (enabled, source_id, kind) in [
                    (
                        format.add_default_front_matter,
                        format.default_front_matter_id.as_deref(),
                        ScrivenerDestinationKind::FrontMatter,
                    ),
                    (
                        format.add_default_back_matter,
                        format.default_back_matter_id.as_deref(),
                        ScrivenerDestinationKind::BackMatter,
                    ),
                ] {
                    if enabled == Some(true)
                        && let Some(source_id) = source_id
                        && inventory
                            .items
                            .iter()
                            .any(|item| item.source_document_id == source_id)
                    {
                        matter_roles
                            .entry(source_id)
                            .and_modify(|role| {
                                if *role != kind {
                                    // Do not silently classify a document selected
                                    // as both front and back matter. Leave it for
                                    // explicit author mapping in the import preview.
                                    *role = ScrivenerDestinationKind::Unmapped;
                                }
                            })
                            .or_insert(kind);
                    }
                }
            }
        }
        // Promote only manuscript ancestors needed to retain a selected
        // matter document's hierarchy. A private Project Notes/Research root
        // must not become a manuscript section just because it contains an
        // explicitly selected publication document.
        let mut matter_container_ids = BTreeSet::new();
        for source_id in matter_roles.keys() {
            let mut parent_id = inventory
                .items
                .iter()
                .find(|item| item.source_document_id == *source_id)
                .and_then(|item| item.parent_source_id.as_deref());
            while let Some(id) = parent_id {
                let Some(parent) = inventory
                    .items
                    .iter()
                    .find(|item| item.source_document_id == id)
                else {
                    break;
                };
                if parent.area == "trash" {
                    break;
                }
                if parent.area == "manuscript"
                    && !matter_roles.contains_key(parent.source_document_id.as_str())
                {
                    matter_container_ids.insert(parent.source_document_id.as_str());
                }
                parent_id = parent.parent_source_id.as_deref();
            }
        }
        let item_by_id = inventory
            .items
            .iter()
            .map(|item| (item.source_document_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let default_kind = |item: &ScrivenerBinderItem| {
            if item.area == "trash" {
                return ScrivenerDestinationKind::Skip;
            }
            if let Some(kind) = matter_roles.get(item.source_document_id.as_str()) {
                return *kind;
            }
            if matter_container_ids.contains(item.source_document_id.as_str()) {
                return ScrivenerDestinationKind::Section;
            }
            match item.area.as_str() {
                "research" | "unknown" => return ScrivenerDestinationKind::Research,
                "trash" => return ScrivenerDestinationKind::Skip,
                _ => {}
            }
            if scrivener_item_is_folder(item) {
                if item
                    .type_code
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case("DraftFolder"))
                {
                    return ScrivenerDestinationKind::Section;
                }
                let children = inventory
                    .items
                    .iter()
                    .filter(|child| {
                        child.parent_source_id.as_deref() == Some(&item.source_document_id)
                    })
                    .collect::<Vec<_>>();
                let has_child_folder = children.iter().any(|child| scrivener_item_is_folder(child));
                let has_child_document = !children.is_empty() && !has_child_folder;
                return if has_child_document {
                    ScrivenerDestinationKind::Chapter
                } else {
                    // Folders are preserved as organisational sections when
                    // their exact publishing role is not known. A folder that
                    // directly groups text documents is a useful chapter
                    // boundary; empty and nested containers remain sections.
                    ScrivenerDestinationKind::Section
                };
            }
            let parent_kind = item
                .parent_source_id
                .as_deref()
                .and_then(|parent| item_by_id.get(parent).copied())
                .map(|parent| {
                    if parent.area == "research" || parent.area == "unknown" {
                        ScrivenerDestinationKind::Research
                    } else if parent.area == "trash" {
                        ScrivenerDestinationKind::Skip
                    } else if scrivener_item_is_folder(parent) {
                        let children = inventory
                            .items
                            .iter()
                            .filter(|child| {
                                child.parent_source_id.as_deref()
                                    == Some(&parent.source_document_id)
                            })
                            .collect::<Vec<_>>();
                        let has_child_folder =
                            children.iter().any(|child| scrivener_item_is_folder(child));
                        if parent
                            .type_code
                            .as_deref()
                            .is_some_and(|value| value.eq_ignore_ascii_case("DraftFolder"))
                            || !children.is_empty() && has_child_folder
                        {
                            ScrivenerDestinationKind::Section
                        } else if !children.is_empty() {
                            ScrivenerDestinationKind::Chapter
                        } else {
                            ScrivenerDestinationKind::Section
                        }
                    } else if parent.area == "manuscript" {
                        // Nested text documents form a parent document with
                        // scene children; preserve that hierarchy where the
                        // source binder expresses it.
                        ScrivenerDestinationKind::Chapter
                    } else {
                        ScrivenerDestinationKind::Research
                    }
                });
            match parent_kind {
                Some(ScrivenerDestinationKind::Chapter) => ScrivenerDestinationKind::Scene,
                Some(ScrivenerDestinationKind::Section) | None => ScrivenerDestinationKind::Chapter,
                Some(ScrivenerDestinationKind::Research) => ScrivenerDestinationKind::Research,
                Some(ScrivenerDestinationKind::Skip) => ScrivenerDestinationKind::Skip,
                Some(ScrivenerDestinationKind::FrontMatter)
                | Some(ScrivenerDestinationKind::BackMatter) => matter_roles
                    .get(item.source_document_id.as_str())
                    .copied()
                    .unwrap_or(ScrivenerDestinationKind::Research),
                Some(ScrivenerDestinationKind::Scene)
                | Some(ScrivenerDestinationKind::Unmapped) => ScrivenerDestinationKind::Research,
            }
        };
        let decisions = inventory
            .items
            .iter()
            .map(|item| {
                let destination_kind = default_kind(item);
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
            let Some(_item) = inventory
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
                // Empty Scrivener text documents are legitimate outline
                // placeholders. Preserve them as editable empty manuscript
                // entries; an actual unreadable file is recorded separately
                // in the item's unsupported diagnostics.
                ScrivenerDestinationKind::Scene => preview.manuscript_items += 1,
                ScrivenerDestinationKind::FrontMatter | ScrivenerDestinationKind::BackMatter => {
                    preview.manuscript_items += 1
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
                    ScrivenerDestinationKind::FrontMatter
                    | ScrivenerDestinationKind::BackMatter => {
                        parent.destination_kind == decision.destination_kind
                            || parent.destination_kind == ScrivenerDestinationKind::Unmapped
                            || parent.destination_kind == ScrivenerDestinationKind::Section
                            || parent.destination_kind == ScrivenerDestinationKind::Chapter
                            || parent.destination_kind == ScrivenerDestinationKind::Research
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
    /// Text enclosed by the Word comment range in its original review document.
    /// This is independent of accept/reject interpretation of tracked changes.
    #[serde(default)]
    pub anchor_text: Option<String>,
    /// False means only a point reference or an incomplete range was available.
    #[serde(default)]
    pub anchor_complete: Option<bool>,
    pub comment_id: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DocxCommentAnchor {
    paragraph_index: usize,
    text: String,
    complete: bool,
}

/// Inventories Word insertions, deletions, and comments without applying them.
pub fn inspect_docx_review(path: impl AsRef<Path>) -> Result<Vec<DocxReviewItem>, ImportError> {
    let bytes = std::fs::read(path).map_err(|error| ImportError::Io(error.to_string()))?;
    let document = read_zip_text(&bytes, "word/document.xml")?;
    let comment_anchors = docx_comment_anchors(&document)?;
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
                anchor_text: None,
                anchor_complete: None,
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
                    .and_then(|id| comment_anchors.get(id))
                    .map(|anchor| anchor.paragraph_index),
                anchor_text: comment_id
                    .as_ref()
                    .and_then(|id| comment_anchors.get(id))
                    .map(|anchor| anchor.text.clone()),
                anchor_complete: comment_id
                    .as_ref()
                    .and_then(|id| comment_anchors.get(id))
                    .map(|anchor| anchor.complete),
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

/// Maps Word comment range/reference markers to the paragraphs and original
/// text they cover. A comment with no marker remains unanchored (`None`).
fn docx_comment_anchors(xml: &str) -> Result<BTreeMap<String, DocxCommentAnchor>, ImportError> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut paragraph = None::<usize>;
    let mut next_paragraph = 0usize;
    let mut active = BTreeMap::<String, String>::new();
    let mut anchors = BTreeMap::<String, DocxCommentAnchor>::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) if event.local_name().as_ref() == b"p" => {
                paragraph = Some(next_paragraph);
                next_paragraph += 1;
            }
            Ok(Event::End(event)) if event.local_name().as_ref() == b"p" => {
                if !active.is_empty() {
                    for quote in active.values_mut() {
                        quote.push('\n');
                    }
                }
                paragraph = None;
            }
            Ok(Event::Start(event) | Event::Empty(event))
                if matches!(
                    event.local_name().as_ref(),
                    b"commentRangeStart" | b"commentRangeEnd" | b"commentReference"
                ) =>
            {
                let id = xml_event_attribute(&event, "id", reader.decoder())?;
                let Some(id) = id else { continue };
                let Some(index) = paragraph else { continue };
                match event.local_name().as_ref() {
                    b"commentRangeStart" => {
                        active.entry(id.clone()).or_default();
                        anchors.entry(id).or_insert(DocxCommentAnchor {
                            paragraph_index: index,
                            text: String::new(),
                            complete: false,
                        });
                    }
                    b"commentRangeEnd" => {
                        let text = active.remove(&id);
                        let complete = text.is_some();
                        let paragraph_index = anchors
                            .get(&id)
                            .map(|anchor| anchor.paragraph_index)
                            .unwrap_or(index);
                        anchors.insert(
                            id,
                            DocxCommentAnchor {
                                paragraph_index,
                                text: text.unwrap_or_default(),
                                complete,
                            },
                        );
                    }
                    b"commentReference" => {
                        anchors.entry(id).or_insert(DocxCommentAnchor {
                            paragraph_index: index,
                            text: String::new(),
                            complete: false,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Start(event)) if matches!(event.local_name().as_ref(), b"t" | b"delText") => {
                in_text = true;
            }
            Ok(Event::End(event)) if matches!(event.local_name().as_ref(), b"t" | b"delText") => {
                in_text = false;
            }
            Ok(Event::Text(event)) if in_text && !active.is_empty() => {
                let text = event
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                for quote in active.values_mut() {
                    quote.push_str(&text);
                }
            }
            Ok(Event::GeneralRef(event)) if in_text && !active.is_empty() => {
                let reference = event
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                let value = event
                    .resolve_char_ref()
                    .map_err(|error| ImportError::Archive(error.to_string()))?
                    .map(|character| character.to_string())
                    .unwrap_or_else(|| decode_xml(&format!("&{reference};")));
                for quote in active.values_mut() {
                    quote.push_str(&value);
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
            Ok(quick_xml::events::Event::GeneralRef(event))
                if in_paragraph
                    && (if accept_changes {
                        deleted == 0
                    } else {
                        inserted == 0
                    }) =>
            {
                let reference = event
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(|error| ImportError::Archive(error.to_string()))?;
                let text = if let Some(character) = event
                    .resolve_char_ref()
                    .map_err(|error| ImportError::Archive(error.to_string()))?
                {
                    character.to_string()
                } else {
                    quick_xml::escape::unescape(&format!("&{reference};"))
                        .map_err(|error| ImportError::Archive(error.to_string()))?
                        .into_owned()
                };
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
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        extension.as_str(),
        "txt" | "text" | "md" | "markdown" | "mdown" | "org"
    ) {
        let preview = inspect_text_document(path, OrgImportMode::SingleDocument)?;
        let source_type = match preview.format {
            TextDocumentFormat::PlainText => SourceType::PlainText,
            TextDocumentFormat::Markdown => SourceType::Markdown,
            TextDocumentFormat::Org => SourceType::Org,
        };
        return Ok(ManuscriptStaging {
            source_type,
            source_path: preview.source_path,
            source_fingerprint: preview.source_fingerprint,
            title: preview.title,
            units: preview
                .units
                .into_iter()
                .map(|unit| ManuscriptUnit {
                    source_document_id: unit.source_document_id,
                    title: unit.title,
                    heading_level: unit.heading_level,
                    body_markdown: unit.body_markdown,
                    unsupported: preview.unsupported.clone(),
                })
                .collect(),
            warnings: preview.warnings,
            unsupported: preview.unsupported,
        });
    }
    let bytes = std::fs::read(path).map_err(|error| ImportError::Io(error.to_string()))?;
    let fingerprint = format!("{:x}", Sha256::digest(&bytes));
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
            let relationships = read_docx_relationships(&bytes)?;
            let extracted = extract_docx_blocks(&xml, &relationships)?;
            split_blocks_into_units(&mut staging, extracted.blocks, "docx");
            staging.unsupported.extend(extracted.unsupported);
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
                heading_level: None,
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
                    heading_level: None,
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
        project_metadata: BTreeMap::new(),
        compile_settings: None,
        section_types: scrivener_section_type_names(&root),
        level_types: scrivener_level_types(&root),
        status_labels: scrivener_status_labels(&root),
        items: Vec::new(),
        warnings: Vec::new(),
        unsupported: Vec::new(),
    };

    if let Some(settings_dir) = find_case_insensitive_child(&project_root, "Settings", true)? {
        if let Some(compile_path) =
            find_case_insensitive_child(&settings_dir, "compile.xml", false)?
        {
            let compile_xml = fs::read_to_string(&compile_path)
                .map_err(|error| ImportError::Io(error.to_string()))?;
            let compile_root = parse_scrivener_xml(&compile_xml)?;
            inventory.project_metadata = scrivener_project_metadata(&compile_root);
            inventory.compile_settings =
                Some(scrivener_compile_settings(&compile_root, &settings_dir)?);
        } else {
            inventory.warnings.push(
                "Scrivener compile.xml was not present; project metadata and format-level compile settings could not be inspected".into(),
            );
        }
    }

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

    if inventory.items.iter().any(|item| item.area == "unknown") {
        inventory.warnings.push(
            "Binder items outside recognised manuscript, research, or trash containers were preserved as private research and excluded from publication by default; review their roles before compiling".into(),
        );
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
            Ok(Event::GeneralRef(event)) => {
                if let Some(node) = stack.last_mut() {
                    let decoded = event
                        .decode()
                        .map_err(|error| ImportError::Archive(error.to_string()))?;
                    let character = if let Some(character) = event
                        .resolve_char_ref()
                        .map_err(|error| ImportError::Archive(error.to_string()))?
                    {
                        character.to_string()
                    } else {
                        match decoded.as_ref() {
                            "amp" => "&".to_owned(),
                            "lt" => "<".to_owned(),
                            "gt" => ">".to_owned(),
                            "apos" => "'".to_owned(),
                            "quot" => "\"".to_owned(),
                            entity => {
                                return Err(ImportError::Archive(format!(
                                    "unsupported XML entity reference &{entity};"
                                )));
                            }
                        }
                    };
                    node.text.push_str(&character);
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

fn scrivener_metadata_node(node: &ScrivenerXmlNode) -> Option<&ScrivenerXmlNode> {
    node.children
        .iter()
        .find(|child| child.name.eq_ignore_ascii_case("MetaData"))
}

fn scrivener_own_metadata_values(node: &ScrivenerXmlNode) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    if let Some(metadata) = scrivener_metadata_node(node) {
        scrivener_field_values(metadata, &mut values);
    } else {
        for child in &node.children {
            if child.children.is_empty() && !child.text.trim().is_empty() {
                values.insert(child.name.clone(), child.text.trim().to_owned());
            }
        }
    }
    values
}

fn scrivener_project_metadata(root: &ScrivenerXmlNode) -> BTreeMap<String, String> {
    let Some(project_settings) = find_scrivener_node(root, "ProjectSettings") else {
        return BTreeMap::new();
    };
    let Some(metadata) = project_settings
        .children
        .iter()
        .find(|child| child.name.eq_ignore_ascii_case("MetaData"))
    else {
        return BTreeMap::new();
    };
    let mut values = BTreeMap::new();
    scrivener_field_values(metadata, &mut values);
    values
}

fn scrivener_section_type_names(root: &ScrivenerXmlNode) -> BTreeMap<String, String> {
    let Some(section_types) = find_scrivener_node(root, "SectionTypes") else {
        return BTreeMap::new();
    };
    let Some(definitions) = section_types
        .children
        .iter()
        .find(|child| child.name.eq_ignore_ascii_case("TypeDefinitions"))
    else {
        return BTreeMap::new();
    };
    definitions
        .children
        .iter()
        .filter(|node| node.name.eq_ignore_ascii_case("Type"))
        .filter_map(|node| {
            let id = node.attributes.get("ID")?;
            let name = node.text.trim();
            (!name.is_empty()).then(|| (id.clone(), name.to_owned()))
        })
        .collect()
}

fn scrivener_level_types(root: &ScrivenerXmlNode) -> BTreeMap<String, String> {
    let Some(level_types) = find_scrivener_node(root, "LevelTypes") else {
        return BTreeMap::new();
    };
    level_types
        .children
        .iter()
        .filter_map(|category| {
            let type_id = category
                .children
                .iter()
                .find(|child| child.name.eq_ignore_ascii_case("Type"))?
                .text
                .trim()
                .to_owned();
            (!type_id.is_empty()).then(|| (category.name.to_ascii_lowercase(), type_id))
        })
        .collect()
}

fn scrivener_status_labels(root: &ScrivenerXmlNode) -> BTreeMap<String, String> {
    let Some(settings) = find_scrivener_node(root, "StatusSettings") else {
        return BTreeMap::new();
    };
    let Some(items) = settings
        .children
        .iter()
        .find(|child| child.name.eq_ignore_ascii_case("StatusItems"))
    else {
        return BTreeMap::new();
    };
    items
        .children
        .iter()
        .filter(|node| node.name.eq_ignore_ascii_case("Status"))
        .filter_map(|node| {
            let id = node.attributes.get("ID")?;
            let label = node.text.trim();
            (!label.is_empty()).then(|| (id.clone(), label.to_owned()))
        })
        .collect()
}

fn parse_scrivener_format_layouts(
    settings_dir: &Path,
) -> Result<BTreeMap<String, BTreeMap<String, ScrivenerCompileLayout>>, ImportError> {
    fn collect_layouts<'a>(node: &'a ScrivenerXmlNode, output: &mut Vec<&'a ScrivenerXmlNode>) {
        if node.name.eq_ignore_ascii_case("Layout") {
            output.push(node);
        }
        for child in &node.children {
            collect_layouts(child, output);
        }
    }

    let Some(format_dir) = find_case_insensitive_child(settings_dir, "Compile Formats", true)?
    else {
        return Ok(BTreeMap::new());
    };
    let entries = fs::read_dir(&format_dir).map_err(|error| ImportError::Io(error.to_string()))?;
    let mut formats = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|error| ImportError::Io(error.to_string()))?;
        let kind = entry
            .file_type()
            .map_err(|error| ImportError::Io(error.to_string()))?;
        if !kind.is_file()
            || !entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("scrformat"))
        {
            continue;
        }
        let source =
            fs::read_to_string(entry.path()).map_err(|error| ImportError::Io(error.to_string()))?;
        let root = parse_scrivener_xml(&source)?;
        let Some(format_node) = find_scrivener_node(&root, "CompileFormat") else {
            continue;
        };
        let Some(format_id) = format_node.attributes.get("ID").cloned() else {
            continue;
        };
        let mut layouts = BTreeMap::new();
        let mut layout_nodes = Vec::new();
        collect_layouts(format_node, &mut layout_nodes);
        for node in layout_nodes {
            let Some(id) = node.attributes.get("ID").cloned() else {
                continue;
            };
            let child = |name: &str| {
                node.children
                    .iter()
                    .find(|child| child.name.eq_ignore_ascii_case(name))
            };
            let include = child("Include");
            let titles = child("Titles");
            let title_prefix = titles
                .and_then(|node| node.children.iter().find(|child| child.name == "Prefix"))
                .map(|node| node.text.clone())
                .filter(|text| !text.is_empty());
            let title_suffix = titles
                .and_then(|node| node.children.iter().find(|child| child.name == "Suffix"))
                .map(|node| node.text.clone())
                .filter(|text| !text.is_empty());
            let text = child("Text");
            let separators = child("Separators");
            let between = separators.and_then(|node| {
                node.children
                    .iter()
                    .find(|child| child.name.eq_ignore_ascii_case("Between"))
            });
            let yes_no = |value: Option<&String>| {
                value.map(|value| {
                    matches!(value.to_ascii_lowercase().as_str(), "yes" | "true" | "1")
                })
            };
            layouts.insert(
                id.clone(),
                ScrivenerCompileLayout {
                    source_layout_id: id.clone(),
                    name: node.attributes.get("Name").cloned(),
                    include_titles: yes_no(include.and_then(|node| node.attributes.get("Titles"))),
                    include_text: yes_no(include.and_then(|node| node.attributes.get("Text"))),
                    title_prefix,
                    title_suffix,
                    text_prefix: text
                        .and_then(|node| node.children.iter().find(|child| child.name == "Prefix"))
                        .map(|node| node.text.clone())
                        .filter(|text| !text.is_empty()),
                    text_suffix: text
                        .and_then(|node| node.children.iter().find(|child| child.name == "Suffix"))
                        .map(|node| node.text.clone())
                        .filter(|text| !text.is_empty()),
                    between_separator_kind: between
                        .and_then(|node| node.attributes.get("Type"))
                        .cloned(),
                    between_separator_text: between
                        .map(|node| node.text.clone())
                        .filter(|text| !text.is_empty()),
                },
            );
        }
        formats.insert(format_id, layouts);
    }
    Ok(formats)
}

fn scrivener_compile_settings(
    root: &ScrivenerXmlNode,
    settings_dir: &Path,
) -> Result<ScrivenerCompileSettings, ImportError> {
    let project_content = find_scrivener_node(root, "Content");
    let content_scope = project_content
        .and_then(|node| node.attributes.get("Scope"))
        .cloned();
    let Some(format_settings) = find_scrivener_node(root, "FormatSettings") else {
        return Ok(ScrivenerCompileSettings {
            content_scope,
            formats: Vec::new(),
            available_layouts: BTreeMap::new(),
        });
    };
    let source_layouts = parse_scrivener_format_layouts(settings_dir)?;
    let formats = format_settings
        .children
        .iter()
        .filter(|node| node.name.eq_ignore_ascii_case("Format"))
        .map(|format| {
            let mut settings = ScrivenerCompileFormatSettings {
                source_format_id: format.attributes.get("ID").cloned().unwrap_or_default(),
                ..ScrivenerCompileFormatSettings::default()
            };
            settings.layouts = source_layouts
                .get(&settings.source_format_id)
                .cloned()
                .unwrap_or_default();
            if let Some(layouts) = format
                .children
                .iter()
                .find(|child| child.name.eq_ignore_ascii_case("SectionLayouts"))
            {
                for layout in layouts
                    .children
                    .iter()
                    .filter(|child| child.name.eq_ignore_ascii_case("Type"))
                {
                    if let Some(section_type_id) = layout.attributes.get("ID") {
                        let layout_name = layout.text.trim();
                        if !layout_name.is_empty() {
                            settings
                                .section_layouts
                                .insert(section_type_id.clone(), layout_name.to_owned());
                        }
                    }
                }
            }
            if let Some(matter) = format
                .children
                .iter()
                .find(|child| child.name.eq_ignore_ascii_case("FrontAndBackMatter"))
            {
                for (name, id, add) in [
                    (
                        "DefaultFrontMatter",
                        &mut settings.default_front_matter_id,
                        &mut settings.add_default_front_matter,
                    ),
                    (
                        "DefaultBackMatter",
                        &mut settings.default_back_matter_id,
                        &mut settings.add_default_back_matter,
                    ),
                ] {
                    if let Some(node) = matter
                        .children
                        .iter()
                        .find(|child| child.name.eq_ignore_ascii_case(name))
                    {
                        let value = node.text.trim();
                        *id = (!value.is_empty()).then(|| value.to_owned());
                        *add = node.attributes.get("Add").map(|value| {
                            matches!(value.to_ascii_lowercase().as_str(), "yes" | "true" | "1")
                        });
                    }
                }
            }
            settings
        })
        .collect();
    Ok(ScrivenerCompileSettings {
        content_scope,
        formats,
        available_layouts: source_layouts,
    })
}

fn scrivener_named_value(node: &ScrivenerXmlNode, name: &str) -> Option<String> {
    node.children
        .iter()
        .find(|child| child.name.eq_ignore_ascii_case(name))
        .map(|child| child.text.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn classify_scrivener_area(title: &str, type_code: Option<&str>, inherited: &str) -> String {
    // Trash and private Research are subtree roles. A descendant's title or
    // section metadata must never promote it back into the publishable draft.
    if inherited.eq_ignore_ascii_case("trash") {
        return "trash".into();
    }
    if inherited.eq_ignore_ascii_case("research") {
        return "research".into();
    }
    match type_code.map(str::to_ascii_lowercase).as_deref() {
        Some("draftfolder") => return "manuscript".into(),
        Some("researchfolder") => return "research".into(),
        Some("trashfolder") => return "trash".into(),
        _ => {}
    }
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
    let own_metadata = scrivener_own_metadata_values(node);
    let include_in_compile = own_metadata
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
    let section_type_id = own_metadata
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("SectionType"))
        .map(|(_, value)| value.clone())
        .filter(|value| !value.is_empty());
    let mut unsupported = Vec::new();
    let mut synopsis = None;
    let mut notes_markdown = None;
    let mut content_markdown = None;
    let mut embedded_assets = Vec::new();
    if read_candidate_content {
        if let Some(data_dir) =
            find_case_insensitive_child(&project_root.join("Files"), "Data", true)?
            && let Some(item_dir) =
                find_case_insensitive_child(&data_dir, &source_document_id, true)?
        {
            if let Some(file) = find_case_insensitive_child(&item_dir, "content.rtf", false)? {
                let bytes = fs::read(&file).map_err(|error| ImportError::Io(error.to_string()))?;
                let rtf = String::from_utf8_lossy(&bytes);
                let (
                    markdown,
                    assets,
                    unsupported_picture_count,
                    unsupported_link_count,
                    unsupported_table_count,
                ) = extract_rtf_markdown_with_assets(&rtf);
                content_markdown = Some(markdown);
                embedded_assets = assets;
                if unsupported_picture_count > 0 {
                    unsupported.push(format!(
                        "{} embedded RTF picture(s) use unsupported or malformed image data",
                        unsupported_picture_count
                    ));
                }
                if unsupported_link_count > 0 {
                    unsupported.push(format!(
                        "{} RTF hyperlink field(s) use unsupported or unsafe destinations",
                        unsupported_link_count
                    ));
                }
                if unsupported_table_count > 0 {
                    unsupported.push(format!(
                        "{} RTF table(s) contain malformed rows, merged cells, or nested table structures that need review",
                        unsupported_table_count
                    ));
                }
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
            && !type_code.as_deref().is_some_and(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "folder" | "draftfolder" | "researchfolder" | "trashfolder" | "group"
                )
            })
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
        section_type_id,
        synopsis,
        notes_markdown,
        content_markdown,
        embedded_assets,
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

fn read_optional_zip_text(bytes: &[u8], member: &str) -> Result<Option<String>, ImportError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| ImportError::Archive(error.to_string()))?;
    let mut file = match archive.by_name(member) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(ImportError::Archive(error.to_string())),
    };
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text)
        .map_err(|error| ImportError::Archive(error.to_string()))?;
    Ok(Some(text))
}

fn read_docx_relationships(bytes: &[u8]) -> Result<BTreeMap<String, (String, bool)>, ImportError> {
    let Some(xml) = read_optional_zip_text(bytes, "word/_rels/document.xml.rels")? else {
        return Ok(BTreeMap::new());
    };
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut relationships = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event) | quick_xml::events::Event::Empty(event))
                if event.local_name().as_ref() == b"Relationship" =>
            {
                let id = xml_event_attribute(&event, "Id", reader.decoder())?;
                let target = xml_event_attribute(&event, "Target", reader.decoder())?;
                let external = xml_event_attribute(&event, "TargetMode", reader.decoder())?
                    .is_some_and(|mode| mode.eq_ignore_ascii_case("external"));
                if let (Some(id), Some(target)) = (id, target) {
                    relationships.insert(id, (target, external));
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(ImportError::Archive(error.to_string())),
        }
    }
    Ok(relationships)
}

fn encode_markdown_link_target(target: &str) -> String {
    let mut encoded = String::new();
    for byte in target.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=%".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn escape_markdown_link_label(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
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

fn extract_docx_blocks(
    xml: &str,
    relationships: &BTreeMap<String, (String, bool)>,
) -> Result<DocxExtractedDocument, ImportError> {
    let mut blocks = Vec::new();
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut in_paragraph = false;
    let mut is_heading = false;
    let mut paragraph = String::new();
    let mut run_text = String::new();
    let (mut bold, mut italic, mut strike) = (false, false, false);
    let mut active_hyperlink: Option<(usize, String)> = None;
    let mut unsupported_hyperlink = false;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event)) => match event.local_name().as_ref() {
                b"hyperlink" if in_paragraph => {
                    flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                    let anchor = xml_event_attribute(&event, "anchor", reader.decoder())?;
                    let target = if let Some(anchor) = anchor {
                        Some(format!("#{anchor}"))
                    } else if let Some(id) = xml_event_attribute(&event, "id", reader.decoder())? {
                        relationships.get(&id).and_then(|(target, external)| {
                            let lower = target.to_ascii_lowercase();
                            (*external
                                && (lower.starts_with("http://")
                                    || lower.starts_with("https://")
                                    || lower.starts_with("mailto:")))
                            .then(|| target.clone())
                        })
                    } else {
                        None
                    };
                    if let Some(target) = target {
                        active_hyperlink =
                            Some((paragraph.len(), encode_markdown_link_target(&target)));
                    } else {
                        unsupported_hyperlink = true;
                    }
                }
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
            Ok(quick_xml::events::Event::End(event))
                if event.local_name().as_ref() == b"hyperlink" =>
            {
                flush_docx_run(&mut paragraph, &mut run_text, bold, italic, strike);
                if let Some((start, target)) = active_hyperlink.take()
                    && start <= paragraph.len()
                {
                    let label = escape_markdown_link_label(&paragraph[start..]);
                    if !label.is_empty() {
                        paragraph.replace_range(start.., &format!("[{label}](<{target}>)"));
                    }
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
    let unsupported = unsupported_hyperlink
        .then(|| {
            "DOCX hyperlink with an unresolved or unsupported destination is flattened to anchor text"
                .into()
        })
        .into_iter()
        .collect();
    Ok(DocxExtractedDocument {
        blocks,
        unsupported,
    })
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
                heading_level: None,
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
            heading_level: None,
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
/// This intentionally preserves bold, italic, underline and strike runs
/// without trying to reproduce page layout or embedded objects.
fn find_rtf_group_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                index += 1;
                if index >= bytes.len() {
                    break;
                }
                if bytes[index].is_ascii_alphabetic() {
                    let word_start = index;
                    while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
                        index += 1;
                    }
                    let word = &bytes[word_start..index];
                    if index < bytes.len() && bytes[index] == b'-' {
                        index += 1;
                    }
                    let number_start = index;
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                    if word == b"bin" && number_start < index {
                        let length = std::str::from_utf8(&bytes[number_start..index])
                            .ok()?
                            .parse::<usize>()
                            .ok()?;
                        if index < bytes.len() && bytes[index] == b' ' {
                            index += 1;
                        }
                        index = index.checked_add(length)?;
                        continue;
                    }
                    if index < bytes.len() && bytes[index] == b' ' {
                        index += 1;
                    }
                } else if bytes[index] == b'\'' {
                    index = index.saturating_add(3);
                } else {
                    index += 1;
                }
            }
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                index += 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => index += 1,
        }
    }
    None
}

fn decode_rtf_picture(group: &[u8]) -> Option<(String, String, Vec<u8>)> {
    let format_at = group
        .windows(b"\\pngblip".len())
        .position(|window| window == b"\\pngblip")
        .map(|at| (at, "image/png", "png"))
        .or_else(|| {
            group
                .windows(b"\\jpegblip".len())
                .position(|window| window == b"\\jpegblip")
                .map(|at| (at, "image/jpeg", "jpg"))
        })?;
    let (at, media_type, extension) = format_at;
    let control_length = if extension == "png" {
        b"\\pngblip".len()
    } else {
        b"\\jpegblip".len()
    };
    let mut index = at + control_length;
    let mut high_nibble = None;
    let mut output = Vec::new();
    while index < group.len() {
        if group[index] == b'\\' {
            index += 1;
            if index >= group.len() {
                break;
            }
            if group[index].is_ascii_alphabetic() {
                let word_start = index;
                while index < group.len() && group[index].is_ascii_alphabetic() {
                    index += 1;
                }
                let word = &group[word_start..index];
                if index < group.len() && group[index] == b'-' {
                    index += 1;
                }
                let number_start = index;
                while index < group.len() && group[index].is_ascii_digit() {
                    index += 1;
                }
                if word == b"bin" && number_start < index {
                    let length = std::str::from_utf8(&group[number_start..index])
                        .ok()?
                        .parse::<usize>()
                        .ok()?;
                    if index < group.len() && group[index] == b' ' {
                        index += 1;
                    }
                    index = index.checked_add(length)?;
                } else if index < group.len() && group[index] == b' ' {
                    index += 1;
                }
            } else if group[index] == b'\'' {
                index = index.saturating_add(3);
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(nibble) = (group[index] as char).to_digit(16) {
            if let Some(high) = high_nibble.take() {
                output.push(((high << 4) | nibble) as u8);
            } else {
                high_nibble = Some(nibble);
            }
        }
        index += 1;
    }
    let valid_signature = if extension == "png" {
        output.starts_with(b"\x89PNG\r\n\x1a\n")
    } else {
        output.starts_with(&[0xff, 0xd8]) && output.ends_with(&[0xff, 0xd9])
    };
    valid_signature.then(|| (media_type.to_owned(), extension.to_owned(), output))
}

fn rtf_child_group_after<'a>(group: &'a [u8], control: &[u8]) -> Option<&'a [u8]> {
    let control_at = group
        .windows(control.len())
        .position(|window| window == control)?;
    let start = group[..control_at].iter().rposition(|byte| *byte == b'{')?;
    let end = find_rtf_group_end(group, start)?;
    Some(&group[start..end])
}

fn rtf_hyperlink_target(instruction: &[u8]) -> Option<String> {
    let instruction = String::from_utf8_lossy(instruction);
    let start = instruction.to_ascii_uppercase().find("HYPERLINK")? + "HYPERLINK".len();
    let mut rest = instruction[start..].trim();
    if rest.starts_with('"') {
        rest = &rest[1..];
        let end = rest.find('"')?;
        let target = &rest[..end];
        let target = markdown_safe_uri(target);
        let scheme = target.split(':').next()?.to_ascii_lowercase();
        return matches!(scheme.as_str(), "http" | "https" | "mailto").then_some(target);
    }
    if let Some(anchor) = rest
        .strip_prefix("\\l ")
        .or_else(|| rest.strip_prefix("\\l"))
    {
        let anchor = anchor.trim().trim_matches('"');
        if !anchor.is_empty() {
            return Some(format!("#{anchor}"));
        }
    }
    None
}

fn markdown_safe_uri(value: &str) -> String {
    value
        .replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
        .replace('<', "%3C")
        .replace('>', "%3E")
}

fn extract_rtf_hyperlinks(source: &str) -> (String, usize, Vec<(String, String)>) {
    let bytes = source.as_bytes();
    let mut segments = Vec::<(usize, usize, String)>::new();
    let mut links = Vec::new();
    let mut unsupported = 0usize;
    let mut search_from = 0usize;
    while let Some(relative) = source[search_from..].find("\\field") {
        let field_at = search_from + relative;
        let after_word = field_at + "\\field".len();
        if source
            .as_bytes()
            .get(after_word)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            search_from = after_word;
            continue;
        }
        let Some(start) = bytes[..field_at].iter().rposition(|byte| *byte == b'{') else {
            search_from = after_word;
            continue;
        };
        let Some(end) = find_rtf_group_end(bytes, start) else {
            unsupported = unsupported.saturating_add(1);
            search_from = after_word;
            continue;
        };
        let field = &bytes[start..end];
        let instruction = rtf_child_group_after(field, b"\\fldinst");
        let result = rtf_child_group_after(field, b"\\fldrslt")
            .map(|group| extract_rtf_markdown_without_assets(&String::from_utf8_lossy(group)))
            .unwrap_or_default();
        let target = instruction.and_then(|group| {
            let text = group
                .strip_prefix(b"{\\*")
                .or_else(|| group.strip_prefix(b"{"))
                .unwrap_or(group);
            let text = text.strip_prefix(b"\\fldinst").unwrap_or(text);
            let text = text.strip_suffix(b"}").unwrap_or(text);
            rtf_hyperlink_target(text)
        });
        let replacement = if let Some(target) = target.filter(|_| !result.is_empty()) {
            let placeholder = format!("WIDEHYPERLINK{}END", links.len());
            links.push((placeholder.clone(), format!("[{result}]({target})")));
            placeholder
        } else {
            unsupported = unsupported.saturating_add(1);
            result
        };
        segments.push((start, end, replacement));
        search_from = end;
    }
    if segments.is_empty() {
        return (source.to_owned(), unsupported, links);
    }
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for (start, end, replacement) in segments {
        if start < cursor {
            continue;
        }
        output.push_str(&source[cursor..start]);
        output.push_str(&replacement);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    (output, unsupported, links)
}

fn extract_rtf_list_text(source: &str) -> (String, Vec<(String, String)>) {
    let bytes = source.as_bytes();
    let mut segments = Vec::<(usize, usize, String)>::new();
    let mut markers = Vec::new();
    let mut search_from = 0usize;
    while let Some(relative) = source[search_from..].find("\\listtext") {
        let control_at = search_from + relative;
        let after_word = control_at + "\\listtext".len();
        if source
            .as_bytes()
            .get(after_word)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            search_from = after_word;
            continue;
        }
        let Some(start) = bytes[..control_at].iter().rposition(|byte| *byte == b'{') else {
            search_from = after_word;
            continue;
        };
        let Some(end) = find_rtf_group_end(bytes, start) else {
            search_from = after_word;
            continue;
        };
        let group = &source[start..end];
        let marker = if group.contains("\\u9679") || group.contains("\\u9675") {
            Some("- ".to_owned())
        } else {
            let plain_marker = group.bytes().collect::<Vec<_>>();
            let mut numeric = String::new();
            let mut after_control = false;
            for byte in plain_marker {
                if byte == b'\\' {
                    after_control = true;
                    continue;
                }
                if after_control {
                    if !byte.is_ascii_alphanumeric() && byte != b'-' {
                        after_control = false;
                    }
                    continue;
                }
                if byte.is_ascii_digit() || (byte == b'.' && !numeric.is_empty()) {
                    numeric.push(char::from(byte));
                } else if !numeric.is_empty() {
                    break;
                }
            }
            numeric.ends_with('.').then(|| format!("{numeric} "))
        };
        let Some(marker) = marker else {
            search_from = end;
            continue;
        };
        let placeholder = format!("WIDERTFLISTITEM{}END", markers.len());
        let mut replacement_end = end;
        if source.as_bytes().get(end..end + 4) == Some(b"\\tab") {
            replacement_end += 4;
            if source.as_bytes().get(replacement_end) == Some(&b' ') {
                replacement_end += 1;
            }
        }
        // Removing a group immediately after an RTF control word can join the
        // following placeholder to that word (for example `\\plainWIDERTF…`),
        // making the RTF reader consume the placeholder as part of the control.
        // Keep a delimiter in the prepared RTF; the parser consumes this space.
        let replacement = if start > 0 && source.as_bytes()[start - 1].is_ascii_alphanumeric() {
            format!(" {placeholder}")
        } else {
            placeholder.clone()
        };
        segments.push((start, replacement_end, replacement));
        markers.push((placeholder, marker));
        search_from = replacement_end;
    }
    if segments.is_empty() {
        return (source.to_owned(), markers);
    }
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for (start, end, placeholder) in segments {
        if start < cursor {
            continue;
        }
        output.push_str(&source[cursor..start]);
        output.push_str(&placeholder);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    (output, markers)
}

fn extract_rtf_markdown_with_assets(
    source: &str,
) -> (String, Vec<ScrivenerEmbeddedAsset>, usize, usize, usize) {
    let (source, unsupported_link_count, links) = extract_rtf_hyperlinks(source);
    let (source, list_markers) = extract_rtf_list_text(&source);
    let source = source.as_str();
    let bytes = source.as_bytes();
    let mut segments = Vec::<(usize, usize, String, Option<ScrivenerEmbeddedAsset>)>::new();
    let mut search_from = 0usize;
    let mut unsupported_picture_count = 0usize;
    while let Some(relative) = source[search_from..].find("\\pict") {
        let pict_at = search_from + relative;
        let after_word = pict_at + "\\pict".len();
        let is_control_word_boundary =
            pict_at == 0 || !source.as_bytes()[pict_at - 1].is_ascii_alphabetic();
        if !is_control_word_boundary
            || source
                .as_bytes()
                .get(after_word)
                .is_some_and(u8::is_ascii_alphabetic)
        {
            search_from = after_word;
            continue;
        }
        let Some(picture_start) = bytes[..pict_at].iter().rposition(|byte| *byte == b'{') else {
            search_from = after_word;
            continue;
        };
        let Some(picture_end) = find_rtf_group_end(bytes, picture_start) else {
            unsupported_picture_count = unsupported_picture_count.saturating_add(1);
            search_from = after_word;
            continue;
        };
        let group = &bytes[picture_start..picture_end];
        let wrapper = source[..pict_at]
            .rfind("\\shppict")
            .filter(|at| !source[*at..pict_at].contains('}'))
            .and_then(|at| {
                let start = bytes[..at].iter().rposition(|byte| *byte == b'{')?;
                let end = find_rtf_group_end(bytes, start)?;
                (end >= picture_end).then_some((start, end))
            });
        let (start, end) = wrapper.unwrap_or((picture_start, picture_end));
        if let Some((media_type, extension, image_bytes)) = decode_rtf_picture(group) {
            let sha256 = format!("{:x}", Sha256::digest(&image_bytes));
            let placeholder = format!("WIDESCRIVENERASSET{sha256}END");
            let asset = ScrivenerEmbeddedAsset {
                placeholder: placeholder.clone(),
                media_type,
                extension,
                sha256,
                bytes: image_bytes,
            };
            segments.push((start, end, format!("\\par {placeholder}\\par"), Some(asset)));
        } else {
            unsupported_picture_count = unsupported_picture_count.saturating_add(1);
            segments.push((start, end, String::new(), None));
        }
        search_from = end;
    }

    let mut prepared = String::with_capacity(source.len());
    let mut cursor = 0usize;
    let mut assets = Vec::new();
    for (start, end, replacement, asset) in segments {
        if start < cursor {
            continue;
        }
        prepared.push_str(&source[cursor..start]);
        prepared.push_str(&replacement);
        if let Some(asset) = asset {
            assets.push(asset);
        }
        cursor = end;
    }
    prepared.push_str(&source[cursor..]);

    let (prepared, table_markers, unsupported_table_count) = extract_rtf_tables(&prepared);
    let mut markdown = extract_rtf_markdown_without_assets(&prepared);
    for (placeholder, table) in table_markers {
        markdown = markdown.replace(&placeholder, &table);
    }
    for (placeholder, link) in links {
        markdown = markdown.replace(&placeholder, &link);
    }
    for (placeholder, marker) in list_markers {
        markdown = replace_rtf_list_marker(&markdown, &placeholder, &marker);
    }
    for asset in &assets {
        let reference = format!(
            "![Illustration](attachments/scrivener-{}.{})",
            asset.sha256, asset.extension
        );
        markdown = markdown.replace(&asset.placeholder, &reference);
    }
    (
        markdown,
        assets,
        unsupported_picture_count,
        unsupported_link_count,
        unsupported_table_count,
    )
}

fn replace_rtf_list_marker(source: &str, placeholder: &str, marker: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    while let Some(relative) = source[cursor..].find(placeholder) {
        let start = cursor + relative;
        output.push_str(&source[cursor..start]);
        output.push_str(marker);
        cursor = start + placeholder.len();
        // RTF list paragraphs carry alignment tabs and indentation after the
        // marker. They are layout metadata, not a Markdown code-block indent.
        while source[cursor..]
            .chars()
            .next()
            .is_some_and(|character| matches!(character, ' ' | '\t'))
        {
            cursor += source[cursor..].chars().next().unwrap().len_utf8();
        }
    }
    output.push_str(&source[cursor..]);
    output
}

fn rtf_control_at(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    if bytes.get(start) != Some(&b'\\') {
        return None;
    }
    let next = *bytes.get(start + 1)?;
    if next.is_ascii_alphabetic() {
        let word_start = start + 1;
        let mut end = word_start;
        while bytes.get(end).is_some_and(u8::is_ascii_alphabetic) {
            end += 1;
        }
        let word = std::str::from_utf8(&bytes[word_start..end])
            .ok()?
            .to_owned();
        if bytes.get(end) == Some(&b'-') {
            end += 1;
        }
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if bytes.get(end) == Some(&b' ') {
            end += 1;
        }
        Some((word, end))
    } else if next == b'\'' {
        Some(("'".into(), (start + 4).min(bytes.len())))
    } else {
        Some(((next as char).to_string(), (start + 2).min(bytes.len())))
    }
}

fn find_rtf_control(source: &str, from: usize, name: &str) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut index = from;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && let Some((word, end)) = rtf_control_at(bytes, index)
        {
            if word == name {
                return Some((index, end));
            }
            index = end;
        } else {
            index += 1;
        }
    }
    None
}

fn rtf_gap_has_visible_text(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && let Some((word, end)) = rtf_control_at(bytes, index)
        {
            // RTF control symbols (escaped punctuation, nonbreaking spaces,
            // and hex escapes) represent visible characters. Control words
            // between adjacent rows are formatting instructions.
            if word.len() == 1 || word == "'" {
                return true;
            }
            index = end;
        } else if !matches!(
            bytes[index],
            b'{' | b'}' | b';' | b' ' | b'\t' | b'\r' | b'\n'
        ) {
            return true;
        } else {
            index += 1;
        }
    }
    false
}

/// Extracts simple rectangular RTF tables as portable Markdown pipe tables.
/// RTF row and cell controls are structural; treating their tabs and row
/// indentation as prose can turn ordinary table content into code blocks.
fn extract_rtf_tables(source: &str) -> (String, Vec<(String, String)>, usize) {
    let mut output = String::with_capacity(source.len());
    let mut markers = Vec::new();
    let mut cursor = 0usize;
    let mut search_from = 0usize;
    let mut table_index = 0usize;
    let mut unsupported_table_count = 0usize;
    while let Some((table_start, _)) = find_rtf_control(source, search_from, "trowd") {
        let mut row_start = table_start;
        let mut rows = Vec::<Vec<String>>::new();
        let table_end;
        loop {
            let Some((row_end, row_control_end)) = find_rtf_control(source, row_start, "row")
            else {
                // A malformed or unsupported table must remain visible to the
                // normal parser rather than silently deleting its source.
                rows.clear();
                table_end = table_start;
                unsupported_table_count = unsupported_table_count.saturating_add(1);
                break;
            };
            let mut cells = Vec::new();
            let mut cell_cursor = row_start;
            let mut cell_search = row_start;
            while let Some((cell_start, cell_end)) = find_rtf_control(source, cell_search, "cell") {
                if cell_start >= row_end {
                    break;
                }
                let cell = extract_rtf_markdown_without_assets(&source[cell_cursor..cell_start]);
                let cell = cell.replace('\n', " <br /> ").replace('|', "\\|");
                cells.push(cell.trim().to_owned());
                cell_cursor = cell_end;
                cell_search = cell_end;
            }
            if cells.is_empty() {
                rows.clear();
                table_end = table_start;
                unsupported_table_count = unsupported_table_count.saturating_add(1);
                break;
            }
            rows.push(cells);

            let Some((next_row_start, _)) = find_rtf_control(source, row_control_end, "trowd")
            else {
                table_end = row_control_end;
                break;
            };
            if rtf_gap_has_visible_text(&source[row_control_end..next_row_start]) {
                table_end = row_control_end;
                break;
            }
            row_start = next_row_start;
        }

        if rows.is_empty() || table_end <= table_start {
            search_from = table_start + "\\trowd".len();
            continue;
        }
        let table_source = &source[table_start..table_end];
        let row_width = rows.first().map(Vec::len).unwrap_or(0);
        if rows.iter().any(|row| row.len() != row_width)
            || ["\\clmgf", "\\clmrg", "\\clvmgf", "\\clvmrg"]
                .iter()
                .any(|control| table_source.contains(control))
            || table_source.contains("\\itap2")
        {
            unsupported_table_count = unsupported_table_count.saturating_add(1);
        }
        output.push_str(&source[cursor..table_start]);
        let placeholder = format!("WIDESCRIVENTABLE{table_index}END");
        table_index += 1;
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut table = String::new();
        for (row_index, mut row) in rows.into_iter().enumerate() {
            row.resize(width, String::new());
            table.push('|');
            for cell in row {
                table.push(' ');
                table.push_str(cell.trim());
                table.push_str(" |");
            }
            table.push('\n');
            if row_index == 0 {
                table.push('|');
                for _ in 0..width {
                    table.push_str(" --- |");
                }
                table.push('\n');
            }
        }
        markers.push((placeholder.clone(), table));
        output.push_str("\\par ");
        output.push_str(&placeholder);
        output.push_str(" \\par ");
        cursor = table_end;
        search_from = table_end;
    }
    output.push_str(&source[cursor..]);
    (output, markers, unsupported_table_count)
}

fn extract_rtf_markdown(source: &str) -> String {
    extract_rtf_markdown_with_assets(source).0
}

fn extract_rtf_markdown_without_assets(source: &str) -> String {
    #[derive(Clone, Copy, Eq, PartialEq)]
    struct Style {
        bold: bool,
        italic: bool,
        underline: bool,
        strike: bool,
        skip: bool,
        unicode_fallback: usize,
        code_page: u32,
    }

    impl Default for Style {
        fn default() -> Self {
            Self {
                bold: false,
                italic: false,
                underline: false,
                strike: false,
                skip: false,
                unicode_fallback: 1,
                code_page: 1252,
            }
        }
    }

    fn rtf_encoding(code_page: u32) -> &'static encoding_rs::Encoding {
        match code_page {
            65001 => encoding_rs::UTF_8,
            932 => encoding_rs::SHIFT_JIS,
            936 => encoding_rs::GBK,
            949 => encoding_rs::EUC_KR,
            950 => encoding_rs::BIG5,
            _ => {
                let label = format!("windows-{code_page}");
                encoding_rs::Encoding::for_label(label.as_bytes())
                    .unwrap_or(encoding_rs::WINDOWS_1252)
            }
        }
    }

    fn skip_rtf_fallback(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, count: usize) {
        let mut skipped = 0;
        while skipped < count {
            match chars.next() {
                None => break,
                Some('{' | '}') => continue,
                Some('\\') => match chars.next() {
                    None => break,
                    Some('\'') => {
                        let _ = chars.next();
                        let _ = chars.next();
                        skipped += 1;
                    }
                    Some(symbol) if !symbol.is_ascii_alphabetic() => skipped += 1,
                    Some(first) => {
                        // A fallback can itself use an escaped RTF control. Consume
                        // that complete control token as one fallback character.
                        let _ = first;
                        while chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                            chars.next();
                        }
                        if chars
                            .peek()
                            .is_some_and(|c| *c == '-' || c.is_ascii_digit())
                        {
                            if chars.peek() == Some(&'-') {
                                chars.next();
                            }
                            while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                                chars.next();
                            }
                        }
                        if chars.peek() == Some(&' ') {
                            chars.next();
                        }
                        skipped += 1;
                    }
                },
                Some(_) => skipped += 1,
            }
        }
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
                    let Some(byte) = chars
                        .next()
                        .and_then(|c| c.to_digit(16))
                        .zip(chars.next().and_then(|c| c.to_digit(16)))
                        .map(|(high, low)| ((high << 4) | low) as u8)
                    else {
                        continue;
                    };
                    let mut bytes = vec![byte];
                    loop {
                        let mut probe = chars.clone();
                        if probe.next() != Some('\\') || probe.next() != Some('\'') {
                            break;
                        }
                        let Some(next_byte) = probe
                            .next()
                            .and_then(|c| c.to_digit(16))
                            .zip(probe.next().and_then(|c| c.to_digit(16)))
                            .map(|(high, low)| ((high << 4) | low) as u8)
                        else {
                            break;
                        };
                        bytes.push(next_byte);
                        chars = probe;
                    }
                    if !style.skip {
                        let (decoded, _, _) = rtf_encoding(style.code_page).decode(&bytes);
                        runs.extend(decoded.chars().map(|decoded| (decoded, style)));
                    }
                    continue;
                }
                if !next.is_ascii_alphabetic() {
                    if !style.skip {
                        match next {
                            '~' => runs.push(('\u{00a0}', style)),
                            '_' => runs.push(('\u{2011}', style)),
                            '-' => runs.push(('\u{00ad}', style)),
                            _ => {}
                        }
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
                    "plain" => {
                        // RTF \\plain resets character formatting at a run boundary.
                        // Keeping inherited emphasis here can wrap a list marker
                        // placeholder in Markdown emphasis and make the list
                        // marker cease to be a list marker.
                        style.bold = false;
                        style.italic = false;
                        style.underline = false;
                        style.strike = false;
                    }
                    "b" => style.bold = number != "0",
                    "i" => style.italic = number != "0",
                    "ul" | "uld" | "uldash" | "uldashd" | "uldashdd" | "uldb" | "ulhair"
                    | "ulth" | "ulthd" | "ulthdash" | "ulthdashd" | "ulthdashdd" | "ulthldash"
                    | "ulthldashd" | "ulthldashdd" | "ulthwave" | "ulw" | "ulwave" => {
                        style.underline = number != "0"
                    }
                    "ulnone" => style.underline = false,
                    "strike" | "strikethrough" => style.strike = number != "0",
                    "ansicpg" | "cpg" => {
                        if let Ok(code_page) = number.parse::<u32>() {
                            style.code_page = code_page;
                        }
                    }
                    "uc" => {
                        if let Ok(count) = number.parse::<usize>() {
                            style.unicode_fallback = count.min(32);
                        }
                    }
                    "par" | "line" if !style.skip => runs.push(('\n', Style::default())),
                    "tab" if !style.skip => runs.push(('\t', Style::default())),
                    "endash" if !style.skip => runs.push(('\u{2013}', style)),
                    "emdash" if !style.skip => runs.push(('\u{2014}', style)),
                    "lquote" if !style.skip => runs.push(('\u{2018}', style)),
                    "rquote" if !style.skip => runs.push(('\u{2019}', style)),
                    "ldblquote" if !style.skip => runs.push(('\u{201c}', style)),
                    "rdblquote" if !style.skip => runs.push(('\u{201d}', style)),
                    "bullet" if !style.skip => runs.push(('\u{2022}', style)),
                    "u" if !style.skip => {
                        if let Ok(code) = number.parse::<i32>() {
                            runs.push((char::from_u32(code as u16 as u32).unwrap_or('�'), style));
                            skip_rtf_fallback(&mut chars, style.unicode_fallback);
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
        let characters = text.chars().collect::<Vec<_>>();
        let leading_end = characters
            .iter()
            .take_while(|character| character.is_whitespace())
            .count();
        let trailing_start = characters
            .iter()
            .rev()
            .take_while(|character| character.is_whitespace())
            .count()
            .min(characters.len() - leading_end);
        let trailing_start = characters.len() - trailing_start;
        let (mut leading, mut escaped, mut trailing) = (
            String::new(),
            String::with_capacity(text.len()),
            String::new(),
        );
        for (index, character) in characters.iter().copied().enumerate() {
            let ordered_list_marker = character == '.'
                && index + 1 < characters.len()
                && characters[index + 1].is_whitespace()
                && characters[..index].iter().all(char::is_ascii_digit)
                && index > 0;
            let escaped_character = if matches!(
                character,
                '\\' | '`'
                    | '*'
                    | '_'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '<'
                    | '>'
                    | '~'
                    | '|'
                    | '#'
                    | '+'
                    | '-'
                    | '!'
                    | '^'
            ) || ordered_list_marker
            {
                format!("\\{character}")
            } else {
                character.to_string()
            };
            if index < leading_end {
                leading.push_str(&escaped_character);
            } else if index >= trailing_start {
                trailing.push_str(&escaped_character);
            } else {
                escaped.push_str(&escaped_character);
            }
        }
        let mut formatted = match (style.bold, style.italic, style.strike) {
            (true, true, true) if !escaped.is_empty() => format!("~~***{escaped}***~~"),
            (true, true, false) if !escaped.is_empty() => format!("***{escaped}***"),
            (true, false, true) if !escaped.is_empty() => format!("~~**{escaped}**~~"),
            (false, true, true) if !escaped.is_empty() => format!("~~*{escaped}*~~"),
            (true, false, false) if !escaped.is_empty() => format!("**{escaped}**"),
            (false, true, false) if !escaped.is_empty() => format!("*{escaped}*"),
            (false, false, true) if !escaped.is_empty() => format!("~~{escaped}~~"),
            _ => escaped,
        };
        if style.underline && !formatted.is_empty() {
            formatted = format!("[{formatted}]{{.underline}}");
        }
        output.push_str(&leading);
        output.push_str(&formatted);
        output.push_str(&trailing);
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
    let output = output.trim();
    let mut prose = String::with_capacity(output.len());
    let mut characters = output.chars().peekable();
    let mut at_paragraph_start = true;
    while let Some(character) = characters.next() {
        if at_paragraph_start && matches!(character, ' ' | '\t') {
            let mut indentation = vec![character];
            while characters
                .peek()
                .is_some_and(|next| matches!(next, ' ' | '\t'))
            {
                indentation.push(characters.next().expect("peeked indentation character"));
            }
            let has_tab = indentation.contains(&'\t');
            let code_indentation = indentation.len() >= 4;
            if has_tab || code_indentation {
                // Scrivener commonly exports a paragraph indent as a space
                // followed by an RTF tab. A raw tab or four spaces at the
                // start of Markdown is a code-block marker. Preserve the
                // visible indent using non-breaking spaces instead.
                for whitespace in indentation {
                    prose.extend(std::iter::repeat_n(
                        '\u{00a0}',
                        if whitespace == '\t' { 4 } else { 1 },
                    ));
                }
            } else {
                prose.extend(indentation);
            }
            at_paragraph_start = false;
            continue;
        }
        prose.push(character);
        if character == '\n' {
            at_paragraph_start = true;
        } else if character != '\r' {
            at_paragraph_start = false;
        }
    }
    prose
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
    fn ambiguous_scrivener_binder_content_defaults_to_private_research() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("Private Material.scriv");
        let private_id = "private-text";
        let private_dir = source.join("Files/Data").join(private_id);
        fs::create_dir_all(&private_dir).unwrap();
        fs::write(
            source.join("Private Material.scrivx"),
            r#"<ScrivenerProject Version="2.0"><Binder>
              <BinderItem UUID="draft-root" Type="DraftFolder"><Title>Internal title</Title><Children/></BinderItem>
              <BinderItem UUID="private-folder" Type="Folder"><Title>Planning material</Title><Children>
                <BinderItem UUID="private-text" Type="Text"><Title>Private notes</Title><Children/></BinderItem>
              </Children></BinderItem>
            </Binder></ScrivenerProject>"#,
        )
        .unwrap();
        fs::write(
            private_dir.join("content.rtf"),
            r#"{\rtf1\ansi PRIVATEPLANNINGSENTINEL}"#,
        )
        .unwrap();

        let inventory = inspect_scrivener_project(&source).unwrap();
        let private = inventory
            .items
            .iter()
            .find(|item| item.source_document_id == private_id)
            .unwrap();
        assert_eq!(private.area, "unknown");
        assert!(
            inventory
                .warnings
                .iter()
                .any(|warning| warning.contains("private research"))
        );

        let plan = ScrivenerImportPlan::proposed(&inventory);
        assert!(
            plan.decisions
                .iter()
                .filter(|decision| { decision.source_document_id.starts_with("private-") })
                .all(|decision| decision.destination_kind == ScrivenerDestinationKind::Research)
        );
        assert_eq!(plan.preview(&inventory).unwrap().research_items, 2);
    }

    #[test]
    fn org_hierarchy_preview_keeps_heading_depth_prose_order_and_unsupported_features() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes.org");
        let contents = "Before headings.\n\n* Part One\nOpening *bright* prose.\n** TODO First scene\n- Keep this list.\nA [[https://example.test/path][useful link]].\n#+begin_src rust\nprintln!(\"kept\");\n#+end_src\n:PROPERTIES:\n:ID: scene-1\n:END:\n*** Later beat\nA final paragraph.";
        fs::write(&source, contents).unwrap();
        let original = fs::read(&source).unwrap();

        let preview = inspect_text_document(&source, OrgImportMode::HeadingHierarchy).unwrap();

        assert_eq!(preview.format, TextDocumentFormat::Org);
        assert_eq!(preview.units.len(), 4);
        assert_eq!(preview.units[0].title, "Preamble");
        assert_eq!(preview.units[1].heading_level, Some(1));
        assert_eq!(preview.units[1].title, "Part One");
        assert!(preview.units[1].body_markdown.contains("**bright**"));
        assert_eq!(preview.units[2].heading_level, Some(2));
        assert_eq!(preview.units[2].title, "TODO First scene");
        assert!(preview.units[2].body_markdown.contains("- Keep this list."));
        assert!(
            preview.units[2]
                .body_markdown
                .contains("[useful link](https://example.test/path)")
        );
        assert!(
            preview.units[2]
                .body_markdown
                .contains("println!(\"kept\");")
        );
        assert_eq!(preview.units[3].heading_level, Some(3));
        assert_eq!(preview.units[4 - 1].title, "Later beat");
        assert!(
            preview
                .unsupported
                .iter()
                .any(|item| item.contains("Org block needs review"))
        );
        assert!(
            preview
                .unsupported
                .iter()
                .any(|item| item.contains("Org drawer/property"))
        );
        assert!(
            preview
                .unsupported
                .iter()
                .any(|item| item.contains("task state"))
        );
        assert_eq!(fs::read(source).unwrap(), original);
    }

    #[test]
    fn plain_text_preview_preserves_utf8_paragraphs_and_crlf_and_rejects_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("draft.txt");
        let contents = "Élodie wrote a line.\r\n\r\n안녕하세요, world.\r\n";
        fs::write(&source, contents.as_bytes()).unwrap();
        let preview = inspect_text_document(&source, OrgImportMode::SingleDocument).unwrap();
        assert_eq!(preview.format, TextDocumentFormat::PlainText);
        assert_eq!(preview.units.len(), 1);
        assert_eq!(preview.units[0].body_markdown, contents);

        fs::write(&source, [0xf0, 0x28, 0x8c, 0x28]).unwrap();
        assert!(matches!(
            inspect_text_document(&source, OrgImportMode::SingleDocument),
            Err(ImportError::InvalidEncoding(_))
        ));
    }

    #[test]
    fn manuscript_inspector_accepts_markdown_and_org_without_modifying_sources() {
        let dir = tempfile::tempdir().unwrap();
        let markdown = dir.path().join("draft.md");
        let org = dir.path().join("draft.org");
        fs::write(&markdown, "# Opening\n\nMarkdown prose.").unwrap();
        fs::write(&org, "* Opening\n\nOrg prose with /emphasis/.").unwrap();
        let markdown_staging = inspect_manuscript(&markdown).unwrap();
        let org_staging = inspect_manuscript(&org).unwrap();
        assert_eq!(markdown_staging.source_type, SourceType::Markdown);
        assert!(
            markdown_staging.units[0]
                .body_markdown
                .contains("Markdown prose.")
        );
        assert_eq!(org_staging.source_type, SourceType::Org);
        assert!(org_staging.units[0].body_markdown.contains("*emphasis*"));
        assert_eq!(
            fs::read_to_string(markdown).unwrap(),
            "# Opening\n\nMarkdown prose."
        );
        assert_eq!(
            fs::read_to_string(org).unwrap(),
            "* Opening\n\nOrg prose with /emphasis/."
        );
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
        archive_with_members(
            &docx,
            &[
                (
                    "word/document.xml",
                    "<w:document><w:body><w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>Chapter One</w:t></w:r></w:p><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Élodie &amp; Sam.</w:t><w:tab/><w:t>Still here.</w:t><w:br/><w:t>New line.</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>italic text</w:t></w:r><w:r><w:rPr><w:strike/></w:rPr><w:t>struck text</w:t></w:r><w:r><w:rPr><w:u w:val=\"single\"/></w:rPr><w:t>underlined text</w:t></w:r><w:hyperlink r:id=\"rId1\"><w:r><w:t>External link</w:t></w:r></w:hyperlink><w:hyperlink w:anchor=\"chapter-one\"><w:r><w:t>Internal link</w:t></w:r></w:hyperlink><w:hyperlink r:id=\"rId2\"><w:r><w:t>Local file link</w:t></w:r></w:hyperlink></w:p></w:body></w:document>",
                ),
                (
                    "word/_rels/document.xml.rels",
                    "<Relationships><Relationship Id=\"rId1\" Target=\"https://example.test/a b?q=one two\" TargetMode=\"External\"/><Relationship Id=\"rId2\" Target=\"file:///private/document.docx\" TargetMode=\"External\"/></Relationships>",
                ),
            ],
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
            staged.units[0]
                .body_markdown
                .contains("[External link](<https://example.test/a%20b?q=one%20two>)")
        );
        assert!(
            staged.units[0]
                .body_markdown
                .contains("[Internal link](<#chapter-one>)")
        );
        assert!(staged.units[0].body_markdown.contains("Local file link"));
        assert!(
            staged
                .unsupported
                .iter()
                .any(|item| item.contains("unsupported destination"))
        );
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
    fn scrivener_rtf_unicode_fallbacks_follow_uc_and_do_not_leak_hex_digits() {
        let source = r#"{\rtf1\ansi\ansicpg1252\uc1 It\u8217\'92s \u8220\'93quoted\u8221\'94, \u233\'e9, \u169\'a9. Digits 92 93 94 remain.}"#;
        let markdown = extract_rtf_markdown(source);
        assert_eq!(markdown, "It’s “quoted”, é, ©. Digits 92 93 94 remain.");
    }

    #[test]
    fn scrivener_rtf_honours_zero_and_multiple_unicode_fallback_counts() {
        let source = r#"{\rtf1\ansi\uc0\u233X {\uc2\u233\'3f\'3f} \uc1\u8217’}"#;
        let markdown = extract_rtf_markdown(source);
        assert_eq!(markdown, "éX é ’");
    }

    #[test]
    fn scrivener_rtf_decodes_declared_codepages_and_mixed_unicode_runs() {
        let source =
            r#"{\rtf1\ansi\ansicpg65001 UTF-8 \'c3\'a9; {\cpg1252 \'e9} \u54620?\u44544?}"#;
        let markdown = extract_rtf_markdown(source);
        assert_eq!(markdown, r"UTF\-8 é; é 한글");
    }

    #[test]
    fn scrivener_rtf_named_punctuation_controls_preserve_dashes_quotes_and_special_hyphens() {
        let source = r#"{\rtf1\ansi 1185\endash 1333; em\emdash dash, \lquote single\rquote  and \ldblquote double\rdblquote. no\~break, non\_break, soft\-hyphen.}"#;
        let markdown = extract_rtf_markdown(source);
        assert!(markdown.contains("1185–1333"), "{markdown:?}");
        assert!(markdown.contains("em—dash"), "{markdown:?}");
        assert!(markdown.contains("‘single’ and “double”"), "{markdown:?}");
        assert!(markdown.contains("no\u{00a0}break"), "{markdown:?}");
        assert!(markdown.contains("non\u{2011}break"), "{markdown:?}");
        assert!(markdown.contains("soft\u{00ad}hyphen"), "{markdown:?}");
    }

    #[test]
    fn scrivener_rtf_emphasis_keeps_surrounding_whitespace_outside_markdown_delimiters() {
        let source = r#"{\rtf1\ansi Start{\b  Travel: }next and {\i phrase }ends.}"#;
        let markdown = extract_rtf_markdown(source);
        assert_eq!(markdown, "Start **Travel:** next and *phrase* ends.");
        assert!(!markdown.contains("** Travel: **"));
    }

    #[test]
    fn scrivener_rtf_underline_uses_semantic_pandoc_span_and_composes_with_emphasis() {
        let source = r#"{\rtf1\ansi Start {\ul underlined words\ulnone} and {\b\ul bold underlined\b0\ul0}.}"#;
        let markdown = extract_rtf_markdown(source);
        assert_eq!(
            markdown,
            "Start [underlined words]{.underline} and [**bold underlined**]{.underline}."
        );
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
    fn scrivener_paragraph_indent_does_not_become_a_markdown_code_block() {
        let source = r#"{\rtf1\ansi First paragraph.\par \tab An indented ordinary paragraph with \b emphasis\b0.\par     Four-space indented prose.\par Next line with\tab an interior tab.}"#;
        let markdown = extract_rtf_markdown(source);
        assert!(
            markdown.contains("\n\n\u{00a0}\u{00a0}\u{00a0}\u{00a0}An indented"),
            "{markdown:?}"
        );
        assert!(
            markdown.contains("\n\n\u{00a0}\u{00a0}\u{00a0}\u{00a0}Four\\-space"),
            "{markdown:?}"
        );
        assert!(markdown.contains("*emphasis*"), "{markdown:?}");
        assert!(
            markdown.contains("Next line with\tan interior tab."),
            "{markdown:?}"
        );
    }

    #[test]
    fn scrivener_rtf_tables_preserve_rows_and_cells_as_markdown_tables() {
        let source = r#"{\rtf1\ansi Before table.\par\trowd\trgaph0\cellx2000\cellx4000\pard\intbl Label\cell\pard\intbl Value\cell\row\trowd\trgaph0\cellx2000\cellx4000\pard\intbl First\cell\pard\intbl Second\cell\row\pard After table.}"#;
        let markdown = extract_rtf_markdown(source);
        assert!(markdown.contains("| Label | Value |"), "{markdown:?}");
        assert!(markdown.contains("| First | Second |"), "{markdown:?}");
        assert!(markdown.contains("| --- | --- |"), "{markdown:?}");
        assert!(markdown.contains("Before table."), "{markdown:?}");
        assert!(markdown.contains("After table."), "{markdown:?}");
        assert!(!markdown.contains("\\trowd"), "{markdown:?}");
    }

    #[test]
    fn scrivener_rtf_merged_table_cells_are_reported_for_review() {
        let source = r#"{\rtf1\ansi\trowd\clmgf\cellx4000\pard\intbl Combined\cell\row}"#;
        let (markdown, _, _, _, unsupported_tables) = extract_rtf_markdown_with_assets(source);
        assert!(markdown.contains("Combined"), "{markdown:?}");
        assert_eq!(unsupported_tables, 1);
    }

    #[test]
    fn scrivener_rtf_irregular_table_rows_are_reported_for_review() {
        let source = r#"{\rtf1\ansi\trowd\cellx2000\cellx4000\pard\intbl A\cell\pard\intbl B\cell\row\trowd\cellx2000\pard\intbl C\cell\row}"#;
        let (markdown, _, _, _, unsupported_tables) = extract_rtf_markdown_with_assets(source);
        assert!(markdown.contains("A"), "{markdown:?}");
        assert!(markdown.contains("B"), "{markdown:?}");
        assert!(markdown.contains("C"), "{markdown:?}");
        assert_eq!(unsupported_tables, 1);
    }

    #[test]
    fn scrivener_rtf_extracts_png_payloads_as_stable_markdown_assets() {
        let source = r#"{\rtf1\ansi Before\par {\*\shppict{\pict\pngblip\picw1\pich1 89504e470d0a1a0a}}\par After}"#;
        let (markdown, assets, unsupported, unsupported_links, unsupported_tables) =
            extract_rtf_markdown_with_assets(source);
        assert_eq!(unsupported, 0);
        assert_eq!(unsupported_links, 0);
        assert_eq!(unsupported_tables, 0);
        assert_eq!(assets.len(), 1);
        let asset = &assets[0];
        assert_eq!(asset.media_type, "image/png");
        assert_eq!(asset.extension, "png");
        assert_eq!(asset.bytes, b"\x89PNG\r\n\x1a\n");
        assert_eq!(asset.sha256, format!("{:x}", Sha256::digest(&asset.bytes)));
        assert!(
            markdown.contains(&format!(
                "![Illustration](attachments/scrivener-{}.png)",
                asset.sha256
            )),
            "{markdown:?}"
        );
        let inventory_json = serde_json::to_string(asset).unwrap();
        assert!(inventory_json.contains(&asset.sha256));
        assert!(!inventory_json.contains("137,80,78,71"));
    }

    #[test]
    fn scrivener_rtf_reports_unsupported_or_malformed_picture_data() {
        let source = r#"{\rtf1\ansi Text {\pict\emfblip 00112233} tail}"#;
        let (markdown, assets, unsupported, unsupported_links, unsupported_tables) =
            extract_rtf_markdown_with_assets(source);
        assert!(assets.is_empty());
        assert_eq!(unsupported, 1);
        assert_eq!(unsupported_links, 0);
        assert_eq!(unsupported_tables, 0);
        assert!(!markdown.contains("00112233"));
    }

    #[test]
    fn scrivener_shppict_control_is_not_mistaken_for_pict_and_does_not_discard_following_text() {
        let source = r#"{\rtf1\ansi Before\par {\*\shppict unsupported shape}\par {\listtext\f0\u9679?}\tab Preserved item\par After.}"#;
        let (markdown, assets, unsupported, _, _) = extract_rtf_markdown_with_assets(source);
        assert!(assets.is_empty());
        assert_eq!(unsupported, 0);
        assert!(markdown.contains("- Preserved item"), "{markdown:?}");
        assert!(markdown.contains("After."), "{markdown:?}");
    }

    #[test]
    fn scrivener_rtf_hyperlink_fields_preserve_visible_text_and_safe_destinations() {
        let source = r#"{\rtf1\ansi Read {\field{\*\fldinst HYPERLINK "https://example.org/a(b)"}{\fldrslt helpful page}} now}"#;
        let (markdown, assets, unsupported_pictures, unsupported_links, _) =
            extract_rtf_markdown_with_assets(source);
        assert!(assets.is_empty());
        assert_eq!(unsupported_pictures, 0);
        assert_eq!(unsupported_links, 0);
        assert!(markdown.contains("[helpful page](https://example.org/a%28b%29)"));

        let unsafe_target = r#"{\rtf1\ansi {\field{\*\fldinst HYPERLINK "javascript:alert(1)"}{\fldrslt visible}}}"#;
        let (safe_markdown, _, _, unsupported_links, _) =
            extract_rtf_markdown_with_assets(unsafe_target);
        assert_eq!(unsupported_links, 1);
        assert!(!safe_markdown.contains("javascript:"));
        assert!(safe_markdown.contains("visible"));
    }

    #[test]
    fn scrivener_rtf_prose_escapes_markdown_syntax_without_losing_literal_punctuation() {
        let source = r#"{\rtf1\ansi # Not a heading\par - Not a list item\par > Not a quotation\par `Not code`\par 1. Not a numbered list\par A | literal | bar\par Keep [brackets] and (parentheses).}"#;
        let markdown = extract_rtf_markdown(source);
        assert!(markdown.contains(r"\# Not a heading"));
        assert!(markdown.contains(r"\- Not a list item"));
        assert!(markdown.contains(r"\> Not a quotation"));
        assert!(markdown.contains(r"\`Not code\`"));
        assert!(markdown.contains(r"1\. Not a numbered list"));
        assert!(markdown.contains(r"A \| literal \| bar"));
        assert!(markdown.contains(r"Keep \[brackets\] and (parentheses)."));
    }

    #[test]
    fn scrivener_rtf_bullet_list_markers_become_markdown_list_items() {
        let source = r#"{\rtf1\ansi Before.\par {\listtext\f0\u9679?}\tab First item\par {\listtext\f0\u9679?}\tab Second item\par After.}"#;
        let markdown = extract_rtf_markdown(source);
        assert!(
            markdown.contains("- First item"),
            "unexpected RTF list output: {markdown:?}"
        );
        assert!(
            markdown.contains("- Second item"),
            "unexpected RTF list output: {markdown:?}"
        );
        assert!(!markdown.contains("•?"));
    }

    #[test]
    fn scrivener_rtf_list_indentation_does_not_create_nested_code_blocks() {
        let source = r#"{\rtf1\ansi \pard\li720{\listtext\f0\u9679?}\tab     Ordinary list prose\par \pard\li720{\listtext\f0\u9679?}\tab     Second item}"#;
        let markdown = extract_rtf_markdown(source);
        assert!(markdown.contains("- Ordinary list prose"), "{markdown:?}");
        assert!(markdown.contains("- Second item"), "{markdown:?}");
        assert!(!markdown.contains("-     "), "{markdown:?}");
    }

    #[test]
    fn scrivener_rtf_list_marker_with_scrivener_formatting_groups_becomes_list_item() {
        let source = r#"{\rtf1\ansi\i1 Styled opening.\par\plain{\listtext\f0\fs24\b0\i0\loch\af0\hich\af0\dbch\af0\uc1\u9679?}{\f0\fs24\b0\i0\cf1 First item}\par After.}"#;
        let markdown = extract_rtf_markdown(source);
        assert!(
            markdown.lines().any(|line| line == "- First item"),
            "unexpected RTF list output: {markdown:?}"
        );
        assert!(
            markdown.contains("After."),
            "unexpected RTF output: {markdown:?}"
        );
    }

    #[test]
    fn scrivener_rtf_numbered_list_markers_retain_numbering() {
        let source = r#"{\rtf1\ansi {\listtext\f0 1.}\tab First item\par {\listtext\f0 2.}\tab Second item\par}"#;
        let markdown = extract_rtf_markdown(source);
        assert!(
            markdown.contains("1. First item"),
            "unexpected RTF list output: {markdown:?}"
        );
        assert!(
            markdown.contains("2. Second item"),
            "unexpected RTF list output: {markdown:?}"
        );
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
              <BinderItem UUID="chapter-uuid" Type="DraftFolder"><Title>Chapter One</Title><Children>
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

        let plan = ScrivenerImportPlan::proposed(&inventory);
        let preview = plan.preview(&inventory).unwrap();
        assert_eq!(preview.total_items, 3);
        assert_eq!(preview.research_items, 1);
        assert!(preview.unresolved_source_ids.is_empty());
        assert_eq!(
            plan.decisions
                .iter()
                .find(|decision| decision.source_document_id == "chapter-uuid")
                .unwrap()
                .destination_kind,
            ScrivenerDestinationKind::Section
        );
        assert_eq!(
            plan.decisions
                .iter()
                .find(|decision| decision.source_document_id == "scene-uuid")
                .unwrap()
                .destination_kind,
            ScrivenerDestinationKind::Chapter
        );
        assert!(!preview.unresolved_source_ids.contains(&"scene-uuid".into()));
        plan.validate_for_import(&inventory).unwrap();
    }

    #[test]
    fn scrivener_compile_settings_and_binder_flags_are_scoped_to_their_owner() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Compile Metadata.scriv");
        let settings = project.join("Settings");
        std::fs::create_dir_all(&settings).unwrap();
        std::fs::write(
            project.join("Compile Metadata.scrivx"),
            r#"<ScrivenerProject Version="2.0"><Binder>
              <BinderItem UUID="folder" Type="Folder"><Title>Organisational heading</Title>
                <MetaData><IncludeInCompile>No</IncludeInCompile><SectionType>type-folder</SectionType></MetaData>
                <Children><BinderItem UUID="child" Type="Text"><Title>Chapter prose</Title>
                  <MetaData><IncludeInCompile>Yes</IncludeInCompile><SectionType>type-chapter</SectionType></MetaData>
                  <Children/></BinderItem></Children></BinderItem>
            </Binder><LevelTypes><Folders><Type>type-folder</Type></Folders>
              <Containers><Type>type-chapter</Type></Containers>
              <Files><Type>type-chapter</Type></Files></LevelTypes>
            <SectionTypes><TypeDefinitions>
              <Type ID="type-folder">Internal heading role</Type>
              <Type ID="type-chapter">Numbered chapter role</Type>
            </TypeDefinitions></SectionTypes></ScrivenerProject>"#,
        )
        .unwrap();
        std::fs::write(
            settings.join("compile.xml"),
            r#"<CompileSettings><ProjectSettings><Content Scope="Included"/>
              <MetaData><ProjectTitle>Imported title</ProjectTitle><Authors>Author name</Authors></MetaData>
            </ProjectSettings><FormatSettings>
              <Format ID="DEFAULT"><SectionLayouts><Type ID="type-folder">Hidden Section</Type>
                <Type ID="type-chapter">Chapter Heading</Type></SectionLayouts></Format>
              <Format ID="retailer"><FrontAndBackMatter>
                <DefaultFrontMatter Add="Yes">front-id</DefaultFrontMatter>
                <DefaultBackMatter Add="No">back-id</DefaultBackMatter>
              </FrontAndBackMatter></Format>
            </FormatSettings></CompileSettings>"#,
        )
        .unwrap();
        std::fs::create_dir_all(settings.join("Compile Formats")).unwrap();
        std::fs::write(
            settings.join("Compile Formats/retailer.scrformat"),
            r#"<CompileFormat ID="retailer"><Layouts><Layout ID="layout-chapter" Name="Chapter title">
              <Include Titles="No" Text="Yes"/><Titles><Prefix>Chapter &lt;$t&gt;</Prefix></Titles>
              <Separators><Between Type="Line">* * *</Between></Separators>
            </Layout></Layouts></CompileFormat>"#,
        )
        .unwrap();

        let inventory = inspect_scrivener_project(&project).unwrap();
        assert_eq!(inventory.items[0].include_in_compile, Some(false));
        assert_eq!(
            inventory.items[0].section_type_id.as_deref(),
            Some("type-folder")
        );
        assert_eq!(inventory.items[1].include_in_compile, Some(true));
        assert_eq!(
            inventory.items[1].section_type_id.as_deref(),
            Some("type-chapter")
        );
        assert_eq!(
            inventory.section_types.get("type-folder").unwrap(),
            "Internal heading role"
        );
        assert_eq!(
            inventory.section_types.get("type-chapter").unwrap(),
            "Numbered chapter role"
        );
        assert_eq!(inventory.level_types.get("folders").unwrap(), "type-folder");
        assert_eq!(inventory.level_types.get("files").unwrap(), "type-chapter");
        let mut defaulted_child = inventory.items[1].clone();
        defaulted_child.section_type_id = None;
        assert_eq!(
            inventory.effective_section_type_id(&defaulted_child),
            Some("type-chapter")
        );
        defaulted_child.section_type_id = Some("explicit-override".into());
        assert_eq!(
            inventory.effective_section_type_id(&defaulted_child),
            Some("explicit-override")
        );
        assert_eq!(
            inventory.project_metadata.get("ProjectTitle").unwrap(),
            "Imported title"
        );
        let plan = ScrivenerImportPlan::proposed(&inventory);
        let excluded_folder = plan
            .decisions
            .iter()
            .find(|decision| decision.source_document_id == "folder")
            .unwrap();
        assert_eq!(
            excluded_folder.destination_kind,
            ScrivenerDestinationKind::Research
        );
        assert!(excluded_folder.include_in_import);
        assert_eq!(
            inventory.project_metadata.get("Authors").unwrap(),
            "Author name"
        );
        let compile_settings = inventory.compile_settings.unwrap();
        let formats = compile_settings.formats;
        assert_eq!(formats.len(), 2);
        assert_eq!(
            formats[0].section_layouts.get("type-folder").unwrap(),
            "Hidden Section"
        );
        assert_eq!(
            formats[0].section_layouts.get("type-chapter").unwrap(),
            "Chapter Heading"
        );
        assert_eq!(
            formats[1].default_front_matter_id.as_deref(),
            Some("front-id")
        );
        assert_eq!(formats[1].add_default_front_matter, Some(true));
        assert_eq!(
            formats[1].default_back_matter_id.as_deref(),
            Some("back-id")
        );
        assert_eq!(formats[1].add_default_back_matter, Some(false));
        let layout = formats[1].layouts.get("layout-chapter").unwrap();
        assert_eq!(layout.name.as_deref(), Some("Chapter title"));
        assert_eq!(layout.include_titles, Some(false));
        assert_eq!(layout.include_text, Some(true));
        assert_eq!(layout.title_prefix.as_deref(), Some("Chapter <$t>"));
        assert_eq!(layout.between_separator_kind.as_deref(), Some("Line"));
        assert_eq!(layout.between_separator_text.as_deref(), Some("* * *"));
        assert_eq!(
            compile_settings.available_layouts["retailer"]["layout-chapter"],
            *layout
        );
    }

    #[test]
    fn format_selected_front_and_back_matter_receive_typed_import_destinations() {
        let make_item = |id: &str, parent: Option<&str>| ScrivenerBinderItem {
            source_document_id: id.into(),
            parent_source_id: parent.map(str::to_owned),
            position: 0,
            title: id.into(),
            type_code: Some("Text".into()),
            area: "unknown".into(),
            include_in_compile: Some(true),
            section_type_id: None,
            synopsis: None,
            notes_markdown: None,
            content_markdown: Some(format!("Content for {id}")),
            embedded_assets: Vec::new(),
            fields: BTreeMap::new(),
            unsupported: Vec::new(),
        };
        let inventory = ScrivenerProjectInventory {
            source_path: "fixture.scriv".into(),
            source_fingerprint: "fingerprint".into(),
            title: "Book".into(),
            format_version: Some("2.0".into()),
            creator: None,
            project_metadata: BTreeMap::new(),
            compile_settings: Some(ScrivenerCompileSettings {
                content_scope: Some("Included".into()),
                formats: vec![ScrivenerCompileFormatSettings {
                    source_format_id: "retailer".into(),
                    default_front_matter_id: Some("front".into()),
                    add_default_front_matter: Some(true),
                    default_back_matter_id: Some("back".into()),
                    add_default_back_matter: Some(true),
                    ..ScrivenerCompileFormatSettings::default()
                }],
                ..ScrivenerCompileSettings::default()
            }),
            section_types: BTreeMap::new(),
            level_types: BTreeMap::new(),
            status_labels: BTreeMap::new(),
            items: vec![
                ScrivenerBinderItem {
                    source_document_id: "draft".into(),
                    parent_source_id: None,
                    position: 0,
                    title: "Draft".into(),
                    type_code: Some("DraftFolder".into()),
                    area: "manuscript".into(),
                    include_in_compile: Some(true),
                    section_type_id: None,
                    synopsis: None,
                    notes_markdown: None,
                    content_markdown: None,
                    embedded_assets: Vec::new(),
                    fields: BTreeMap::new(),
                    unsupported: Vec::new(),
                },
                {
                    let mut item = make_item("matter-folder", None);
                    item.position = 1;
                    item.type_code = Some("Folder".into());
                    item
                },
                {
                    let mut item = make_item("front", Some("matter-folder"));
                    item.position = 0;
                    item
                },
                {
                    let mut item = make_item("back", Some("matter-folder"));
                    item.position = 1;
                    item
                },
                {
                    let mut item = make_item("private-sibling", Some("matter-folder"));
                    item.position = 2;
                    item
                },
                {
                    let mut item = make_item("alternate", None);
                    item.position = 2;
                    item
                },
            ],
            warnings: Vec::new(),
            unsupported: Vec::new(),
        };
        let plan = ScrivenerImportPlan::proposed(&inventory);
        let mapping = plan
            .decisions
            .iter()
            .map(|decision| {
                (
                    decision.source_document_id.as_str(),
                    decision.destination_kind,
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(mapping["front"], ScrivenerDestinationKind::FrontMatter);
        assert_eq!(mapping["back"], ScrivenerDestinationKind::BackMatter);
        assert_eq!(
            mapping["matter-folder"],
            ScrivenerDestinationKind::Research,
            "a private Project Notes container must not become a publishable section"
        );
        assert_eq!(
            mapping["private-sibling"],
            ScrivenerDestinationKind::Research
        );
        assert_eq!(mapping["alternate"], ScrivenerDestinationKind::Research);
        let mut plan = plan;
        plan.decisions
            .iter_mut()
            .find(|decision| decision.source_document_id == "draft")
            .unwrap()
            .destination_kind = ScrivenerDestinationKind::Section;
        plan.validate_for_import(&inventory).unwrap();

        let mut conflicting = inventory.clone();
        conflicting.compile_settings.as_mut().unwrap().formats.push(
            ScrivenerCompileFormatSettings {
                source_format_id: "alternate-retailer".into(),
                default_front_matter_id: Some("back".into()),
                add_default_front_matter: Some(true),
                ..ScrivenerCompileFormatSettings::default()
            },
        );
        let conflicting_plan = ScrivenerImportPlan::proposed(&conflicting);
        assert_eq!(
            conflicting_plan
                .decisions
                .iter()
                .find(|decision| decision.source_document_id == "back")
                .unwrap()
                .destination_kind,
            ScrivenerDestinationKind::Unmapped,
            "conflicting format roles must require an explicit import decision"
        );
    }

    #[test]
    fn trash_subtrees_stay_excluded_and_empty_organisational_folders_are_not_errors() {
        let make_item =
            |id: &str, parent: Option<&str>, title: &str, type_code: &str, area: &str| {
                ScrivenerBinderItem {
                    source_document_id: id.into(),
                    parent_source_id: parent.map(str::to_owned),
                    position: 0,
                    title: title.into(),
                    type_code: Some(type_code.into()),
                    area: area.into(),
                    include_in_compile: Some(true),
                    section_type_id: None,
                    synopsis: None,
                    notes_markdown: None,
                    content_markdown: None,
                    embedded_assets: Vec::new(),
                    fields: BTreeMap::new(),
                    unsupported: Vec::new(),
                }
            };
        let mut inventory = ScrivenerProjectInventory {
            source_path: "fixture.scriv".into(),
            source_fingerprint: "fingerprint".into(),
            title: "Book".into(),
            format_version: Some("2.0".into()),
            creator: None,
            project_metadata: BTreeMap::new(),
            compile_settings: Some(ScrivenerCompileSettings {
                formats: vec![ScrivenerCompileFormatSettings {
                    source_format_id: "retailer".into(),
                    default_front_matter_id: Some("trashed-page".into()),
                    add_default_front_matter: Some(true),
                    ..ScrivenerCompileFormatSettings::default()
                }],
                ..ScrivenerCompileSettings::default()
            }),
            section_types: BTreeMap::new(),
            level_types: BTreeMap::new(),
            status_labels: BTreeMap::new(),
            items: vec![
                make_item("trash-root", None, "Trash", "TrashFolder", "trash"),
                make_item(
                    "trashed-page",
                    Some("trash-root"),
                    "Former Manuscript Page",
                    "DraftFolder",
                    "trash",
                ),
            ],
            warnings: Vec::new(),
            unsupported: Vec::new(),
        };
        assert_eq!(
            classify_scrivener_area("Former Manuscript Page", Some("DraftFolder"), "trash"),
            "trash"
        );
        let plan = ScrivenerImportPlan::proposed(&inventory);
        assert!(plan.decisions.iter().all(|decision| {
            decision.destination_kind == ScrivenerDestinationKind::Skip
                && !decision.include_in_import
        }));
        plan.validate_for_import(&inventory).unwrap();

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("Empty Group.scriv");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("Empty Group.scrivx"),
            r#"<ScrivenerProject Version="2.0"><Binder>
              <BinderItem UUID="draft" Type="DraftFolder"><Title>Draft</Title><Children>
                <BinderItem UUID="part" Type="Folder"><Title>Part One</Title><Children>
                  <BinderItem UUID="chapter" Type="Folder"><Title>Chapter One</Title><Children/></BinderItem>
                </Children></BinderItem>
              </Children></BinderItem>
            </Binder></ScrivenerProject>"#,
        )
        .unwrap();
        inventory = inspect_scrivener_project(&source).unwrap();
        assert!(inventory.items.iter().all(|item| {
            !item
                .unsupported
                .iter()
                .any(|warning| warning.contains("no readable content.rtf"))
        }));
        assert!(
            ScrivenerImportPlan::proposed(&inventory)
                .validate_for_import(&inventory)
                .is_ok()
        );
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
            project_metadata: BTreeMap::new(),
            compile_settings: None,
            section_types: BTreeMap::new(),
            level_types: BTreeMap::new(),
            status_labels: BTreeMap::new(),
            items: vec![
                ScrivenerBinderItem {
                    source_document_id: "same".into(),
                    parent_source_id: None,
                    position: 0,
                    title: String::new(),
                    type_code: None,
                    area: "unknown".into(),
                    include_in_compile: None,
                    section_type_id: None,
                    synopsis: None,
                    notes_markdown: None,
                    content_markdown: None,
                    embedded_assets: Vec::new(),
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
                    section_type_id: None,
                    synopsis: None,
                    notes_markdown: None,
                    content_markdown: None,
                    embedded_assets: Vec::new(),
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
                    "<w:document><w:body><w:p><w:r><w:t>Base </w:t></w:r><w:ins w:id=\"2\" w:author=\"Editor\" w:date=\"2026-01-01\"><w:r><w:t>change</w:t></w:r></w:ins><w:del w:id=\"3\" w:author=\"Editor\"><w:r><w:delText>old</w:delText></w:r></w:del></w:p><w:p><w:r><w:t>Before </w:t></w:r><w:commentRangeStart w:id=\"7\"/><w:r><w:t>Second &amp; final.</w:t></w:r><w:commentRangeEnd w:id=\"7\"/><w:r><w:t> After.</w:t></w:r><w:r><w:commentReference w:id=\"7\"/></w:r></w:p></w:body></w:document>",
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
        assert_eq!(comment.anchor_text.as_deref(), Some("Second & final."));
        assert_eq!(comment.anchor_complete, Some(true));
        assert_eq!(comment.text, "Clarify this.");
        let unanchored = items
            .iter()
            .find(|item| item.comment_id.as_deref() == Some("8"))
            .unwrap();
        assert_eq!(unanchored.paragraph_index, None);
        assert_eq!(unanchored.anchor_text, None);
        assert_eq!(unanchored.anchor_complete, None);
        assert_eq!(
            docx_review_text(&docx, true).unwrap(),
            "Base change\n\nBefore Second & final. After."
        );
        assert_eq!(
            docx_review_text(&docx, false).unwrap(),
            "Base old\n\nBefore Second & final. After."
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

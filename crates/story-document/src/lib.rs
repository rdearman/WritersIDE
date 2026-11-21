//! Source-preserving semantic Markdown and renderer-neutral compilation IR.
use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const SEMANTIC_SCHEMA_VERSION: u16 = 1;
pub const COMPILE_IR_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SemanticKind {
    Paragraph,
    Heading { level: u8 },
    BlockQuote,
    CodeBlock { info: String },
    List { ordered: Option<u64> },
    Item,
    FootnoteDefinition { label: String },
    Table,
    TableHead,
    TableRow,
    TableCell,
    Emphasis,
    Strong,
    Strikethrough,
    Link { destination: String, title: String },
    Image { destination: String, title: String },
    Text { value: String },
    InlineCode { value: String },
    Html { value: String },
    FootnoteReference { label: String },
    SoftBreak,
    HardBreak,
    Rule,
    TaskListMarker { checked: bool },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticNode {
    pub kind: SemanticKind,
    pub source: SourceRange,
    pub parent: Option<usize>,
}

/// A lossless Markdown source plus a rebuildable semantic projection.
/// The original bytes remain authoritative; the projection never serializes
/// back into prose and therefore cannot normalize author formatting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticDocument {
    pub schema_version: u16,
    pub source: String,
    pub source_sha256: String,
    pub nodes: Vec<SemanticNode>,
}

impl SemanticDocument {
    pub fn parse(source: impl Into<String>) -> Self {
        let source = source.into();
        let parser = Parser::new_ext(&source, markdown_options()).into_offset_iter();
        let mut nodes: Vec<SemanticNode> = Vec::new();
        let mut parents: Vec<usize> = Vec::new();
        for (event, range) in parser {
            match event {
                Event::Start(tag) => {
                    let index = nodes.len();
                    nodes.push(SemanticNode {
                        kind: kind_from_tag(tag),
                        source: SourceRange {
                            start: range.start,
                            end: range.end,
                        },
                        parent: parents.last().copied(),
                    });
                    parents.push(index);
                }
                Event::End(_) => {
                    if let Some(index) = parents.pop() {
                        nodes[index].source.end = range.end;
                    }
                }
                event => nodes.push(SemanticNode {
                    kind: kind_from_leaf(event),
                    source: SourceRange {
                        start: range.start,
                        end: range.end,
                    },
                    parent: parents.last().copied(),
                }),
            }
        }
        let source_sha256 = format!("{:x}", Sha256::digest(source.as_bytes()));
        Self {
            schema_version: SEMANTIC_SCHEMA_VERSION,
            source,
            source_sha256,
            nodes,
        }
    }

    pub fn validate_ranges(&self) -> bool {
        self.schema_version == SEMANTIC_SCHEMA_VERSION
            && self.source_sha256 == source_fingerprint(&self.source)
            && self.nodes.iter().enumerate().all(|(index, node)| {
                let parent_contains = node.parent.is_none_or(|parent| {
                    parent < index
                        && self.nodes.get(parent).is_some_and(|parent_node| {
                            parent_node.source.start <= node.source.start
                                && node.source.end <= parent_node.source.end
                        })
                });
                parent_contains
                    && node.source.start <= node.source.end
                    && node.source.end <= self.source.len()
                    && self.source.is_char_boundary(node.source.start)
                    && self.source.is_char_boundary(node.source.end)
            })
    }

    /// Returns the exact canonical Markdown bytes represented by this projection.
    pub fn to_markdown(&self) -> &str {
        &self.source
    }
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
}

fn kind_from_tag(tag: Tag<'_>) -> SemanticKind {
    match tag {
        Tag::Paragraph => SemanticKind::Paragraph,
        Tag::Heading(level, _, _) => SemanticKind::Heading { level: level as u8 },
        Tag::BlockQuote => SemanticKind::BlockQuote,
        Tag::CodeBlock(kind) => SemanticKind::CodeBlock {
            info: match kind {
                pulldown_cmark::CodeBlockKind::Indented => String::new(),
                pulldown_cmark::CodeBlockKind::Fenced(info) => info.into_string(),
            },
        },
        Tag::List(ordered) => SemanticKind::List { ordered },
        Tag::Item => SemanticKind::Item,
        Tag::FootnoteDefinition(label) => SemanticKind::FootnoteDefinition {
            label: label.into_string(),
        },
        Tag::Table(_) => SemanticKind::Table,
        Tag::TableHead => SemanticKind::TableHead,
        Tag::TableRow => SemanticKind::TableRow,
        Tag::TableCell => SemanticKind::TableCell,
        Tag::Emphasis => SemanticKind::Emphasis,
        Tag::Strong => SemanticKind::Strong,
        Tag::Strikethrough => SemanticKind::Strikethrough,
        Tag::Link(_, destination, title) => SemanticKind::Link {
            destination: destination.into_string(),
            title: title.into_string(),
        },
        Tag::Image(_, destination, title) => SemanticKind::Image {
            destination: destination.into_string(),
            title: title.into_string(),
        },
    }
}

fn kind_from_leaf(event: Event<'_>) -> SemanticKind {
    match event {
        Event::Text(value) => SemanticKind::Text {
            value: value.into_string(),
        },
        Event::Code(value) => SemanticKind::InlineCode {
            value: value.into_string(),
        },
        Event::Html(value) => SemanticKind::Html {
            value: value.into_string(),
        },
        Event::FootnoteReference(label) => SemanticKind::FootnoteReference {
            label: label.into_string(),
        },
        Event::SoftBreak => SemanticKind::SoftBreak,
        Event::HardBreak => SemanticKind::HardBreak,
        Event::Rule => SemanticKind::Rule,
        Event::TaskListMarker(checked) => SemanticKind::TaskListMarker { checked },
        Event::Start(_) | Event::End(_) => unreachable!("container events are handled separately"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContentUnitKind {
    FrontMatter,
    SectionBody,
    ChapterBody,
    Scene,
    BackMatter,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileSegment {
    pub source_entity_id: String,
    pub document: SemanticDocument,
}

impl CompileSegment {
    pub fn new(source_entity_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            source_entity_id: source_entity_id.into(),
            document: SemanticDocument::parse(text),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileUnit {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: ContentUnitKind,
    pub title: String,
    pub segments: Vec<CompileSegment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StructuralKind {
    Section,
    Chapter,
    Group,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileGroup {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: StructuralKind,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileSourceMapEntry {
    pub output: SourceRange,
    pub source_entity_id: String,
    pub source_sha256: String,
    pub source: SourceRange,
}

/// Renderer-neutral ordered manuscript snapshot. A renderer may choose a
/// target-specific presentation while preserving unit identity and source maps.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileIr {
    pub version: u16,
    pub title: String,
    pub groups: Vec<CompileGroup>,
    pub units: Vec<CompileUnit>,
}

impl CompileIr {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            version: COMPILE_IR_VERSION,
            title: title.into(),
            groups: Vec::new(),
            units: Vec::new(),
        }
    }

    pub fn push_group(&mut self, group: CompileGroup) {
        self.groups.push(group);
    }

    pub fn push(&mut self, unit: CompileUnit) {
        self.units.push(unit);
    }

    pub fn validate(&self) -> bool {
        if self.version != COMPILE_IR_VERSION {
            return false;
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut parents = std::collections::BTreeMap::<&str, Option<&str>>::new();
        for group in &self.groups {
            if group.id.is_empty() || !ids.insert(group.id.as_str()) {
                return false;
            }
            parents.insert(&group.id, group.parent_id.as_deref());
        }
        for unit in &self.units {
            if unit.id.is_empty() || !ids.insert(unit.id.as_str()) {
                return false;
            }
            parents.insert(&unit.id, unit.parent_id.as_deref());
            if unit
                .segments
                .iter()
                .any(|segment| !segment.document.validate_ranges())
            {
                return false;
            }
        }
        if parents
            .values()
            .flatten()
            .any(|parent| !ids.contains(parent))
        {
            return false;
        }
        for id in parents.keys() {
            let mut path = std::collections::BTreeSet::new();
            let mut cursor = Some(*id);
            while let Some(current) = cursor {
                if !path.insert(current) {
                    return false;
                }
                cursor = parents.get(current).copied().flatten();
            }
        }
        true
    }

    pub fn render_markdown(&self) -> (String, Vec<CompileSourceMapEntry>) {
        let mut output = String::new();
        let mut source_map = Vec::new();
        let groups = self
            .groups
            .iter()
            .map(|group| (group.id.as_str(), group))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut active_group_path: Vec<&str> = Vec::new();
        for unit in &self.units {
            let mut target_group_path = Vec::new();
            let mut parent = unit.parent_id.as_deref();
            while let Some(id) = parent {
                let Some(group) = groups.get(id) else {
                    break;
                };
                target_group_path.push(id);
                parent = group.parent_id.as_deref();
            }
            target_group_path.reverse();
            let shared = active_group_path
                .iter()
                .zip(&target_group_path)
                .take_while(|(a, b)| a == b)
                .count();
            for id in target_group_path.iter().skip(shared) {
                if let Some(group) = groups.get(id) {
                    let level = match group.kind {
                        StructuralKind::Section => 1,
                        StructuralKind::Chapter => 2,
                        StructuralKind::Group => 3,
                    };
                    if !output.is_empty() && !output.ends_with("\n\n") {
                        output.push_str("\n\n");
                    }
                    output.push_str(&"#".repeat(level));
                    output.push(' ');
                    output.push_str(&group.title);
                    output.push_str("\n\n");
                }
            }
            active_group_path = target_group_path;
            if matches!(
                unit.kind,
                ContentUnitKind::Scene | ContentUnitKind::FrontMatter | ContentUnitKind::BackMatter
            ) {
                let level = if active_group_path.is_empty() { 2 } else { 3 };
                output.push_str(&"#".repeat(level));
                output.push(' ');
                output.push_str(&unit.title);
                output.push_str("\n\n");
            }
            for (index, segment) in unit.segments.iter().enumerate() {
                if index > 0 {
                    output.push_str("\n\n---\n\n");
                }
                let start = output.len();
                output.push_str(&segment.document.source);
                source_map.push(CompileSourceMapEntry {
                    output: SourceRange {
                        start,
                        end: output.len(),
                    },
                    source_entity_id: segment.source_entity_id.clone(),
                    source_sha256: segment.document.source_sha256.clone(),
                    source: SourceRange {
                        start: 0,
                        end: segment.document.source.len(),
                    },
                });
            }
            output.push_str("\n\n");
        }
        (output, source_map)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompileFormat {
    Markdown,
    Html,
    Epub,
    Docx,
    Pdf,
}

impl CompileFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Self::Markdown),
            "html" | "html5" => Some(Self::Html),
            "epub" | "epub3" => Some(Self::Epub),
            "docx" => Some(Self::Docx),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Html => "html",
            Self::Epub => "epub",
            Self::Docx => "docx",
            Self::Pdf => "pdf",
        }
    }
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Markdown => "text/markdown; charset=utf-8",
            Self::Html => "text/html; charset=utf-8",
            Self::Epub => "application/epub+zip",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Pdf => "application/pdf",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileArtifact {
    pub format: CompileFormat,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
    /// Source offsets refer to the UTF-8 Compile IR Markdown stream supplied to the renderer.
    pub source_map: Vec<CompileSourceMapEntry>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BookDesign {
    /// General purpose, one-sided book layout suitable for fiction.
    #[default]
    TradePaperback,
    /// Double-spaced manuscript layout suitable for editorial submission.
    Manuscript,
    /// One-sided, roomy layout for nonfiction and reference works.
    Nonfiction,
}

/// Portable, project-relative compile edition configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EditionRecipe {
    pub version: u16,
    pub name: String,
    pub format: CompileFormat,
    /// Destination within the project; absolute paths and parent traversal are forbidden.
    pub output: String,
    #[serde(default)]
    pub design: BookDesign,
    #[serde(default)]
    pub expand_transclusions: bool,
    #[serde(default)]
    pub spellcheck: bool,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub dictionary: Option<String>,
}

impl EditionRecipe {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!(
                "unsupported edition recipe version {}",
                self.version
            ));
        }
        if self.name.trim().is_empty() {
            return Err("edition name cannot be empty".into());
        }
        validate_project_relative_path(&self.output)?;
        if self.spellcheck && self.language.as_deref().is_none_or(str::is_empty) {
            return Err("spellcheck editions require a language tag".into());
        }
        if let Some(dictionary) = &self.dictionary {
            if !self.spellcheck {
                return Err("edition dictionary is only valid when spellcheck is enabled".into());
            }
            validate_project_relative_path(dictionary)?;
        }
        Ok(())
    }
    pub fn from_toml(source: &str) -> Result<Self, String> {
        let recipe: Self = toml::from_str(source).map_err(|error| error.to_string())?;
        recipe.validate()?;
        Ok(recipe)
    }
    pub fn to_toml(&self) -> Result<String, String> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|error| error.to_string())
    }
}

fn validate_project_relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "edition paths must be non-empty project-relative paths without traversal: {value}"
        ));
    }
    Ok(())
}

impl BookDesign {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "trade" | "trade-paperback" => Some(Self::TradePaperback),
            "manuscript" => Some(Self::Manuscript),
            "nonfiction" => Some(Self::Nonfiction),
            _ => None,
        }
    }

    fn pandoc_variables(self) -> &'static [&'static str] {
        match self {
            Self::TradePaperback => &[
                "documentclass=book",
                "classoption=oneside",
                "classoption=openany",
                "papersize=6in:9in",
                "geometry=margin=0.75in",
                "fontsize=11pt",
                "linestretch=1.15",
            ],
            Self::Manuscript => &[
                "documentclass=article",
                "papersize=letter",
                "geometry=margin=1in",
                "fontsize=12pt",
                "linestretch=2",
            ],
            Self::Nonfiction => &[
                "documentclass=book",
                "classoption=oneside",
                "classoption=openany",
                "papersize=letter",
                "geometry=margin=1in",
                "fontsize=11pt",
                "linestretch=1.25",
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompileDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source_entity_id: Option<String>,
}

/// Deterministic, non-mutating checks useful before publication or export.
pub fn publication_diagnostics(ir: &CompileIr) -> Vec<CompileDiagnostic> {
    let mut diagnostics = Vec::new();
    if ir.title.trim().is_empty() {
        diagnostics.push(CompileDiagnostic {
            code: "PUB001".into(),
            severity: DiagnosticSeverity::Warning,
            message: "project title is empty".into(),
            source_entity_id: None,
        });
    }
    if ir.units.is_empty() {
        diagnostics.push(CompileDiagnostic {
            code: "PUB002".into(),
            severity: DiagnosticSeverity::Warning,
            message: "manuscript contains no content units or scenes".into(),
            source_entity_id: None,
        });
    }
    for unit in &ir.units {
        if unit
            .segments
            .iter()
            .all(|segment| segment.document.source.trim().is_empty())
        {
            diagnostics.push(CompileDiagnostic {
                code: "PUB003".into(),
                severity: DiagnosticSeverity::Information,
                message: format!("content unit '{}' has no prose", unit.title),
                source_entity_id: Some(unit.id.clone()),
            });
        }
    }
    diagnostics
}

#[derive(Clone, Debug)]
pub struct PandocRenderer {
    program: PathBuf,
}

impl Default for PandocRenderer {
    fn default() -> Self {
        Self {
            program: PathBuf::from("pandoc"),
        }
    }
}

impl PandocRenderer {
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    pub fn render(
        &self,
        ir: &CompileIr,
        format: CompileFormat,
        resource_root: Option<&Path>,
    ) -> Result<CompileArtifact, String> {
        self.render_with_design(ir, format, resource_root, BookDesign::default())
    }

    pub fn render_with_design(
        &self,
        ir: &CompileIr,
        format: CompileFormat,
        resource_root: Option<&Path>,
        design: BookDesign,
    ) -> Result<CompileArtifact, String> {
        if !ir.validate() {
            return Err("refusing to render invalid Compile IR".into());
        }
        let (markdown, source_map) = ir.render_markdown();
        let bytes = if format == CompileFormat::Markdown {
            markdown.into_bytes()
        } else {
            let temporary =
                tempfile::tempdir().map_err(|e| format!("create renderer workspace: {e}"))?;
            let texmf_var = temporary.path().join("texmf-var");
            let texmf_config = temporary.path().join("texmf-config");
            std::fs::create_dir_all(&texmf_var)
                .map_err(|e| format!("create writable TeX cache: {e}"))?;
            std::fs::create_dir_all(&texmf_config)
                .map_err(|e| format!("create writable TeX config: {e}"))?;
            let output_path = temporary
                .path()
                .join(format!("output.{}", format.extension()));
            let mut command = Command::new(&self.program);
            command
                .arg(
                    "--from=markdown+footnotes+pipe_tables+task_lists+strikeout+fenced_code_blocks",
                )
                .arg(match format {
                    CompileFormat::Html => "--to=html5",
                    CompileFormat::Epub => "--to=epub3",
                    CompileFormat::Docx => "--to=docx",
                    CompileFormat::Pdf => "--to=pdf",
                    CompileFormat::Markdown => unreachable!(),
                })
                .arg("--standalone")
                .arg("--wrap=none")
                .arg("--metadata")
                .arg(format!("title={}", ir.title))
                .arg("--output")
                .arg(&output_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            if format == CompileFormat::Pdf {
                command
                    .arg("--pdf-engine=lualatex")
                    .arg("--pdf-engine-opt=-interaction=nonstopmode")
                    .arg("--pdf-engine-opt=-halt-on-error")
                    .env("TEXMFVAR", &texmf_var)
                    .env("TEXMFCACHE", &texmf_var)
                    .env("TEXMFCONFIG", &texmf_config);
                for variable in design.pandoc_variables() {
                    command.arg(format!("--variable={variable}"));
                }
            }
            if let Some(root) = resource_root {
                command
                    .current_dir(root)
                    .arg(format!("--resource-path={}", root.display()));
            }
            let mut child = command
                .spawn()
                .map_err(|e| format!("cannot start {}: {e}", self.program.display()))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(markdown.as_bytes())
                    .map_err(|e| format!("write renderer input: {e}"))?;
            }
            let result = child
                .wait_with_output()
                .map_err(|e| format!("wait for renderer: {e}"))?;
            if !result.status.success() {
                return Err(format!(
                    "{} failed ({}): {}",
                    self.program.display(),
                    result.status,
                    String::from_utf8_lossy(&result.stderr).trim()
                ));
            }
            std::fs::read(&output_path).map_err(|e| format!("read rendered output: {e}"))?
        };
        Ok(CompileArtifact {
            format,
            media_type: format.media_type(),
            bytes,
            source_map,
        })
    }
}

pub fn source_fingerprint(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edition_recipes_validate_round_trip_and_reject_path_escape() {
        let recipe = EditionRecipe {
            version: 1,
            name: "Reader EPUB".into(),
            format: CompileFormat::Epub,
            output: "exports/reader.epub".into(),
            design: BookDesign::Nonfiction,
            expand_transclusions: true,
            spellcheck: false,
            language: None,
            dictionary: None,
        };
        assert_eq!(
            EditionRecipe::from_toml(&recipe.to_toml().unwrap()).unwrap(),
            recipe
        );
        let mut invalid = recipe;
        invalid.output = "../outside.epub".into();
        assert!(invalid.validate().is_err());
        invalid.output = "exports/out.epub".into();
    }
    use std::io::Read;

    #[test]
    fn parses_markdown_semantics_with_utf8_source_ranges_without_rewriting_source() {
        let source = "# Chapter\n\nÉlodie *saw* **three** lights.\n\n> A quoted line.\n\n- first\n- second\n\nA note[^1].\n\n[^1]: source note\n\n```rust\nlet x = 1;\n```\n";
        let document = SemanticDocument::parse(source);
        assert_eq!(document.source, source);
        assert_eq!(document.to_markdown(), source);
        assert!(document.validate_ranges());
        assert_eq!(document.source_sha256, source_fingerprint(source));
        assert!(
            document
                .nodes
                .iter()
                .any(|node| matches!(node.kind, SemanticKind::Heading { level: 1 }))
        );
        assert!(
            document
                .nodes
                .iter()
                .any(|node| node.kind == SemanticKind::Emphasis)
        );
        assert!(
            document
                .nodes
                .iter()
                .any(|node| node.kind == SemanticKind::Strong)
        );
        assert!(
            document
                .nodes
                .iter()
                .any(|node| node.kind == SemanticKind::BlockQuote)
        );
        assert!(
            document
                .nodes
                .iter()
                .any(|node| matches!(node.kind, SemanticKind::List { .. }))
        );
        assert!(
            document
                .nodes
                .iter()
                .any(|node| matches!(node.kind, SemanticKind::FootnoteDefinition { .. }))
        );
        assert!(
            document
                .nodes
                .iter()
                .any(|node| matches!(node.kind, SemanticKind::CodeBlock { .. }))
        );
        for node in &document.nodes {
            assert!(source.is_char_boundary(node.source.start));
            assert!(source.is_char_boundary(node.source.end));
        }
    }

    #[test]
    fn compile_ir_is_ordered_renderer_neutral_and_maps_unicode_source_segments() {
        let text = "Mara — then left.";
        let mut ir = CompileIr::new("Novel");
        ir.push_group(CompileGroup {
            id: "section-1".into(),
            parent_id: None,
            kind: StructuralKind::Section,
            title: "Part One".into(),
        });
        ir.push_group(CompileGroup {
            id: "chapter-1".into(),
            parent_id: Some("section-1".into()),
            kind: StructuralKind::Chapter,
            title: "Chapter One".into(),
        });
        ir.push(CompileUnit {
            id: "scene-1".into(),
            parent_id: Some("chapter-1".into()),
            kind: ContentUnitKind::Scene,
            title: "Chapter One".into(),
            segments: vec![CompileSegment::new("scene-1", text)],
        });
        let (markdown, map) = ir.render_markdown();
        assert_eq!(
            markdown,
            "# Part One\n\n## Chapter One\n\n### Chapter One\n\nMara — then left.\n\n"
        );
        assert_eq!(&markdown[map[0].output.start..map[0].output.end], text);
        assert_eq!(map[0].source.end, text.len());
        let json = serde_json::to_vec(&ir).unwrap();
        assert_eq!(serde_json::from_slice::<CompileIr>(&json).unwrap(), ir);
        assert!(ir.validate());
    }

    #[test]
    fn compile_ir_rejects_duplicate_ids_missing_parents_and_cycles() {
        let mut ir = CompileIr::new("Book");
        ir.push_group(CompileGroup {
            id: "a".into(),
            parent_id: Some("b".into()),
            kind: StructuralKind::Section,
            title: "A".into(),
        });
        ir.push_group(CompileGroup {
            id: "b".into(),
            parent_id: Some("a".into()),
            kind: StructuralKind::Section,
            title: "B".into(),
        });
        assert!(!ir.validate());
        ir.groups[0].parent_id = None;
        assert!(ir.validate());
        ir.push(CompileUnit {
            id: "b".into(),
            parent_id: None,
            kind: ContentUnitKind::Scene,
            title: "Duplicate".into(),
            segments: Vec::new(),
        });
        assert!(!ir.validate());
    }

    #[test]
    fn pandoc_renderer_produces_html_docx_epub_pdf_and_source_maps() {
        if Command::new("pandoc").arg("--version").output().is_err() {
            return;
        }
        let mut ir = CompileIr::new("Writer’s Novel");
        ir.push_group(CompileGroup {
            id: "chapter".into(),
            parent_id: None,
            kind: StructuralKind::Chapter,
            title: "Chapter One".into(),
        });
        ir.push(CompileUnit {
            id: "scene".into(),
            parent_id: Some("chapter".into()),
            kind: ContentUnitKind::Scene,
            title: "A Door Opens".into(),
            segments: vec![CompileSegment::new(
                "scene-uuid",
                "Élodie *opened* the **door**.\n\n- One\n- Two\n",
            )],
        });
        let renderer = PandocRenderer::default();
        for format in [
            CompileFormat::Html,
            CompileFormat::Docx,
            CompileFormat::Epub,
        ] {
            let artifact = renderer.render(&ir, format, None).unwrap();
            assert!(!artifact.bytes.is_empty());
            assert_eq!(artifact.media_type, format.media_type());
            assert_eq!(artifact.source_map.len(), 1);
        }
        let docx = renderer.render(&ir, CompileFormat::Docx, None).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(docx.bytes)).unwrap();
        let mut document = String::new();
        archive
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut document)
            .unwrap();
        assert!(document.contains("Élodie"));
        let epub = renderer.render(&ir, CompileFormat::Epub, None).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(epub.bytes)).unwrap();
        assert!(archive.by_name("META-INF/container.xml").is_ok());
        if Command::new("lualatex").arg("--version").output().is_ok() {
            let pdf = renderer.render(&ir, CompileFormat::Pdf, None).unwrap();
            assert!(pdf.bytes.starts_with(b"%PDF-"));
            assert_eq!(pdf.source_map[0].source_entity_id, "scene-uuid");
        }
    }

    #[test]
    fn pandoc_backend_errors_are_explicit_and_markdown_needs_no_external_tool() {
        let mut ir = CompileIr::new("Book");
        ir.push(CompileUnit {
            id: "unit".into(),
            parent_id: None,
            kind: ContentUnitKind::Scene,
            title: "Scene".into(),
            segments: vec![CompileSegment::new("scene", "Prose.")],
        });
        let missing = PandocRenderer::with_program("writers-ide-test-missing-pandoc");
        assert!(
            missing
                .render(&ir, CompileFormat::Html, None)
                .unwrap_err()
                .contains("cannot start")
        );
        assert_eq!(
            missing
                .render(&ir, CompileFormat::Markdown, None)
                .unwrap()
                .bytes,
            b"## Scene\n\nProse.\n\n"
        );
    }

    #[test]
    fn book_designs_are_typed_and_publication_diagnostics_are_read_only() {
        assert_eq!(BookDesign::parse("trade"), Some(BookDesign::TradePaperback));
        assert_eq!(
            BookDesign::parse("manuscript"),
            Some(BookDesign::Manuscript)
        );
        assert_eq!(BookDesign::parse("invalid"), None);
        assert!(
            BookDesign::Manuscript
                .pandoc_variables()
                .contains(&"linestretch=2")
        );
        let mut ir = CompileIr::new(" ");
        ir.push(CompileUnit {
            id: "empty-unit".into(),
            parent_id: None,
            kind: ContentUnitKind::ChapterBody,
            title: "Empty chapter".into(),
            segments: vec![CompileSegment::new("empty-unit", "  ")],
        });
        let diagnostics = publication_diagnostics(&ir);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, "PUB001");
        assert_eq!(
            diagnostics[1].source_entity_id.as_deref(),
            Some("empty-unit")
        );
        assert!(ir.units[0].segments[0].document.source.trim().is_empty());
    }
}

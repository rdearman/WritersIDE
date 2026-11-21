//! Source-preserving semantic Markdown and renderer-neutral compilation IR.
use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
        for unit in &self.units {
            output.push_str("## ");
            output.push_str(&unit.title);
            output.push_str("\n\n");
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

pub fn source_fingerprint(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(markdown, "## Chapter One\n\nMara — then left.\n\n");
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
}

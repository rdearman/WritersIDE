use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadingError {
    #[error("reading I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported reference format: {0}")]
    Unsupported(String),
    #[error("invalid or unsupported EPUB: {0}")]
    Epub(String),
    #[error("reference is not valid UTF-8")]
    Utf8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceDocument {
    pub title: String,
    pub media_type: String,
    pub source_bytes: Vec<u8>,
    pub extracted_text: String,
    pub sha256: String,
    /// Language declared by the source when available. Users may override it per document.
    pub language_code: Option<String>,
}

pub fn import_reference(path: impl AsRef<Path>) -> Result<ReferenceDocument, ReadingError> {
    let path = path.as_ref();
    let source_bytes = fs::read(path)?;
    let sha256 = format!("{:x}", Sha256::digest(&source_bytes));
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "txt" => Ok(ReferenceDocument {
            title: title(path),
            media_type: "text/plain".into(),
            extracted_text: String::from_utf8(source_bytes.clone())
                .map_err(|_| ReadingError::Utf8)?,
            source_bytes,
            sha256,
            language_code: None,
        }),
        "md" | "markdown" => Ok(ReferenceDocument {
            title: title(path),
            media_type: "text/markdown".into(),
            extracted_text: String::from_utf8(source_bytes.clone())
                .map_err(|_| ReadingError::Utf8)?,
            source_bytes,
            sha256,
            language_code: None,
        }),
        "epub" => parse_epub(path, source_bytes, sha256),
        _ => Err(ReadingError::Unsupported(extension)),
    }
}

fn parse_epub(
    path: &Path,
    source_bytes: Vec<u8>,
    sha256: String,
) -> Result<ReferenceDocument, ReadingError> {
    let mut epub = epub::doc::EpubDoc::new(path).map_err(|e| ReadingError::Epub(e.to_string()))?;
    let title = epub.mdata("title").unwrap_or_else(|| title(path));
    let language_code = epub
        .mdata("language")
        .filter(|tag| story_core::is_valid_language_tag(tag));
    let mut chapters = Vec::new();
    for page in 0..epub.spine.len() {
        if page > 0 {
            epub.go_next()
                .map_err(|e| ReadingError::Epub(e.to_string()))?;
        }
        let html = epub
            .get_current_str()
            .map_err(|e| ReadingError::Epub(e.to_string()))?;
        chapters.push(html_text(&html));
    }
    Ok(ReferenceDocument {
        title,
        media_type: "application/epub+zip".into(),
        source_bytes,
        extracted_text: chapters.join("\n\n"),
        sha256,
        language_code,
    })
}

fn title(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Reference")
        .to_owned()
}

fn html_text(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut inside = false;
    let mut in_entity = false;
    let mut entity = String::new();
    for ch in html.chars() {
        if ch == '<' {
            inside = true;
            continue;
        }
        if ch == '>' && inside {
            inside = false;
            continue;
        }
        if inside {
            continue;
        }
        if in_entity {
            if ch == ';' {
                match entity.as_str() {
                    "amp" => output.push('&'),
                    "lt" => output.push('<'),
                    "gt" => output.push('>'),
                    "quot" => output.push('"'),
                    "apos" | "#39" => output.push('\''),
                    "nbsp" => output.push(' '),
                    _ => {
                        if let Some(code) = entity
                            .strip_prefix("#x")
                            .and_then(|n| u32::from_str_radix(n, 16).ok())
                            .or_else(|| entity.strip_prefix('#').and_then(|n| n.parse().ok()))
                            .and_then(char::from_u32)
                        {
                            output.push(code)
                        } else {
                            output.push('&');
                            output.push_str(&entity);
                            output.push(';')
                        }
                    }
                }
                entity.clear();
                in_entity = false;
            } else if entity.len() < 12 && (ch.is_ascii_alphanumeric() || ch == '#') {
                entity.push(ch)
            } else {
                output.push('&');
                output.push_str(&entity);
                entity.clear();
                in_entity = false;
                output.push(ch)
            }
            continue;
        }
        if ch == '&' {
            entity.clear();
            in_entity = true;
        } else {
            output.push(ch)
        }
    }
    if in_entity {
        output.push('&');
        output.push_str(&entity);
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn minimal_epub(path: &Path) {
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let stored =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();
        let compressed =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("META-INF/container.xml", compressed)
            .unwrap();
        zip.write_all(br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
        zip.start_file("OEBPS/content.opf", compressed).unwrap();
        zip.write_all(br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="uid">fixture</dc:identifier><dc:title>Fixture Reader</dc:title><dc:language>en</dc:language></metadata><manifest><item id="chapter" href="chapter.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="chapter"/></spine></package>"#).unwrap();
        zip.start_file("OEBPS/chapter.xhtml", compressed).unwrap();
        zip.write_all(b"<html><body><p>Elinor &amp; Tomas entered.</p></body></html>")
            .unwrap();
        zip.finish().unwrap();
    }
    #[test]
    fn html_reader_strips_markup_and_decodes_common_entities() {
        assert_eq!(
            html_text("<p>Mara &amp; Jo&nbsp;wait.</p>"),
            "Mara & Jo wait."
        );
    }
    #[test]
    fn text_and_markdown_keep_source_bytes_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.md");
        fs::write(&path, "# A heading\n\nText.").unwrap();
        let before = fs::read(&path).unwrap();
        let doc = import_reference(&path).unwrap();
        assert_eq!(doc.extracted_text, "# A heading\n\nText.");
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn synthetic_epub_import_extracts_spine_without_modifying_reference() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.epub");
        minimal_epub(&path);
        let before = fs::read(&path).unwrap();
        let doc = import_reference(&path).unwrap();
        assert_eq!(doc.title, "Fixture Reader");
        assert_eq!(doc.language_code.as_deref(), Some("en"));
        assert!(doc.extracted_text.contains("Elinor & Tomas entered."));
        assert_eq!(fs::read(path).unwrap(), before);
    }
}

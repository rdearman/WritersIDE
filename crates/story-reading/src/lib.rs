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
        }),
        "md" | "markdown" => Ok(ReferenceDocument {
            title: title(path),
            media_type: "text/markdown".into(),
            extracted_text: String::from_utf8(source_bytes.clone())
                .map_err(|_| ReadingError::Utf8)?,
            source_bytes,
            sha256,
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
}

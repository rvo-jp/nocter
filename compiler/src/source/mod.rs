//! Source files, source maps, and byte-based spans.

use crate::diagnostics::Diagnostic;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId(u32);

impl SourceId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteSpan {
    pub source: SourceId,
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub const fn new(source: SourceId, start: usize, end: usize) -> Self {
        Self { source, start, end }
    }

    pub const fn len(self) -> usize {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineColumn {
    pub line: usize,
    pub column_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonSpan {
    pub file: String,
    pub absolute_path: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_column_byte: usize,
    pub end_line: usize,
    pub end_column_byte: usize,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    id: SourceId,
    display_path: String,
    absolute_path: Option<PathBuf>,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn id(&self) -> SourceId {
        self.id
    }

    pub fn display_path(&self) -> &str {
        &self.display_path
    }

    pub fn absolute_path(&self) -> Option<&PathBuf> {
        self.absolute_path.as_ref()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_column_byte(&self, offset: usize) -> Result<LineColumn, String> {
        if offset > self.text.len() {
            return Err(format!(
                "byte offset {offset} is outside source `{}`",
                self.display_path
            ));
        }

        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(0) => 0,
            Err(index) => index - 1,
        };
        let line_start = self.line_starts[line_index];

        Ok(LineColumn {
            line: line_index + 1,
            column_byte: offset - line_start + 1,
        })
    }
}

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_source(
        &mut self,
        display_path: impl Into<String>,
        absolute_path: Option<PathBuf>,
        text: impl Into<String>,
    ) -> SourceId {
        let id = SourceId::new(self.files.len() as u32);
        let text = text.into();
        let file = SourceFile {
            id,
            display_path: display_path.into(),
            absolute_path,
            line_starts: compute_line_starts(&text),
            text,
        };
        self.files.push(file);
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(id.raw() as usize)
    }

    pub(crate) fn sources_with_absolute_paths(&self) -> impl Iterator<Item = (&Path, SourceId)> {
        self.files
            .iter()
            .filter_map(|file| file.absolute_path.as_deref().map(|path| (path, file.id)))
    }

    pub fn load_file(&mut self, display_path: impl AsRef<Path>) -> Result<SourceId, Diagnostic> {
        let display_path = display_path.as_ref();
        let display = display_path.to_string_lossy().into_owned();
        let absolute_path = match display_path.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                return Err(Diagnostic::error(
                    "E0100",
                    format!("failed to resolve source file `{display}`: {error}"),
                ));
            }
        };
        let bytes = match fs::read(&absolute_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Err(Diagnostic::error(
                    "E0100",
                    format!("failed to read source file `{display}`: {error}"),
                ));
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                return Err(Diagnostic::error(
                    "E0100",
                    format!("source file `{display}` is not valid UTF-8: {error}"),
                ));
            }
        };
        let normalized = normalize_line_endings(&display, &text)?;

        Ok(self.add_source(display, Some(absolute_path), normalized))
    }

    pub fn span_to_json(&self, span: ByteSpan) -> Result<JsonSpan, String> {
        let file = self
            .get(span.source)
            .ok_or_else(|| format!("unknown source id {}", span.source.raw()))?;

        if span.start > span.end {
            return Err(format!(
                "invalid span {}..{} in `{}`",
                span.start,
                span.end,
                file.display_path()
            ));
        }

        if span.end > file.text().len() {
            return Err(format!(
                "span end {} is outside source `{}`",
                span.end,
                file.display_path()
            ));
        }

        let start = file.line_column_byte(span.start)?;
        let end = file.line_column_byte(span.end)?;

        Ok(JsonSpan {
            file: file.display_path().to_string(),
            absolute_path: file
                .absolute_path()
                .map(|path| path.to_string_lossy().into_owned()),
            start_byte: span.start,
            end_byte: span.end,
            start_line: start.line,
            start_column_byte: start.column_byte,
            end_line: end.line,
            end_column_byte: end.column_byte,
        })
    }
}

fn compute_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];

    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }

    starts
}

fn normalize_line_endings(display_path: &str, text: &str) -> Result<String, Diagnostic> {
    let bytes = text.as_bytes();
    let mut normalized = String::with_capacity(text.len());
    let mut index = 0usize;
    let mut line = 1usize;
    let mut column_byte = 1usize;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                normalized.push('\n');
                index += 2;
                line += 1;
                column_byte = 1;
            }
            b'\r' => {
                let mut diagnostic = Diagnostic::error(
                    "E0100",
                    format!(
                        "source file `{display_path}` contains a bare carriage return at line {line}, byte column {column_byte}"
                    ),
                );
                diagnostic.help = Some(
                    "use LF or CRLF line endings; use the `\\r` escape inside literals when a carriage-return byte is intended"
                        .to_string(),
                );
                return Err(diagnostic);
            }
            b'\n' => {
                normalized.push('\n');
                index += 1;
                line += 1;
                column_byte = 1;
            }
            _ => {
                let ch = text[index..]
                    .chars()
                    .next()
                    .expect("index is inside UTF-8 text");
                normalized.push(ch);
                index += ch.len_utf8();
                column_byte += ch.len_utf8();
            }
        }
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_byte_span_to_json_span() {
        let mut sources = SourceMap::new();
        let id = sources.add_source("app.nct", None, "func main(): i32 {\n    return 0\n}\n");
        let span = ByteSpan::new(id, 23, 29);
        let json = sources.span_to_json(span).unwrap();

        assert_eq!(json.file, "app.nct");
        assert_eq!(json.start_line, 2);
        assert_eq!(json.start_column_byte, 5);
        assert_eq!(json.end_line, 2);
        assert_eq!(json.end_column_byte, 11);
    }

    #[test]
    fn normalizes_crlf_to_lf() {
        let normalized = normalize_line_endings("app.nct", "let a = 1\r\nlet b = 2\r\n").unwrap();
        assert_eq!(normalized, "let a = 1\nlet b = 2\n");
    }

    #[test]
    fn rejects_bare_carriage_return() {
        let diagnostic = normalize_line_endings("app.nct", "let a = 1\rlet b = 2").unwrap_err();
        assert!(diagnostic.message.contains("bare carriage return"));
    }
}

use std::fmt;

use crate::{ByteOffset, LineIndex, SourceId, Span, TextRange};

/// Display and diagnostic name of a source input.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SourceName(String);

impl SourceName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One immutable normalized UTF-8 source.
#[derive(Clone, Debug)]
pub struct SourceFile {
    id: SourceId,
    name: SourceName,
    text: String,
    len: ByteOffset,
    lines: LineIndex,
}

impl SourceFile {
    #[must_use]
    pub const fn id(&self) -> SourceId {
        self.id
    }

    #[must_use]
    pub const fn name(&self) -> &SourceName {
        &self.name
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn text_at(&self, range: TextRange) -> Option<&str> {
        let start = usize::try_from(range.start().get()).ok()?;
        let end = usize::try_from(range.end().get()).ok()?;
        self.text.get(start..end)
    }

    #[must_use]
    pub const fn len(&self) -> ByteOffset {
        self.len
    }

    #[must_use]
    pub const fn lines(&self) -> &LineIndex {
        &self.lines
    }

    /// Creates a source span after checking it against this file.
    ///
    /// # Panics
    ///
    /// Panics when the range extends beyond the normalized source text.
    #[must_use]
    pub fn span(&self, range: TextRange) -> Span {
        assert!(range.end().get() <= self.len().get(), "span exceeds source");
        Span::new(self.id, range)
    }
}

/// Source ingestion failure reported in normalized coordinates when an offset exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceError {
    RawCarriageReturn { normalized_offset: ByteOffset },
    InvalidUtf8 { normalized_offset: ByteOffset },
    SourceTooLarge,
    TooManySources,
}

/// Owns source identities and normalized text for one compiler invocation.
#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    #[must_use]
    pub const fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Adds UTF-8 source after normalizing CRLF to LF.
    ///
    /// A raw carriage return is rejected before a source identity is allocated.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] for raw carriage returns, invalid UTF-8, inputs larger than the
    /// coordinate representation, or an exhausted source-identity space.
    pub fn add_bytes(&mut self, name: SourceName, bytes: &[u8]) -> Result<SourceId, SourceError> {
        let normalized = normalize(bytes)?;
        let len = ByteOffset::new(
            u32::try_from(normalized.len()).map_err(|_| SourceError::SourceTooLarge)?,
        );
        let lines = LineIndex::new(&normalized, len);
        let index = u32::try_from(self.files.len()).map_err(|_| SourceError::TooManySources)?;
        let id = SourceId::from_index(index);
        self.files.push(SourceFile {
            id,
            name,
            text: normalized,
            len,
            lines,
        });
        Ok(id)
    }

    #[must_use]
    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(id.index() as usize)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn normalize(bytes: &[u8]) -> Result<String, SourceError> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut cursor = 0;

    while cursor < bytes.len() {
        if bytes[cursor] == b'\r' {
            if bytes.get(cursor + 1) == Some(&b'\n') {
                normalized.push(b'\n');
                cursor += 2;
                continue;
            }

            return Err(SourceError::RawCarriageReturn {
                normalized_offset: checked_offset(normalized.len())?,
            });
        }

        normalized.push(bytes[cursor]);
        cursor += 1;
    }

    if normalized.len() > u32::MAX as usize {
        return Err(SourceError::SourceTooLarge);
    }

    String::from_utf8(normalized).map_err(|error| SourceError::InvalidUtf8 {
        normalized_offset: ByteOffset::new(
            u32::try_from(error.utf8_error().valid_up_to())
                .expect("validated source length fits a u32"),
        ),
    })
}

fn checked_offset(value: usize) -> Result<ByteOffset, SourceError> {
    u32::try_from(value)
        .map(ByteOffset::new)
        .map_err(|_| SourceError::SourceTooLarge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_crlf_before_allocating_identity() {
        let mut sources = SourceMap::new();
        let id = sources
            .add_bytes(SourceName::new("app.nct"), b"first\r\nsecond\r\n")
            .unwrap();

        assert_eq!(id.index(), 0);
        assert_eq!(sources.get(id).unwrap().text(), "first\nsecond\n");
    }

    #[test]
    fn rejects_raw_carriage_return_in_normalized_coordinates() {
        let mut sources = SourceMap::new();
        let error = sources
            .add_bytes(SourceName::new("app.nct"), b"a\r\nb\rc")
            .unwrap_err();

        assert_eq!(
            error,
            SourceError::RawCarriageReturn {
                normalized_offset: ByteOffset::new(3),
            }
        );
        assert!(sources.is_empty());
    }

    #[test]
    fn reports_invalid_utf8_after_newline_normalization() {
        let mut sources = SourceMap::new();
        let error = sources
            .add_bytes(SourceName::new("app.nct"), b"a\r\n\xFF")
            .unwrap_err();

        assert_eq!(
            error,
            SourceError::InvalidUtf8 {
                normalized_offset: ByteOffset::new(2),
            }
        );
    }

    #[test]
    fn indexes_lines_in_normalized_coordinates() {
        let mut sources = SourceMap::new();
        let id = sources
            .add_bytes(SourceName::new("app.nct"), "a\r\nβ\n".as_bytes())
            .unwrap();
        let source = sources.get(id).unwrap();

        assert_eq!(source.lines().line_count(), 3);
        let location = source.lines().line_column(ByteOffset::new(4)).unwrap();
        assert_eq!(location.line(), 1);
        assert_eq!(location.byte_column(), 2);
        assert_eq!(
            source.lines().line_range(1),
            Some(TextRange::new(ByteOffset::new(2), ByteOffset::new(5)))
        );
        assert_eq!(
            source.text_at(TextRange::new(ByteOffset::new(2), ByteOffset::new(4))),
            Some("β")
        );
        assert_eq!(
            source.text_at(TextRange::new(ByteOffset::new(3), ByteOffset::new(4))),
            None
        );
    }
}

use std::collections::BTreeMap;

use crate::{ByteOffset, CoordinateError, TextRange, Utf16Position};

#[derive(Clone, Copy, Debug)]
struct EncodedScalar {
    byte_start: u32,
    utf16_start: u32,
    byte_len: u32,
    utf16_len: u32,
}

/// Zero-based line and UTF-8 byte column in normalized source.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LineColumn {
    line: u32,
    byte_column: u32,
}

impl LineColumn {
    #[must_use]
    pub const fn line(self) -> u32 {
        self.line
    }

    #[must_use]
    pub const fn byte_column(self) -> u32 {
        self.byte_column
    }
}

/// Immutable line-start index over one normalized source.
#[derive(Clone, Debug)]
pub struct LineIndex {
    starts: Vec<ByteOffset>,
    source_len: ByteOffset,
    encoded_lines: BTreeMap<u32, Vec<EncodedScalar>>,
}

impl LineIndex {
    pub(crate) fn new(text: &str, source_len: ByteOffset) -> Self {
        let mut starts = vec![ByteOffset::new(0)];
        let mut encoded_lines = BTreeMap::<u32, Vec<EncodedScalar>>::new();
        let mut line = 0_u32;
        let mut byte_column = 0_u32;
        let mut utf16_column = 0_u32;

        for (index, scalar) in text.char_indices() {
            if scalar == '\n' {
                starts.push(ByteOffset::new(
                    u32::try_from(index + 1).expect("source length was validated"),
                ));
                line += 1;
                byte_column = 0;
                utf16_column = 0;
                continue;
            }

            let byte_len = u32::try_from(scalar.len_utf8()).expect("one scalar fits u32");
            let utf16_len = u32::try_from(scalar.len_utf16()).expect("one scalar fits u32");
            if byte_len != utf16_len {
                encoded_lines.entry(line).or_default().push(EncodedScalar {
                    byte_start: byte_column,
                    utf16_start: utf16_column,
                    byte_len,
                    utf16_len,
                });
            }
            byte_column += byte_len;
            utf16_column += utf16_len;
        }

        Self {
            starts,
            source_len,
            encoded_lines,
        }
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    #[must_use]
    pub fn line_column(&self, offset: ByteOffset) -> Option<LineColumn> {
        if offset > self.source_len {
            return None;
        }
        let index = self
            .starts
            .partition_point(|line_start| *line_start <= offset)
            .saturating_sub(1);
        let start = self.starts[index];
        Some(LineColumn {
            line: u32::try_from(index).ok()?,
            byte_column: offset.get() - start.get(),
        })
    }

    /// Returns a line range including its terminating LF when present.
    #[must_use]
    pub fn line_range(&self, line: u32) -> Option<TextRange> {
        let index = usize::try_from(line).ok()?;
        let start = *self.starts.get(index)?;
        let end = self
            .starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.source_len);
        Some(TextRange::new(start, end))
    }

    /// Returns a line range excluding its terminating LF.
    #[must_use]
    pub fn line_content_range(&self, line: u32) -> Option<TextRange> {
        let range = self.line_range(line)?;
        let has_lf = self.starts.get(usize::try_from(line).ok()? + 1).is_some();
        let end = if has_lf {
            ByteOffset::new(range.end().get() - 1)
        } else {
            range.end()
        };
        Some(TextRange::new(range.start(), end))
    }

    pub(crate) fn utf16_position(
        &self,
        text: &str,
        offset: ByteOffset,
    ) -> Result<Utf16Position, CoordinateError> {
        if offset > self.source_len {
            return Err(CoordinateError::ByteOutOfBounds {
                offset,
                source_len: self.source_len,
            });
        }
        let byte = usize::try_from(offset.get()).expect("u32 fits usize on supported hosts");
        if !text.is_char_boundary(byte) {
            return Err(CoordinateError::NotUtf8Boundary { offset });
        }

        let location = self
            .line_column(offset)
            .expect("validated byte offset has a line");
        let character = self.byte_column_to_utf16(location.line, location.byte_column)?;
        Ok(Utf16Position::new(location.line, character))
    }

    pub(crate) fn byte_offset(
        &self,
        position: Utf16Position,
    ) -> Result<ByteOffset, CoordinateError> {
        let Some(content) = self.line_content_range(position.line()) else {
            return Err(CoordinateError::LineOutOfBounds {
                line: position.line(),
                line_count: u32::try_from(self.line_count())
                    .expect("source line count fits its byte-coordinate boundary"),
            });
        };
        let byte_len = content.len();
        let utf16_len = self.byte_column_to_utf16(position.line(), byte_len)?;
        if position.character() > utf16_len {
            return Err(CoordinateError::CharacterOutOfBounds {
                position,
                line_utf16_len: utf16_len,
            });
        }

        let byte_column = self.utf16_column_to_byte(position, byte_len)?;
        Ok(ByteOffset::new(content.start().get() + byte_column))
    }

    fn byte_column_to_utf16(&self, line: u32, byte_column: u32) -> Result<u32, CoordinateError> {
        let mut difference = 0_u32;
        if let Some(scalars) = self.encoded_lines.get(&line) {
            for scalar in scalars {
                if byte_column < scalar.byte_start {
                    break;
                }
                if byte_column == scalar.byte_start {
                    return Ok(scalar.utf16_start);
                }
                if byte_column < scalar.byte_start + scalar.byte_len {
                    return Err(CoordinateError::NotUtf8Boundary {
                        offset: ByteOffset::new(self.starts[line as usize].get() + byte_column),
                    });
                }
                difference += scalar.byte_len - scalar.utf16_len;
            }
        }
        Ok(byte_column - difference)
    }

    fn utf16_column_to_byte(
        &self,
        position: Utf16Position,
        byte_len: u32,
    ) -> Result<u32, CoordinateError> {
        let mut difference = 0_u32;
        if let Some(scalars) = self.encoded_lines.get(&position.line()) {
            for scalar in scalars {
                if position.character() < scalar.utf16_start {
                    break;
                }
                if position.character() == scalar.utf16_start {
                    return Ok(scalar.byte_start);
                }
                if position.character() < scalar.utf16_start + scalar.utf16_len {
                    return Err(CoordinateError::SplitUtf16Scalar { position });
                }
                difference += scalar.byte_len - scalar.utf16_len;
            }
        }
        let byte_column = position.character() + difference;
        debug_assert!(byte_column <= byte_len);
        Ok(byte_column)
    }
}

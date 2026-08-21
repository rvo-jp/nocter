use crate::{ByteOffset, TextRange};

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
}

impl LineIndex {
    pub(crate) fn new(text: &str, source_len: ByteOffset) -> Self {
        let mut starts = vec![ByteOffset::new(0)];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(index, _)| {
                    ByteOffset::new(u32::try_from(index + 1).expect("source length was validated"))
                }),
        );
        Self { starts, source_len }
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
}

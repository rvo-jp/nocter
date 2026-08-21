use std::fmt;

use crate::ByteOffset;

/// Zero-based line and UTF-16 code-unit column used at editor protocol boundaries.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Utf16Position {
    line: u32,
    character: u32,
}

impl Utf16Position {
    #[must_use]
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }

    #[must_use]
    pub const fn line(self) -> u32 {
        self.line
    }

    #[must_use]
    pub const fn character(self) -> u32 {
        self.character
    }
}

/// Half-open range in zero-based UTF-16 editor coordinates.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Utf16Range {
    start: Utf16Position,
    end: Utf16Position,
}

impl Utf16Range {
    /// Creates an unchecked protocol range. Conversion validates its ordering and boundaries.
    #[must_use]
    pub const fn new(start: Utf16Position, end: Utf16Position) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn start(self) -> Utf16Position {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Utf16Position {
        self.end
    }
}

/// Failure to translate between normalized compiler coordinates and UTF-16 editor coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateError {
    ByteOutOfBounds {
        offset: ByteOffset,
        source_len: ByteOffset,
    },
    NotUtf8Boundary {
        offset: ByteOffset,
    },
    LineOutOfBounds {
        line: u32,
        line_count: u32,
    },
    CharacterOutOfBounds {
        position: Utf16Position,
        line_utf16_len: u32,
    },
    SplitUtf16Scalar {
        position: Utf16Position,
    },
    ReversedRange {
        range: Utf16Range,
    },
}

impl fmt::Display for CoordinateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteOutOfBounds { offset, source_len } => write!(
                formatter,
                "byte offset {} exceeds source length {}",
                offset.get(),
                source_len.get()
            ),
            Self::NotUtf8Boundary { offset } => {
                write!(
                    formatter,
                    "byte offset {} splits a UTF-8 scalar",
                    offset.get()
                )
            }
            Self::LineOutOfBounds { line, line_count } => {
                write!(formatter, "line {line} exceeds line count {line_count}")
            }
            Self::CharacterOutOfBounds {
                position,
                line_utf16_len,
            } => write!(
                formatter,
                "UTF-16 character {} exceeds line {} length {}",
                position.character(),
                position.line(),
                line_utf16_len
            ),
            Self::SplitUtf16Scalar { position } => write!(
                formatter,
                "UTF-16 position {}:{} splits a surrogate pair",
                position.line(),
                position.character()
            ),
            Self::ReversedRange { range } => write!(
                formatter,
                "UTF-16 range {}:{}..{}:{} is reversed",
                range.start().line(),
                range.start().character(),
                range.end().line(),
                range.end().character()
            ),
        }
    }
}

impl std::error::Error for CoordinateError {}

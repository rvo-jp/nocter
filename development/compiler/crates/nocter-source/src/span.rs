use std::fmt;

/// Stable identity of one source added to a [`crate::SourceMap`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(u32);

impl SourceId {
    pub(crate) const fn from_index(index: u32) -> Self {
        Self(index)
    }

    /// Returns the zero-based source-map index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source:{}", self.0)
    }
}

/// Byte offset in normalized UTF-8 source text.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ByteOffset(u32);

impl ByteOffset {
    /// Creates an offset already known to fit the source-size boundary.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Half-open byte range in normalized UTF-8 source text.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextRange {
    start: ByteOffset,
    end: ByteOffset,
}

impl TextRange {
    /// Creates a half-open range.
    ///
    /// # Panics
    ///
    /// Panics when `start` is greater than `end`.
    #[must_use]
    pub const fn new(start: ByteOffset, end: ByteOffset) -> Self {
        assert!(start.get() <= end.get(), "text range start exceeds end");
        Self { start, end }
    }

    #[must_use]
    pub const fn empty(at: ByteOffset) -> Self {
        Self { start: at, end: at }
    }

    #[must_use]
    pub const fn start(self) -> ByteOffset {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> ByteOffset {
        self.end
    }

    #[must_use]
    pub const fn len(self) -> u32 {
        self.end.get() - self.start.get()
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start.get() == self.end.get()
    }
}

/// A normalized source range paired with its source identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Span {
    source: SourceId,
    range: TextRange,
}

impl Span {
    #[must_use]
    pub const fn new(source: SourceId, range: TextRange) -> Self {
        Self { source, range }
    }

    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }
}

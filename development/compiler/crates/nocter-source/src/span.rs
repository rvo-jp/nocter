use std::fmt;

/// Stable identity of one physical source value in a [`crate::SourceIdentityDomain`].
///
/// The numeric index is meaningful only inside a map that contains this exact identity. Equality
/// and ordering also include an opaque issuance identity. A revision family may retain the full
/// identity across metadata-only generations; a content transition always receives a new one.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct SourceId(u64);

impl SourceId {
    pub(crate) const fn new(index: u32, identity: u32) -> Self {
        Self(((identity as u64) << 32) | index as u64)
    }

    /// Returns the zero-based source-map index.
    #[must_use]
    pub const fn index(self) -> u32 {
        let bytes = self.0.to_le_bytes();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    /// Returns the complete process-local identity used by internal invalidation products.
    ///
    /// These bytes are not a stable serialization format and must never enter diagnostics or
    /// generated artifacts.
    #[must_use]
    pub const fn identity_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

impl Ord for SourceId {
    fn cmp(&self, another: &Self) -> std::cmp::Ordering {
        // Semantic traversal orders sources by their current immutable map positions. The opaque
        // issuance identity is only a total-order tiebreaker for foreign values; allowing it to
        // lead this comparison would make current semantic order depend on process history.
        self.index()
            .cmp(&another.index())
            .then_with(|| self.0.cmp(&another.0))
    }
}

impl PartialOrd for SourceId {
    fn partial_cmp(&self, another: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(another))
    }
}

impl fmt::Debug for SourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The issuance identity is intentionally opaque and process-local. Keeping it out of
        // diagnostics and snapshots preserves deterministic presentation while equality still
        // enforces ownership.
        formatter
            .debug_tuple("SourceId")
            .field(&self.index())
            .finish()
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source:{}", self.index())
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
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

    /// Returns whether this half-open range contains `offset`.
    #[must_use]
    pub const fn contains_offset(self, offset: ByteOffset) -> bool {
        self.start.get() <= offset.get() && offset.get() < self.end.get()
    }

    /// Returns whether an editor cursor lies within this range or on its trailing boundary.
    ///
    /// A cursor is a position between bytes rather than a byte in the range. Completion and
    /// signature queries therefore keep ownership at `end` while ordinary semantic range lookup
    /// remains half-open through [`Self::contains_offset`].
    #[must_use]
    pub const fn contains_cursor(self, cursor: ByteOffset) -> bool {
        self.start.get() <= cursor.get() && cursor.get() <= self.end.get()
    }

    /// Returns whether this range completely contains another half-open range.
    #[must_use]
    pub const fn contains_range(self, inner: Self) -> bool {
        self.start.get() <= inner.start.get() && inner.end.get() <= self.end.get()
    }

    /// Returns whether two non-empty half-open ranges overlap.
    ///
    /// Empty ranges represent positions rather than byte sets and therefore never overlap.
    #[must_use]
    pub const fn overlaps(self, another: Self) -> bool {
        !self.is_empty()
            && !another.is_empty()
            && self.start.get() < another.end.get()
            && another.start.get() < self.end.get()
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

#[cfg(test)]
mod tests {
    use super::{ByteOffset, TextRange};

    const fn offset(value: u32) -> ByteOffset {
        ByteOffset::new(value)
    }

    #[test]
    fn half_open_range_relations_exclude_end_and_adjacency() {
        let range = TextRange::new(offset(2), offset(5));
        assert!(range.contains_offset(offset(2)));
        assert!(range.contains_offset(offset(4)));
        assert!(!range.contains_offset(offset(5)));
        assert!(range.contains_cursor(offset(5)));
        assert!(!range.contains_cursor(offset(6)));

        assert!(range.contains_range(TextRange::new(offset(3), offset(5))));
        assert!(range.contains_range(TextRange::empty(offset(5))));
        assert!(!range.contains_range(TextRange::new(offset(1), offset(3))));

        assert!(range.overlaps(TextRange::new(offset(4), offset(7))));
        assert!(!range.overlaps(TextRange::new(offset(5), offset(7))));
        assert!(!range.overlaps(TextRange::empty(offset(3))));
    }
}

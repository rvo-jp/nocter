use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::{
    ByteOffset, CoordinateError, LineIndex, SourceId, Span, TextRange, Utf16Position, Utf16Range,
};

static NEXT_SOURCE_IDENTITY: AtomicU32 = AtomicU32::new(1);

fn allocate_source_identity() -> Option<u32> {
    NEXT_SOURCE_IDENTITY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .ok()
}

/// Revision-family owner of stable physical-source identities.
///
/// Maps created in the same domain reuse an identity only while both the exact source name and
/// normalized text remain unchanged. A content transition allocates a new identity, including
/// when text later returns to an earlier value. This lets a revisioned compiler reuse
/// source-backed products across metadata-only generations without making a stale product valid
/// after an intervening edit.
#[derive(Clone)]
pub struct SourceIdentityDomain {
    state: Arc<Mutex<SourceIdentityState>>,
}

impl fmt::Debug for SourceIdentityDomain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A domain is an authority token, not a diagnostic view of every source admitted to it.
        // In particular, deriving `Debug` here would make otherwise harmless debug output from a
        // compiler revision retain and disclose the full source corpus.
        formatter.write_str("SourceIdentityDomain")
    }
}

#[derive(Debug, Default)]
struct SourceIdentityState {
    current: BTreeMap<String, CurrentSourceIdentity>,
}

#[derive(Debug)]
struct CurrentSourceIdentity {
    text: Arc<str>,
    identity: u32,
}

impl SourceIdentityDomain {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SourceIdentityState::default())),
        }
    }

    fn admit(&self, name: &SourceName, text: String) -> Option<(u32, Arc<str>)> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(current) = state.current.get(name.as_str())
            && current.text.as_ref() == text
        {
            return Some((current.identity, Arc::clone(&current.text)));
        }
        let identity = allocate_source_identity()?;
        let text: Arc<str> = text.into();
        state.current.insert(
            name.as_str().to_owned(),
            CurrentSourceIdentity {
                text: Arc::clone(&text),
                identity,
            },
        );
        Some((identity, text))
    }
}

impl Default for SourceIdentityDomain {
    fn default() -> Self {
        Self::new()
    }
}

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
    text: Arc<str>,
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

    /// Reports whether this file is the normalized source produced from one byte input.
    ///
    /// This keeps newline and UTF-8 normalization inside the source owner. Callers that validate
    /// an external source observation do not need to reconstruct a temporary source map or repeat
    /// the normalization rules themselves.
    ///
    /// # Errors
    ///
    /// Returns the same source-ingestion error as [`SourceMap::add_bytes`] when `bytes` are not a
    /// valid source input.
    pub fn matches_source_bytes(&self, bytes: &[u8]) -> Result<bool, SourceError> {
        Ok(self.text.as_ref() == normalize(bytes)?)
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

    /// Converts a normalized UTF-8 byte offset to a zero-based UTF-16 position.
    ///
    /// # Errors
    ///
    /// Returns an error when the offset is outside this source or splits a UTF-8 scalar.
    pub fn utf16_position(&self, offset: ByteOffset) -> Result<Utf16Position, CoordinateError> {
        self.lines.utf16_position(&self.text, offset)
    }

    /// Converts a zero-based UTF-16 position to a normalized UTF-8 byte offset.
    ///
    /// # Errors
    ///
    /// Returns an error when the line or character is outside this source or the position splits
    /// a surrogate pair.
    pub fn byte_offset(&self, position: Utf16Position) -> Result<ByteOffset, CoordinateError> {
        self.lines.byte_offset(position)
    }

    /// Converts a normalized byte range to a UTF-16 range.
    ///
    /// # Errors
    ///
    /// Returns an error when either boundary is invalid for this source.
    pub fn utf16_range(&self, range: TextRange) -> Result<Utf16Range, CoordinateError> {
        Ok(Utf16Range::new(
            self.utf16_position(range.start())?,
            self.utf16_position(range.end())?,
        ))
    }

    /// Converts a UTF-16 range to a normalized byte range.
    ///
    /// # Errors
    ///
    /// Returns an error for reversed or invalid boundaries.
    pub fn text_range(&self, range: Utf16Range) -> Result<TextRange, CoordinateError> {
        if range.start() > range.end() {
            return Err(CoordinateError::ReversedRange { range });
        }
        Ok(TextRange::new(
            self.byte_offset(range.start())?,
            self.byte_offset(range.end())?,
        ))
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

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawCarriageReturn { normalized_offset } => write!(
                formatter,
                "raw carriage return at normalized byte {}",
                normalized_offset.get()
            ),
            Self::InvalidUtf8 { normalized_offset } => write!(
                formatter,
                "invalid UTF-8 at normalized byte {}",
                normalized_offset.get()
            ),
            Self::SourceTooLarge => formatter.write_str("source exceeds the byte-coordinate limit"),
            Self::TooManySources => formatter.write_str("source identity space is exhausted"),
        }
    }
}

impl std::error::Error for SourceError {}

/// Owns source identities and normalized text for one immutable compiler source graph.
#[derive(Clone, Debug)]
pub struct SourceMap {
    identity_domain: SourceIdentityDomain,
    files: Vec<SourceFile>,
}

impl SourceMap {
    #[must_use]
    pub fn new() -> Self {
        Self::in_identity_domain(SourceIdentityDomain::new())
    }

    /// Creates an empty map in one revision family's physical-source identity domain.
    #[must_use]
    pub fn in_identity_domain(identity_domain: SourceIdentityDomain) -> Self {
        Self {
            identity_domain,
            files: Vec::new(),
        }
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
        let (identity, normalized) = self
            .identity_domain
            .admit(&name, normalized)
            .ok_or(SourceError::TooManySources)?;
        let id = SourceId::new(index, identity);
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
        self.files
            .get(id.index() as usize)
            .filter(|source| source.id() == id)
    }

    /// Returns the first source with one exact display name.
    ///
    /// Names are lookup metadata, not identities; the source-graph authority is responsible for
    /// uniqueness where a canonical path requires it. Editor adapters use this lookup only after
    /// that authority has closed the graph.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Option<&SourceFile> {
        self.files
            .iter()
            .find(|source| source.name().as_str() == name)
    }

    /// Iterates every source in stable identity order.
    pub fn iter(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter()
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

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
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
        assert!(
            sources
                .get(id)
                .unwrap()
                .matches_source_bytes(b"first\r\nsecond\r\n")
                .unwrap()
        );
        assert!(
            !sources
                .get(id)
                .unwrap()
                .matches_source_bytes(b"other")
                .unwrap()
        );
        assert!(matches!(
            sources.get(id).unwrap().matches_source_bytes(&[0xff]),
            Err(SourceError::InvalidUtf8 { .. })
        ));
    }

    #[test]
    fn resolves_an_exact_source_name_without_reinterpreting_it() {
        let mut sources = SourceMap::new();
        let first = sources
            .add_bytes(SourceName::new("/workspace/a.nct"), b"first")
            .unwrap();
        sources
            .add_bytes(SourceName::new("/workspace/b.nct"), b"second")
            .unwrap();

        assert_eq!(
            sources.find_by_name("/workspace/a.nct").map(SourceFile::id),
            Some(first)
        );
        assert!(sources.find_by_name("workspace/a.nct").is_none());
    }

    #[test]
    fn source_identity_cannot_alias_the_same_index_in_another_map() {
        let mut first = SourceMap::new();
        let first_id = first
            .add_bytes(SourceName::new("first.nct"), b"first")
            .unwrap();
        let cloned = first.clone();
        let mut second = SourceMap::new();
        let second_id = second
            .add_bytes(SourceName::new("second.nct"), b"second")
            .unwrap();

        assert_eq!(first_id.index(), second_id.index());
        assert_ne!(first_id, second_id);
        assert!(first.get(second_id).is_none());
        assert!(second.get(first_id).is_none());
        assert_eq!(cloned.get(first_id).map(SourceFile::text), Some("first"));
        assert_eq!(std::mem::size_of::<SourceId>(), std::mem::size_of::<u64>());
    }

    #[test]
    fn revision_family_reuses_only_the_unchanged_current_source_value() {
        let domain = SourceIdentityDomain::new();
        let mut first = SourceMap::in_identity_domain(domain.clone());
        let first_id = first
            .add_bytes(SourceName::new("source.nct"), b"first")
            .unwrap();
        let mut unchanged = SourceMap::in_identity_domain(domain.clone());
        let unchanged_id = unchanged
            .add_bytes(SourceName::new("source.nct"), b"first")
            .unwrap();
        let mut changed = SourceMap::in_identity_domain(domain.clone());
        let changed_id = changed
            .add_bytes(SourceName::new("source.nct"), b"second")
            .unwrap();
        let mut restored = SourceMap::in_identity_domain(domain);
        let restored_id = restored
            .add_bytes(SourceName::new("source.nct"), b"first")
            .unwrap();

        assert_eq!(first_id, unchanged_id);
        assert_ne!(unchanged_id, changed_id);
        assert_ne!(first_id, restored_id);
        assert!(unchanged.get(first_id).is_some());
        assert!(changed.get(first_id).is_none());
        assert!(restored.get(first_id).is_none());
    }

    #[test]
    fn independent_identity_domains_do_not_alias_equal_source_values() {
        let mut first = SourceMap::new();
        let first_id = first
            .add_bytes(SourceName::new("source.nct"), b"same")
            .unwrap();
        let mut second = SourceMap::new();
        let second_id = second
            .add_bytes(SourceName::new("source.nct"), b"same")
            .unwrap();

        assert_ne!(first_id, second_id);
        assert!(first.get(second_id).is_none());
        assert!(second.get(first_id).is_none());
    }

    #[test]
    fn identity_domain_debug_output_does_not_expose_admitted_sources() {
        let domain = SourceIdentityDomain::new();
        let mut sources = SourceMap::in_identity_domain(domain.clone());
        sources
            .add_bytes(SourceName::new("secret.nct"), b"private source text")
            .unwrap();

        assert_eq!(format!("{domain:?}"), "SourceIdentityDomain");
    }

    #[test]
    fn source_order_uses_the_current_map_index_not_identity_history() {
        let domain = SourceIdentityDomain::new();
        let mut historical = SourceMap::in_identity_domain(domain.clone());
        historical
            .add_bytes(SourceName::new("later.nct"), b"later")
            .unwrap();
        historical
            .add_bytes(SourceName::new("earlier.nct"), b"earlier")
            .unwrap();

        let mut current = SourceMap::in_identity_domain(domain);
        let earlier = current
            .add_bytes(SourceName::new("earlier.nct"), b"earlier")
            .unwrap();
        let later = current
            .add_bytes(SourceName::new("later.nct"), b"later")
            .unwrap();
        let mut ordered = [later, earlier];
        ordered.sort_unstable();

        assert_eq!(ordered, [earlier, later]);
    }

    #[test]
    fn cloned_maps_can_diverge_without_aliasing_new_source_identities() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("source.nct"), b"first")
            .unwrap();
        let mut snapshot = sources.clone();

        assert_eq!(snapshot.get(source).map(SourceFile::text), Some("first"));
        let second = sources
            .add_bytes(SourceName::new("second.nct"), b"second")
            .unwrap();
        let third = snapshot
            .add_bytes(SourceName::new("third.nct"), b"third")
            .unwrap();

        assert_eq!(second.index(), third.index());
        assert_ne!(second, third);
        assert!(sources.get(third).is_none());
        assert!(snapshot.get(second).is_none());
        assert_eq!(sources.get(second).map(SourceFile::text), Some("second"));
        assert_eq!(snapshot.get(third).map(SourceFile::text), Some("third"));
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

    #[test]
    fn converts_utf16_positions_without_splitting_scalars() {
        let mut sources = SourceMap::new();
        let id = sources
            .add_bytes(
                SourceName::new("unicode.nct"),
                "a😀βe\u{301}\nplain\n".as_bytes(),
            )
            .unwrap();
        let source = sources.get(id).unwrap();

        let boundaries = [
            (0, Utf16Position::new(0, 0)),
            (1, Utf16Position::new(0, 1)),
            (5, Utf16Position::new(0, 3)),
            (7, Utf16Position::new(0, 4)),
            (8, Utf16Position::new(0, 5)),
            (10, Utf16Position::new(0, 6)),
            (11, Utf16Position::new(1, 0)),
            (16, Utf16Position::new(1, 5)),
            (17, Utf16Position::new(2, 0)),
        ];
        for (byte, position) in boundaries {
            assert_eq!(source.utf16_position(ByteOffset::new(byte)), Ok(position));
            assert_eq!(source.byte_offset(position), Ok(ByteOffset::new(byte)));
        }

        assert_eq!(
            source.utf16_position(ByteOffset::new(2)),
            Err(CoordinateError::NotUtf8Boundary {
                offset: ByteOffset::new(2),
            })
        );
        assert_eq!(
            source.byte_offset(Utf16Position::new(0, 2)),
            Err(CoordinateError::SplitUtf16Scalar {
                position: Utf16Position::new(0, 2),
            })
        );
    }

    #[test]
    fn validates_utf16_lines_characters_and_ranges() {
        let mut sources = SourceMap::new();
        let id = sources
            .add_bytes(SourceName::new("ranges.nct"), b"one\r\ntwo")
            .unwrap();
        let source = sources.get(id).unwrap();

        assert_eq!(
            source.byte_offset(Utf16Position::new(0, 4)),
            Err(CoordinateError::CharacterOutOfBounds {
                position: Utf16Position::new(0, 4),
                line_utf16_len: 3,
            })
        );
        assert_eq!(
            source.byte_offset(Utf16Position::new(2, 0)),
            Err(CoordinateError::LineOutOfBounds {
                line: 2,
                line_count: 2,
            })
        );
        assert_eq!(
            source.utf16_position(ByteOffset::new(8)),
            Err(CoordinateError::ByteOutOfBounds {
                offset: ByteOffset::new(8),
                source_len: ByteOffset::new(7),
            })
        );

        let utf16 = Utf16Range::new(Utf16Position::new(0, 1), Utf16Position::new(1, 2));
        let bytes = TextRange::new(ByteOffset::new(1), ByteOffset::new(6));
        assert_eq!(source.text_range(utf16), Ok(bytes));
        assert_eq!(source.utf16_range(bytes), Ok(utf16));

        let reversed = Utf16Range::new(Utf16Position::new(1, 0), Utf16Position::new(0, 0));
        assert_eq!(
            source.text_range(reversed),
            Err(CoordinateError::ReversedRange { range: reversed })
        );
    }
}

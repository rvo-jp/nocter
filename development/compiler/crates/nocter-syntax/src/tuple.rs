/// One source-authored structural-tuple element position.
///
/// Tuple positions have a narrower spelling than integer literals: only canonical ASCII decimal
/// digits are accepted. Radix prefixes, digit separators, and leading zeroes are not positions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TupleElementIndex(usize);

impl TupleElementIndex {
    /// Decodes one complete tuple-element suffix spelling.
    ///
    /// # Errors
    ///
    /// Returns [`TupleElementIndexError::InvalidSpelling`] when the spelling is not canonical
    /// decimal syntax, and [`TupleElementIndexError::OutOfRange`] when its value cannot be
    /// represented by the compiler's sequence-index domain.
    pub fn from_spelling(spelling: &str) -> Result<Self, TupleElementIndexError> {
        let bytes = spelling.as_bytes();
        if bytes.is_empty()
            || !bytes.iter().all(u8::is_ascii_digit)
            || (bytes.len() > 1 && bytes[0] == b'0')
        {
            return Err(TupleElementIndexError::InvalidSpelling);
        }
        spelling
            .parse::<usize>()
            .map(Self)
            .map_err(|_| TupleElementIndexError::OutOfRange)
    }

    #[must_use]
    pub const fn position(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TupleElementIndexError {
    InvalidSpelling,
    OutOfRange,
}

#[cfg(test)]
mod tests {
    use super::{TupleElementIndex, TupleElementIndexError};

    #[test]
    fn accepts_only_canonical_representable_decimal_positions() {
        assert_eq!(TupleElementIndex::from_spelling("0").unwrap().position(), 0);
        assert_eq!(
            TupleElementIndex::from_spelling("10").unwrap().position(),
            10
        );

        for spelling in ["", "00", "01", "1_0", "0x10", "-1"] {
            assert_eq!(
                TupleElementIndex::from_spelling(spelling),
                Err(TupleElementIndexError::InvalidSpelling),
                "{spelling}"
            );
        }
        assert_eq!(
            TupleElementIndex::from_spelling(
                "9999999999999999999999999999999999999999999999999999999999999999"
            ),
            Err(TupleElementIndexError::OutOfRange)
        );
    }
}

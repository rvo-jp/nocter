//! Finite homogeneous cleanup spans, independent of index backing and physical storage.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ArrayRange<I> {
    /// A nonempty checked half-open interval.
    Constant { start: u64, end: u64 },
    /// [0, index), where the retained index is below the containing array's length.
    Before(I),
    /// (index, length), under the same retained normal-bounds evidence.
    After(I),
}
impl<I> ArrayRange<I> {
    /// Project backing identity without interpreting it as a numeric index.
    /// # Errors
    /// Returns the index projection's error, if this span uses an index.
    pub fn try_map_index<J, E>(
        self,
        map: impl FnOnce(I) -> Result<J, E>,
    ) -> Result<ArrayRange<J>, E> {
        Ok(match self {
            Self::Constant { start, end } => ArrayRange::Constant { start, end },
            Self::Before(index) => ArrayRange::Before(map(index)?),
            Self::After(index) => ArrayRange::After(map(index)?),
        })
    }
    /// Resolve one retained numeric value for an execution consumer. This is integrity of an
    /// existing cleanup contract, never a replacement for the original source bounds check.
    /// Dynamic spans may be empty; a successor cannot overflow because index < length <= MAX.
    #[must_use]
    pub fn resolve(
        &self,
        length: u64,
        index: impl FnOnce(&I) -> Option<u64>,
    ) -> Option<std::ops::Range<u64>> {
        match self {
            Self::Constant { start, end } => {
                (start < end && *end <= length).then_some(*start..*end)
            }
            Self::Before(key) | Self::After(key) => {
                let value = index(key)?;
                if value >= length {
                    return None;
                }
                Some(match self {
                    Self::Before(_) => 0..value,
                    Self::After(_) => value.checked_add(1)?..length,
                    Self::Constant { .. } => unreachable!(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_reverse_spans_visit_every_survivor_once() {
        for length in 1..=8 {
            for hole in 0..length {
                let visited: Vec<_> = [ArrayRange::After(()), ArrayRange::Before(())]
                    .into_iter()
                    .flat_map(|span| span.resolve(length, |()| Some(hole)).unwrap().rev())
                    .collect();
                let expected: Vec<_> = (0..length).rev().filter(|&index| index != hole).collect();
                assert_eq!(visited, expected);
            }
        }
    }
    #[test]
    fn empty_sides_and_maximum_successor_preserve_exact_bounds() {
        assert_eq!(ArrayRange::Before(()).resolve(1, |()| Some(0)), Some(0..0));
        assert_eq!(ArrayRange::After(()).resolve(1, |()| Some(0)), Some(1..1));
        assert_eq!(
            ArrayRange::After(()).resolve(u64::MAX, |()| Some(u64::MAX - 1)),
            Some(u64::MAX..u64::MAX)
        );
        for length in [0, 1, u64::MAX] {
            assert_eq!(
                ArrayRange::After(()).resolve(length, |()| Some(length)),
                None
            );
        }
        assert_eq!(ArrayRange::Before(()).resolve(2, |()| None), None);
        assert_eq!(
            ArrayRange::<()>::Constant { start: 1, end: 1 }
                .resolve(2, |()| panic!("constant span has no index")),
            None
        );
    }
}

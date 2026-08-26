use std::fmt;

use nocter_persistent::{PersistentVector, PersistentVectorIter};

use crate::ClosureId;
use crate::id::SemanticId;

/// A structurally shared sequence whose positions are canonical closure identities.
///
/// This is the model-level identity contract used by closure construction authorities. Its
/// persistent storage representation stays private and may change without affecting checking.
#[derive(Clone)]
pub struct ClosureSequence<T> {
    values: PersistentVector<T>,
}

impl<T> Default for ClosureSequence<T> {
    fn default() -> Self {
        Self {
            values: PersistentVector::default(),
        }
    }
}

impl<T> fmt::Debug for ClosureSequence<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClosureSequence")
            .field("closure_count", &self.values.len())
            .finish()
    }
}

impl<T> ClosureSequence<T> {
    #[must_use]
    pub fn get(&self, id: ClosureId) -> Option<&T> {
        self.values.get(id.index())
    }

    #[must_use]
    pub fn iter(&self) -> ClosureSequenceIter<'_, T> {
        ClosureSequenceIter {
            values: self.values.iter(),
            next: 0,
        }
    }

    #[must_use]
    pub fn next_id(&self) -> ClosureId {
        ClosureId::new(self.values.len())
    }

    pub fn insert(&mut self, value: T) -> ClosureId {
        let id = self.next_id();
        self.values.push(value);
        id
    }

    /// Replaces one known closure value while preserving every shared branch.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownClosureSequenceId`] when `id` is outside this sequence.
    pub fn replace(&mut self, id: ClosureId, value: T) -> Result<(), UnknownClosureSequenceId> {
        self.values
            .set(id.index(), value)
            .ok_or(UnknownClosureSequenceId(id))
    }
}

impl<'a, T> IntoIterator for &'a ClosureSequence<T> {
    type IntoIter = ClosureSequenceIter<'a, T>;
    type Item = (ClosureId, &'a T);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct ClosureSequenceIter<'a, T> {
    values: PersistentVectorIter<'a, T>,
    next: usize,
}

impl<'a, T> Iterator for ClosureSequenceIter<'a, T> {
    type Item = (ClosureId, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.values.next()?;
        let id = ClosureId::new(self.next);
        self.next += 1;
        Some((id, value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}

impl<T> ExactSizeIterator for ClosureSequenceIter<'_, T> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownClosureSequenceId(pub ClosureId);

#[cfg(test)]
mod tests {
    use super::ClosureSequence;

    #[test]
    fn sibling_replacements_preserve_the_shared_prefix() {
        let mut base = ClosureSequence::default();
        let first_id = base.insert(1);
        let second_id = base.insert(2);
        let mut first = base.clone();
        let mut second = base.clone();

        first.replace(second_id, 20).unwrap();
        second.replace(second_id, 30).unwrap();

        assert_eq!(base.get(first_id), Some(&1));
        assert_eq!(base.get(second_id), Some(&2));
        assert_eq!(first.get(second_id), Some(&20));
        assert_eq!(second.get(second_id), Some(&30));
    }
}

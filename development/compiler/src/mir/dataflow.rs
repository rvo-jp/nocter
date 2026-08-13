//! Dense sets over typed identity domains local to one MIR body.

use super::{LoanId, LocalId, ProjectionPathId};
use std::marker::PhantomData;

pub(super) trait DenseId: Copy {
    fn index(self) -> usize;
}

impl DenseId for LocalId {
    fn index(self) -> usize {
        self.index()
    }
}

impl DenseId for LoanId {
    fn index(self) -> usize {
        self.index()
    }
}

impl DenseId for ProjectionPathId {
    fn index(self) -> usize {
        self.index()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DenseSet<I> {
    words: Vec<u64>,
    identity: PhantomData<I>,
}

impl<I: DenseId> DenseSet<I> {
    pub(super) fn new(identity_count: usize) -> Self {
        Self {
            words: vec![0; identity_count.div_ceil(u64::BITS as usize)],
            identity: PhantomData,
        }
    }

    pub(super) fn insert(&mut self, identity: I) {
        let word = identity.index() / u64::BITS as usize;
        let bit = identity.index() % u64::BITS as usize;
        if let Some(word) = self.words.get_mut(word) {
            *word |= 1 << bit;
        }
    }

    pub(super) fn contains(&self, identity: I) -> bool {
        let word = identity.index() / u64::BITS as usize;
        let bit = identity.index() % u64::BITS as usize;
        self.words
            .get(word)
            .is_some_and(|word| word & (1 << bit) != 0)
    }

    pub(super) fn remove(&mut self, identity: I) {
        let word = identity.index() / u64::BITS as usize;
        let bit = identity.index() % u64::BITS as usize;
        if let Some(word) = self.words.get_mut(word) {
            *word &= !(1 << bit);
        }
    }

    /// Intersects `self` with `other` and reports whether the set changed.
    pub(super) fn intersect_with(&mut self, other: &Self) -> bool {
        debug_assert_eq!(self.words.len(), other.words.len());
        let mut changed = false;
        for (word, other) in self.words.iter_mut().zip(&other.words) {
            let intersection = *word & *other;
            changed |= intersection != *word;
            *word = intersection;
        }
        changed
    }

    /// Unions `self` with `other` and reports whether the set changed.
    pub(super) fn union_with(&mut self, other: &Self) -> bool {
        debug_assert_eq!(self.words.len(), other.words.len());
        let mut changed = false;
        for (word, other) in self.words.iter_mut().zip(&other.words) {
            let union = *word | *other;
            changed |= union != *word;
            *word = union;
        }
        changed
    }
}

pub(super) type LocalSet = DenseSet<LocalId>;
pub(super) type LoanSet = DenseSet<LoanId>;
pub(super) type ProjectionSet = DenseSet<ProjectionPathId>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_local_ids_across_word_boundaries() {
        let mut set = LocalSet::new(130);
        for index in [0, 63, 64, 129] {
            set.insert(LocalId::from_index(index));
        }

        for index in [0, 63, 64, 129] {
            assert!(set.contains(LocalId::from_index(index)));
        }
        set.remove(LocalId::from_index(64));
        assert!(!set.contains(LocalId::from_index(64)));
        for index in [1, 62, 65, 128, 130] {
            assert!(!set.contains(LocalId::from_index(index)));
        }
    }

    #[test]
    fn intersection_reports_only_real_changes() {
        let mut left = LocalSet::new(70);
        left.insert(LocalId::from_index(1));
        left.insert(LocalId::from_index(65));
        let mut right = LocalSet::new(70);
        right.insert(LocalId::from_index(65));

        assert!(left.intersect_with(&right));
        assert!(!left.contains(LocalId::from_index(1)));
        assert!(left.contains(LocalId::from_index(65)));
        assert!(!left.intersect_with(&right));
    }
}

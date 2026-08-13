//! Dense sets over one MIR body's `LocalId` domain.

use super::LocalId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LocalSet {
    words: Vec<u64>,
}

impl LocalSet {
    pub(super) fn new(local_count: usize) -> Self {
        Self {
            words: vec![0; local_count.div_ceil(u64::BITS as usize)],
        }
    }

    pub(super) fn insert(&mut self, local: LocalId) {
        let word = local.index() / u64::BITS as usize;
        let bit = local.index() % u64::BITS as usize;
        if let Some(word) = self.words.get_mut(word) {
            *word |= 1 << bit;
        }
    }

    pub(super) fn contains(&self, local: LocalId) -> bool {
        let word = local.index() / u64::BITS as usize;
        let bit = local.index() % u64::BITS as usize;
        self.words
            .get(word)
            .is_some_and(|word| word & (1 << bit) != 0)
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
}

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

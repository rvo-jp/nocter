//! Define-once slots with cheap immutable views over one fixed semantic identity domain.
use crate::id::SemanticId;
use nocter_persistent::{PersistentVector, PersistentVectorIter};
use std::{marker::PhantomData, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SparseInsertionError {
    UnknownId,
    AlreadyDefined,
}

/// A read-only view of the values already defined in a reserved identity domain.
/// Missing slots contain no provisional value and do not participate in iteration.
#[derive(Debug)]
pub struct SparseArena<I, T> {
    values: PersistentVector<Option<T>>,
    defined: usize,
    family: Arc<()>,
    identity: PhantomData<fn() -> I>,
}
impl<I, T> Clone for SparseArena<I, T> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            defined: self.defined,
            family: self.family.clone(),
            identity: PhantomData,
        }
    }
}
#[allow(private_bounds)]
impl<I: SemanticId, T> SparseArena<I, T> {
    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        self.values.get(id.index())?.as_ref()
    }
    #[must_use]
    pub const fn len(&self) -> usize {
        self.defined
    }
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.defined == 0
    }
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.defined == self.values.len()
    }
    /// Exact ancestry of define-once views, independently of structural value equality.
    /// Only the single non-cloneable builder can add slots to this family.
    #[must_use]
    pub fn preserves(&self, older: &Self) -> bool {
        Arc::ptr_eq(&self.family, &older.family) && self.defined >= older.defined
    }
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (I, &T)> {
        Iter {
            values: self.values.iter().enumerate(),
            remaining: self.defined,
            identity: PhantomData,
        }
    }
}
#[allow(private_bounds)]
impl<I: SemanticId + PartialEq, T: PartialEq> PartialEq for SparseArena<I, T> {
    fn eq(&self, other: &Self) -> bool {
        self.values.len() == other.values.len()
            && self.len() == other.len()
            && self.iter().eq(other.iter())
    }
}
#[allow(private_bounds)]
impl<I: SemanticId + Eq, T: Eq> Eq for SparseArena<I, T> {}

struct Iter<'a, I, T> {
    values: std::iter::Enumerate<PersistentVectorIter<'a, Option<T>>>,
    remaining: usize,
    identity: PhantomData<fn() -> I>,
}
impl<'a, I: SemanticId, T> Iterator for Iter<'a, I, T> {
    type Item = (I, &'a T);
    fn next(&mut self) -> Option<Self::Item> {
        for (index, value) in self.values.by_ref() {
            if let Some(value) = value {
                self.remaining -= 1;
                return Some((I::new(index), value));
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<I: SemanticId, T> ExactSizeIterator for Iter<'_, I, T> {}

/// Sole writer for a fixed domain. It cannot be cloned or reconstructed from an immutable view.
#[derive(Debug)]
pub struct SparseArenaBuilder<I, T> {
    current: SparseArena<I, T>,
}
#[allow(private_bounds)]
impl<I: SemanticId, T> SparseArenaBuilder<I, T> {
    #[must_use]
    pub fn new(reserved_count: usize) -> Self {
        let mut values = PersistentVector::default();
        for _ in 0..reserved_count {
            values.push(None);
        }
        Self {
            current: SparseArena {
                values,
                defined: 0,
                family: Arc::new(()),
                identity: PhantomData,
            },
        }
    }
    /// # Errors
    /// Rejects an unknown or already defined identity without replacing any value.
    /// # Panics
    /// Panics only if persistent storage violates its previously checked slot bound.
    pub fn insert(&mut self, id: I, value: T) -> Result<(), SparseInsertionError> {
        match self.current.values.get(id.index()) {
            None => return Err(SparseInsertionError::UnknownId),
            Some(Some(_)) => return Err(SparseInsertionError::AlreadyDefined),
            Some(None) => {}
        }
        self.current
            .values
            .set(id.index(), Some(value))
            .expect("reserved sparse slot");
        self.current.defined += 1;
        Ok(())
    }
    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        self.current.get(id)
    }
    #[must_use]
    pub fn snapshot(&self) -> SparseArena<I, T> {
        self.current.clone()
    }
    #[must_use]
    pub fn finish(self) -> SparseArena<I, T> {
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccessCallableId, ArenaBuilder};
    #[test]
    fn sparse_views_keep_holes_and_exact_ancestry_without_replacing_values() {
        let mut names = ArenaBuilder::<AccessCallableId, ()>::new();
        let ids: Vec<_> = (0..130).map(|_| names.insert(())).collect();
        let mut writer = SparseArenaBuilder::new(ids.len());
        let empty = writer.snapshot();
        for index in (0..130).rev().step_by(2) {
            writer.insert(ids[index], index).unwrap();
        }
        let first = writer.snapshot();
        assert!(empty.is_empty());
        assert_eq!(first.iter().len(), 65);
        assert_eq!(
            first.iter().map(|(_, &value)| value).collect::<Vec<_>>(),
            (1..130).step_by(2).collect::<Vec<_>>()
        );
        assert!(first.get(ids[0]).is_none());
        assert_eq!(
            writer.insert(ids[1], 900),
            Err(SparseInsertionError::AlreadyDefined)
        );
        writer.insert(ids[0], 0).unwrap();
        let second = writer.finish();
        assert_eq!(first.get(ids[0]), None);
        assert_eq!(second.get(ids[0]), Some(&0));
        assert_eq!(second.get(ids[1]), Some(&1));
        assert!(second.preserves(&first) && first.preserves(&empty));
        assert!(!first.preserves(&second));
        assert!(!second.is_complete());
        let other = SparseArenaBuilder::<AccessCallableId, usize>::new(ids.len()).finish();
        assert!(!other.preserves(&empty));
    }
}

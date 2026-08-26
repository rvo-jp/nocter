use std::marker::PhantomData;

use nocter_persistent::{PersistentVector, PersistentVectorIter};

use crate::id::SemanticId;

/// A structurally shared dense sequence addressed by one semantic ID domain.
///
/// Semantic authorities keep this collection private and expose immutable domain-specific views.
/// Cloning shares the complete root; append and replacement copy only one 32-way tree path.
#[derive(Clone, Debug)]
pub struct PersistentArena<I, T> {
    values: PersistentVector<T>,
    identity: PhantomData<fn() -> I>,
}

impl<I, T> Default for PersistentArena<I, T> {
    fn default() -> Self {
        Self {
            values: PersistentVector::default(),
            identity: PhantomData,
        }
    }
}

#[allow(private_bounds)]
impl<I: SemanticId, T> PersistentArena<I, T> {
    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        self.values.get(id.index())
    }

    #[must_use]
    pub fn iter(&self) -> PersistentArenaIter<'_, I, T> {
        PersistentArenaIter {
            values: self.values.iter(),
            next: 0,
            identity: PhantomData,
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use]
    pub fn next_id(&self) -> I {
        I::new(self.values.len())
    }

    pub fn insert(&mut self, value: T) -> I {
        let id = self.next_id();
        self.values.push(value);
        id
    }

    /// Replaces the value assigned to `id` while preserving all shared branches.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownPersistentId`] when `id` is outside this arena.
    pub fn replace(&mut self, id: I, value: T) -> Result<(), UnknownPersistentId<I>> {
        self.values
            .set(id.index(), value)
            .ok_or(UnknownPersistentId(id))
    }
}

#[allow(private_bounds)]
impl<'a, I: SemanticId, T> IntoIterator for &'a PersistentArena<I, T> {
    type IntoIter = PersistentArenaIter<'a, I, T>;
    type Item = (I, &'a T);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct PersistentArenaIter<'a, I, T> {
    values: PersistentVectorIter<'a, T>,
    next: usize,
    identity: PhantomData<fn() -> I>,
}

#[allow(private_bounds)]
impl<'a, I: SemanticId, T> Iterator for PersistentArenaIter<'a, I, T> {
    type Item = (I, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.values.next()?;
        let id = I::new(self.next);
        self.next += 1;
        Some((id, value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}

impl<I: SemanticId, T> ExactSizeIterator for PersistentArenaIter<'_, I, T> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownPersistentId<I>(pub I);

#[cfg(test)]
mod tests {
    use crate::{BodyId, PersistentArena};

    #[test]
    fn sibling_replacements_preserve_the_shared_prefix() {
        let mut base = PersistentArena::<BodyId, _>::default();
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

use std::marker::PhantomData;

use crate::id::SemanticId;

/// An immutable dense arena addressed by one semantic ID domain.
///
/// Arena position is storage, not semantic meaning. Callers that need deterministic IDs must feed
/// the corresponding builder in canonical order; semantic algorithms must not select candidates
/// by arena iteration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arena<I, T> {
    values: Box<[T]>,
    identity: PhantomData<fn() -> I>,
}

impl<I, T> Default for Arena<I, T> {
    fn default() -> Self {
        Self {
            values: Box::new([]),
            identity: PhantomData,
        }
    }
}

#[allow(private_bounds)]
impl<I: SemanticId, T> Arena<I, T> {
    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        self.values.get(id.index())
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (I, &T)> {
        self.values
            .iter()
            .enumerate()
            .map(|(index, value)| (I::new(index), value))
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// The single mutable construction path for an [`Arena`].
///
/// Finishing consumes the builder so an arena cannot be mutated after it enters a program.
#[derive(Debug)]
pub struct ArenaBuilder<I, T> {
    values: Vec<T>,
    identity: PhantomData<fn() -> I>,
}

impl<I, T> Default for ArenaBuilder<I, T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            identity: PhantomData,
        }
    }
}

#[allow(private_bounds)]
impl<I: SemanticId, T> ArenaBuilder<I, T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, value: T) -> I {
        let id = I::new(self.values.len());
        self.values.push(value);
        id
    }

    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        self.values.get(id.index())
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
    pub fn finish(self) -> Arena<I, T> {
        Arena {
            values: self.values.into_boxed_slice(),
            identity: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ArenaBuilder, ModuleId, PackageId};

    #[test]
    fn typed_ids_cannot_cross_arena_domains() {
        let mut packages = ArenaBuilder::<PackageId, _>::new();
        let first = packages.insert("root");
        let second = packages.insert("std");
        let packages = packages.finish();

        assert_eq!(packages.get(first), Some(&"root"));
        assert_eq!(packages.get(second), Some(&"std"));
        assert_eq!(
            packages.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            ["root", "std"]
        );

        let mut modules = ArenaBuilder::<ModuleId, _>::new();
        let root = modules.insert("/");
        assert_eq!(modules.finish().get(root), Some(&"/"));
    }

    #[test]
    fn finished_arena_has_no_mutation_path() {
        let mut builder = ArenaBuilder::<PackageId, _>::new();
        let package = builder.insert(7);
        let arena = builder.finish();

        assert_eq!(arena.get(package), Some(&7));
        assert_eq!(arena.len(), 1);
        assert!(!arena.is_empty());
    }
}

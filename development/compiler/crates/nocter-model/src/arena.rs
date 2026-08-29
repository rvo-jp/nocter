use std::marker::PhantomData;

use crate::construction_identity::ConstructionIdentity;
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

    /// Transforms every immutable slot while preserving its typed identity domain.
    ///
    /// # Errors
    ///
    /// Returns the first transformation error without producing a partial arena.
    pub fn try_map<U, E>(
        &self,
        mut transform: impl FnMut(I, &T) -> Result<U, E>,
    ) -> Result<Arena<I, U>, E> {
        let values = self
            .iter()
            .map(|(id, value)| transform(id, value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Arena {
            values: values.into_boxed_slice(),
            identity: PhantomData,
        })
    }
}

/// The single mutable construction path for an [`Arena`].
///
/// Finishing consumes the builder so an arena cannot be mutated after it enters a program.
#[derive(Debug)]
pub struct ArenaBuilder<I, T> {
    values: Vec<T>,
    construction: ConstructionIdentity,
    identity: PhantomData<fn() -> I>,
}

/// An opaque append position used to discard provisional arena construction.
#[derive(Debug, Eq, PartialEq)]
pub struct ArenaCheckpoint<I, T> {
    len: usize,
    construction: ConstructionIdentity,
    identity: PhantomData<fn() -> (I, T)>,
}

impl<I, T> Clone for ArenaCheckpoint<I, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I, T> Copy for ArenaCheckpoint<I, T> {}

impl<I, T: Clone> Clone for ArenaBuilder<I, T> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            construction: ConstructionIdentity::fresh(),
            identity: PhantomData,
        }
    }
}

impl<I, T> Default for ArenaBuilder<I, T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            construction: ConstructionIdentity::fresh(),
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

    /// Returns the identity assigned by the next insertion without reserving a slot.
    ///
    /// This is useful when two immutable arena values refer to each other. The caller must consume
    /// the identity before performing another insertion into this builder.
    #[must_use]
    pub fn next_id(&self) -> I {
        I::new(self.values.len())
    }

    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        self.values.get(id.index())
    }

    #[must_use]
    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.values.get_mut(id.index())
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Captures the current append boundary without cloning stored values.
    #[must_use]
    pub const fn checkpoint(&self) -> ArenaCheckpoint<I, T> {
        ArenaCheckpoint {
            len: self.values.len(),
            construction: self.construction,
            identity: PhantomData,
        }
    }

    /// Discards every value appended after `checkpoint`.
    ///
    /// # Panics
    ///
    /// Panics when the checkpoint is newer than this builder's current state.
    pub fn rollback(&mut self, checkpoint: ArenaCheckpoint<I, T>) {
        assert_eq!(
            checkpoint.construction, self.construction,
            "arena checkpoint belongs to another builder"
        );
        assert!(
            checkpoint.len <= self.values.len(),
            "arena checkpoint cannot be newer than the builder"
        );
        self.values.truncate(checkpoint.len);
    }

    #[must_use]
    pub fn finish(self) -> Arena<I, T> {
        Arena {
            values: self.values.into_boxed_slice(),
            identity: PhantomData,
        }
    }

    /// Transforms every slot while preserving its typed identity.
    ///
    /// # Errors
    ///
    /// Returns the first transformation error without producing a partial immutable arena.
    pub fn try_finish_with<U, E>(
        self,
        mut transform: impl FnMut(I, T) -> Result<U, E>,
    ) -> Result<Arena<I, U>, E> {
        let values = self
            .values
            .into_iter()
            .enumerate()
            .map(|(index, value)| transform(I::new(index), value))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Arena {
            values: values.into_boxed_slice(),
            identity: PhantomData,
        })
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
    fn immutable_transform_preserves_typed_identity_order() {
        let mut source = ArenaBuilder::<PackageId, _>::new();
        let first = source.insert(2_u32);
        let second = source.insert(3_u32);
        let source = source.finish();

        let mapped = source
            .try_map::<_, ()>(|id, value| Ok((id, value * 2)))
            .unwrap();

        assert_eq!(mapped.get(first), Some(&(first, 4)));
        assert_eq!(mapped.get(second), Some(&(second, 6)));
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

    #[test]
    fn rollback_discards_only_values_after_the_checkpoint() {
        let mut builder = ArenaBuilder::<PackageId, _>::new();
        let retained = builder.insert("retained");
        let checkpoint = builder.checkpoint();
        let discarded = builder.insert("discarded");

        builder.rollback(checkpoint);

        assert_eq!(builder.get(retained), Some(&"retained"));
        assert_eq!(builder.get(discarded), None);
        assert_eq!(builder.insert("replacement"), discarded);
    }

    #[test]
    #[should_panic(expected = "arena checkpoint belongs to another builder")]
    fn checkpoint_cannot_truncate_another_builder() {
        let first = ArenaBuilder::<PackageId, i32>::new();
        let checkpoint = first.checkpoint();
        let mut second = ArenaBuilder::<PackageId, i32>::new();
        second.insert(1);

        second.rollback(checkpoint);
    }
}

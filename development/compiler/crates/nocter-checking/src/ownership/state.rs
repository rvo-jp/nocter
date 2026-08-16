use std::collections::BTreeMap;

use nocter_model::FieldId;

use crate::PlaceRoot;

/// Canonical storage path used by ownership transfer and dataflow.
///
/// Only named fields may extend a root. Indexes and dereferences never enter this identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct MovePath {
    root: PlaceRoot,
    fields: Box<[FieldId]>,
}

impl MovePath {
    pub(crate) fn root(root: PlaceRoot) -> Self {
        Self {
            root,
            fields: Box::new([]),
        }
    }

    #[allow(dead_code, reason = "named-field move checking is the next consumer")]
    pub(crate) fn field(&self, field: FieldId) -> Self {
        let mut fields = self.fields.to_vec();
        fields.push(field);
        Self {
            root: self.root,
            fields: fields.into_boxed_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationState {
    Initialized,
    Uninitialized,
    MaybeInitialized,
}

impl InitializationState {
    fn join(self, another: Self) -> Self {
        if self == another {
            self
        } else {
            Self::MaybeInitialized
        }
    }
}

/// Flow state keyed only by semantic owned paths.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct OwnershipState {
    paths: BTreeMap<MovePath, InitializationState>,
}

impl OwnershipState {
    pub(crate) fn declare_initialized(
        &mut self,
        path: MovePath,
    ) -> Result<(), OwnershipStateError> {
        if self.paths.contains_key(&path) {
            return Err(OwnershipStateError::DuplicatePath(path));
        }
        self.paths.insert(path, InitializationState::Initialized);
        Ok(())
    }

    pub(crate) fn require_initialized(&self, path: &MovePath) -> Result<(), OwnershipStateError> {
        match self.paths.get(path).copied() {
            Some(InitializationState::Initialized) => Ok(()),
            Some(state) => Err(OwnershipStateError::NotInitialized {
                path: path.clone(),
                state,
            }),
            None => Err(OwnershipStateError::UnknownPath(path.clone())),
        }
    }

    pub(crate) fn move_out(&mut self, path: &MovePath) -> Result<(), OwnershipStateError> {
        self.require_initialized(path)?;
        self.paths
            .insert(path.clone(), InitializationState::Uninitialized);
        Ok(())
    }

    #[allow(dead_code, reason = "branch checking is the next consumer")]
    pub(crate) fn join_branches(
        &self,
        left: &Self,
        right: &Self,
    ) -> Result<Self, OwnershipStateError> {
        let mut joined = BTreeMap::new();
        for path in self.paths.keys() {
            let left = left
                .paths
                .get(path)
                .copied()
                .ok_or_else(|| OwnershipStateError::JoinPathMismatch(path.clone()))?;
            let right = right
                .paths
                .get(path)
                .copied()
                .ok_or_else(|| OwnershipStateError::JoinPathMismatch(path.clone()))?;
            joined.insert(path.clone(), left.join(right));
        }
        Ok(Self { paths: joined })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OwnershipStateError {
    DuplicatePath(MovePath),
    UnknownPath(MovePath),
    NotInitialized {
        path: MovePath,
        state: InitializationState,
    },
    JoinPathMismatch(MovePath),
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, LocalBindingId};

    use super::{InitializationState, MovePath, OwnershipState, OwnershipStateError};
    use crate::PlaceRoot;

    fn local_path() -> MovePath {
        let mut locals = ArenaBuilder::<LocalBindingId, _>::new();
        let local = locals.insert(());
        let _ = locals.finish();
        MovePath::root(PlaceRoot::Local(local))
    }

    #[test]
    fn move_transition_consumes_one_initialized_path() {
        let path = local_path();
        let mut state = OwnershipState::default();
        state.declare_initialized(path.clone()).unwrap();
        state.move_out(&path).unwrap();

        assert_eq!(
            state.require_initialized(&path),
            Err(OwnershipStateError::NotInitialized {
                path,
                state: InitializationState::Uninitialized,
            })
        );
    }

    #[test]
    fn control_flow_join_retains_uncertain_initialization() {
        let path = local_path();
        let mut initialized = OwnershipState::default();
        initialized.declare_initialized(path.clone()).unwrap();
        let mut moved = initialized.clone();
        moved.move_out(&path).unwrap();
        let joined = initialized.join_branches(&initialized, &moved).unwrap();

        assert_eq!(
            joined.require_initialized(&path),
            Err(OwnershipStateError::NotInitialized {
                path,
                state: InitializationState::MaybeInitialized,
            })
        );
    }

    #[test]
    fn branch_local_paths_do_not_escape_the_entry_state() {
        let mut locals = ArenaBuilder::<LocalBindingId, _>::new();
        let outer = MovePath::root(PlaceRoot::Local(locals.insert(())));
        let branch = MovePath::root(PlaceRoot::Local(locals.insert(())));
        let _ = locals.finish();
        let mut entry = OwnershipState::default();
        entry.declare_initialized(outer).unwrap();
        let mut left = entry.clone();
        left.declare_initialized(branch).unwrap();
        let joined = entry.join_branches(&left, &entry).unwrap();

        assert_eq!(joined, entry);
    }
}

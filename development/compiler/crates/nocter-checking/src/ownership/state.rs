use std::collections::{BTreeMap, BTreeSet};

use nocter_model::FieldId;

use crate::{CheckedPlace, PlaceAccess, PlaceProjection, PlaceRoot};

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

    pub(crate) fn field(&self, field: FieldId) -> Self {
        let mut fields = self.fields.to_vec();
        fields.push(field);
        Self {
            root: self.root,
            fields: fields.into_boxed_slice(),
        }
    }

    pub(crate) fn from_place(place: &CheckedPlace) -> Option<Self> {
        let mut path = Self::root(place.root());
        if place.access() != PlaceAccess::Owned {
            return Some(path);
        }
        for projection in place.projections() {
            let PlaceProjection::Field(field) = projection else {
                return None;
            };
            path = path.field(*field);
        }
        Some(path)
    }

    pub(crate) fn root_identity(&self) -> PlaceRoot {
        self.root
    }

    pub(crate) fn fields(&self) -> &[FieldId] {
        &self.fields
    }

    fn is_prefix_of(&self, another: &Self) -> bool {
        self.root == another.root
            && self.fields.len() <= another.fields.len()
            && another.fields.starts_with(&self.fields)
    }

    fn parent(&self) -> Option<Self> {
        let (_, fields) = self.fields.split_last()?;
        Some(Self {
            root: self.root,
            fields: fields.into(),
        })
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
        let state = self.state_at(path)?;
        if state != InitializationState::Initialized {
            return Err(OwnershipStateError::NotInitialized {
                path: path.clone(),
                state,
            });
        }
        if let Some(state) = self
            .paths
            .iter()
            .filter(|(candidate, _)| path != *candidate && path.is_prefix_of(candidate))
            .map(|(_, state)| *state)
            .find(|state| *state != InitializationState::Initialized)
        {
            return Err(OwnershipStateError::NotInitialized {
                path: path.clone(),
                state,
            });
        }
        Ok(())
    }

    pub(crate) fn move_out(&mut self, path: &MovePath) -> Result<(), OwnershipStateError> {
        self.require_initialized(path)?;
        self.paths
            .retain(|candidate, _| candidate == path || !path.is_prefix_of(candidate));
        self.paths
            .insert(path.clone(), InitializationState::Uninitialized);
        Ok(())
    }

    /// Replaces one writable owned path after its right-hand side has completed.
    ///
    /// A field may repair its own partial state, but it cannot synthesize storage below an
    /// unavailable parent. Replacing a complete path removes every more-specific partial fact.
    pub(crate) fn assign(&mut self, path: &MovePath) -> Result<(), OwnershipStateError> {
        let mut prefix = MovePath::root(path.root_identity());
        for field in path.fields() {
            let state = self.state_at(&prefix)?;
            if state != InitializationState::Initialized {
                return Err(OwnershipStateError::UnavailableAssignmentParent {
                    path: prefix,
                    state,
                });
            }
            prefix = prefix.field(*field);
        }
        self.paths
            .retain(|candidate, _| candidate == path || !path.is_prefix_of(candidate));
        self.paths
            .insert(path.clone(), InitializationState::Initialized);
        Ok(())
    }

    fn state_at(&self, path: &MovePath) -> Result<InitializationState, OwnershipStateError> {
        let mut current = Some(path.clone());
        while let Some(candidate) = current {
            if let Some(state) = self.paths.get(&candidate) {
                return Ok(*state);
            }
            current = candidate.parent();
        }
        Err(OwnershipStateError::UnknownPath(path.clone()))
    }

    pub(crate) fn join_reachable(&self, incoming: &[Self]) -> Result<Self, OwnershipStateError> {
        if incoming.is_empty() {
            return Ok(self.clone());
        }
        let roots = self
            .paths
            .keys()
            .map(MovePath::root_identity)
            .collect::<BTreeSet<_>>();
        let paths = self
            .paths
            .keys()
            .chain(incoming.iter().flat_map(|state| {
                state
                    .paths
                    .keys()
                    .filter(|path| roots.contains(&path.root_identity()))
            }))
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut joined = BTreeMap::new();
        for path in paths {
            let mut states = incoming.iter();
            let mut state = states
                .next()
                .expect("nonempty incoming state was checked")
                .state_at(&path)?;
            for incoming in states {
                state = state.join(incoming.state_at(&path)?);
            }
            joined.insert(path, state);
        }
        Ok(Self { paths: joined })
    }

    pub(crate) fn initialization(
        &self,
        path: &MovePath,
    ) -> Result<InitializationState, OwnershipStateError> {
        self.state_at(path)
    }

    pub(crate) fn has_descendant(&self, path: &MovePath) -> bool {
        self.paths
            .keys()
            .any(|candidate| candidate != path && path.is_prefix_of(candidate))
    }

    pub(crate) fn forget_root(&mut self, root: PlaceRoot) {
        self.paths.retain(|path, _| path.root_identity() != root);
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
    UnavailableAssignmentParent {
        path: MovePath,
        state: InitializationState,
    },
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, FieldId, LocalBindingId};

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
        let joined = initialized
            .join_reachable(&[initialized.clone(), moved])
            .unwrap();

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
        let joined = entry.join_reachable(&[left, entry.clone()]).unwrap();

        assert_eq!(joined, entry);
    }

    #[test]
    fn one_reachable_exit_still_filters_branch_local_paths() {
        let mut locals = ArenaBuilder::<LocalBindingId, _>::new();
        let outer = MovePath::root(PlaceRoot::Local(locals.insert(())));
        let branch = MovePath::root(PlaceRoot::Local(locals.insert(())));
        let _ = locals.finish();
        let mut entry = OwnershipState::default();
        entry.declare_initialized(outer).unwrap();
        let mut exit = entry.clone();
        exit.declare_initialized(branch).unwrap();
        let joined = entry.join_reachable(&[exit]).unwrap();

        assert_eq!(joined, entry);
    }

    #[test]
    fn field_move_preserves_disjoint_field_and_invalidates_parent() {
        let root = local_path();
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let first = root.field(fields.insert(()));
        let second = root.field(fields.insert(()));
        let _ = fields.finish();
        let mut state = OwnershipState::default();
        state.declare_initialized(root.clone()).unwrap();
        state.move_out(&first).unwrap();

        assert!(state.require_initialized(&second).is_ok());
        assert!(matches!(
            state.require_initialized(&root),
            Err(OwnershipStateError::NotInitialized { .. })
        ));
        assert!(matches!(
            state.require_initialized(&first),
            Err(OwnershipStateError::NotInitialized { .. })
        ));
    }

    #[test]
    fn field_state_joins_against_inherited_parent_state() {
        let root = local_path();
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let field = root.field(fields.insert(()));
        let _ = fields.finish();
        let mut entry = OwnershipState::default();
        entry.declare_initialized(root).unwrap();
        let mut moved = entry.clone();
        moved.move_out(&field).unwrap();
        let joined = entry.join_reachable(&[moved, entry.clone()]).unwrap();

        assert_eq!(
            joined.require_initialized(&field),
            Err(OwnershipStateError::NotInitialized {
                path: field,
                state: InitializationState::MaybeInitialized,
            })
        );
    }

    #[test]
    fn assignment_repairs_a_field_and_then_the_complete_parent() {
        let root = local_path();
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let field = root.field(fields.insert(()));
        let _ = fields.finish();
        let mut state = OwnershipState::default();
        state.declare_initialized(root.clone()).unwrap();
        state.move_out(&field).unwrap();

        state.assign(&field).unwrap();

        assert!(state.require_initialized(&field).is_ok());
        assert!(state.require_initialized(&root).is_ok());
    }

    #[test]
    fn field_assignment_cannot_recreate_a_moved_parent() {
        let root = local_path();
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let field = root.field(fields.insert(()));
        let _ = fields.finish();
        let mut state = OwnershipState::default();
        state.declare_initialized(root.clone()).unwrap();
        state.move_out(&root).unwrap();

        assert!(matches!(
            state.assign(&field),
            Err(OwnershipStateError::UnavailableAssignmentParent { .. })
        ));
    }
}

use std::collections::{BTreeMap, BTreeSet};

use nocter_model::{BodyNodeId, FieldId};

use crate::{CheckedPlace, PlaceAccess, PlaceProjection, PlaceRoot};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum TemporaryIdentity {
    Value(BodyNodeId),
    InterpolationInProgress(BodyNodeId),
    PatternResidual(BodyNodeId),
    PatternUnmatched(BodyNodeId),
}

/// Canonical storage path used by ownership transfer and dataflow.
///
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MoveProjection {
    Field(FieldId),
    TupleElement(usize),
}

/// Only statically selected aggregate positions may extend a root. Dynamic indexes and
/// dereferences never enter this identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct MovePath {
    root: PlaceRoot,
    projections: Box<[MoveProjection]>,
}

impl MovePath {
    pub(crate) fn root(root: PlaceRoot) -> Self {
        Self {
            root,
            projections: Box::new([]),
        }
    }

    pub(crate) fn field(&self, field: FieldId) -> Self {
        self.project(MoveProjection::Field(field))
    }

    pub(crate) fn tuple_element(&self, index: usize) -> Self {
        self.project(MoveProjection::TupleElement(index))
    }

    fn project(&self, projection: MoveProjection) -> Self {
        let mut projections = self.projections.to_vec();
        projections.push(projection);
        Self {
            root: self.root,
            projections: projections.into_boxed_slice(),
        }
    }

    pub(crate) fn from_place(place: &CheckedPlace) -> Option<Self> {
        let mut path = Self::root(place.root());
        if place.access() != PlaceAccess::Owned {
            return None;
        }
        for projection in place.projections() {
            path = match projection {
                PlaceProjection::Field { field, .. } => path.field(*field),
                PlaceProjection::TupleElement { index, .. } => path.tuple_element(*index),
                _ => return None,
            };
        }
        Some(path)
    }

    /// Returns the flow-tracked prefix that must remain initialized while using `place`.
    ///
    /// Static roots are immutable program storage, not invocation-owned storage. They are always
    /// available and therefore deliberately have no entry in `OwnershipState`.
    pub(crate) fn required_initialized(place: &CheckedPlace) -> Option<Self> {
        if matches!(place.root(), PlaceRoot::Static(_)) {
            return None;
        }
        if let Some(path) = Self::from_place(place) {
            return Some(path);
        }

        let mut path = Self::root(place.root());
        for projection in place.projections() {
            match projection {
                PlaceProjection::Field { field, .. } => path = path.field(*field),
                PlaceProjection::TupleElement { index, .. } => path = path.tuple_element(*index),
                PlaceProjection::BorrowDeref { .. }
                | PlaceProjection::BuiltinIndex { .. }
                | PlaceProjection::CoercedBuiltinIndex { .. }
                | PlaceProjection::SelectedIndex { .. } => break,
            }
        }
        Some(path)
    }

    pub(crate) fn root_identity(&self) -> PlaceRoot {
        self.root
    }

    pub(crate) fn projections(&self) -> &[MoveProjection] {
        &self.projections
    }

    fn is_prefix_of(&self, another: &Self) -> bool {
        self.root == another.root
            && self.projections.len() <= another.projections.len()
            && another.projections.starts_with(&self.projections)
    }

    fn parent(&self) -> Option<Self> {
        let (_, projections) = self.projections.split_last()?;
        Some(Self {
            root: self.root,
            projections: projections.into(),
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

/// Flow state for named storage and evaluated owned temporaries.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct OwnershipState {
    paths: BTreeMap<MovePath, InitializationState>,
    temporaries: BTreeMap<TemporaryIdentity, InitializationState>,
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
        for projection in path.projections() {
            let state = self.state_at(&prefix)?;
            if state != InitializationState::Initialized {
                return Err(OwnershipStateError::UnavailableAssignmentParent {
                    path: prefix,
                    state,
                });
            }
            prefix = prefix.project(*projection);
        }
        let explicit = self.paths.contains_key(path);
        let has_descendant = self
            .paths
            .keys()
            .any(|candidate| candidate != path && path.is_prefix_of(candidate));
        self.paths
            .retain(|candidate, _| candidate == path || !path.is_prefix_of(candidate));
        if explicit {
            self.paths
                .insert(path.clone(), InitializationState::Initialized);
        } else if has_descendant {
            // Removing the more-specific facts restores the initialized state inherited from the
            // nearest ancestor; retaining an explicit initialized child would be redundant.
            debug_assert_eq!(self.state_at(path)?, InitializationState::Initialized);
        }
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
        let temporary_ids = incoming
            .iter()
            .flat_map(|state| state.temporaries.keys().copied())
            .collect::<BTreeSet<_>>();
        let temporaries = temporary_ids
            .into_iter()
            .filter_map(|temporary| {
                let mut states = incoming.iter().map(|state| {
                    state
                        .temporaries
                        .get(&temporary)
                        .copied()
                        .unwrap_or(InitializationState::Uninitialized)
                });
                let first = states.next().expect("nonempty incoming state was checked");
                let joined = states.fold(first, InitializationState::join);
                (joined != InitializationState::Uninitialized).then_some((temporary, joined))
            })
            .collect();
        Ok(Self {
            paths: joined,
            temporaries,
        })
    }

    pub(crate) fn declare_temporary(
        &mut self,
        temporary: TemporaryIdentity,
    ) -> Result<(), OwnershipStateError> {
        if self
            .temporaries
            .insert(temporary, InitializationState::Initialized)
            .is_some()
        {
            return Err(OwnershipStateError::DuplicateTemporary(temporary));
        }
        Ok(())
    }

    pub(crate) fn consume_temporary(
        &mut self,
        temporary: TemporaryIdentity,
    ) -> Result<(), OwnershipStateError> {
        match self.temporaries.remove(&temporary) {
            Some(InitializationState::Initialized) => Ok(()),
            Some(InitializationState::Uninitialized | InitializationState::MaybeInitialized)
            | None => Err(OwnershipStateError::UnavailableTemporary(temporary)),
        }
    }

    pub(crate) fn temporary_initialization(
        &self,
        temporary: TemporaryIdentity,
    ) -> InitializationState {
        self.temporaries
            .get(&temporary)
            .copied()
            .unwrap_or(InitializationState::Uninitialized)
    }

    pub(crate) fn temporary_identities(&self) -> Vec<TemporaryIdentity> {
        self.temporaries.keys().copied().collect()
    }

    pub(crate) fn forget_temporaries_except(&mut self, retained: &[TemporaryIdentity]) {
        self.temporaries
            .retain(|temporary, _| retained.binary_search(temporary).is_ok());
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

    pub(crate) fn contains_root(&self, root: PlaceRoot) -> bool {
        self.paths.keys().any(|path| path.root_identity() == root)
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
    DuplicateTemporary(TemporaryIdentity),
    UnavailableTemporary(TemporaryIdentity),
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, BodyNodeId, FieldId, LocalBindingId};

    use super::{
        InitializationState, MovePath, OwnershipState, OwnershipStateError, TemporaryIdentity,
    };
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
    fn replacing_an_inherited_initialized_field_does_not_create_partial_state() {
        let root = local_path();
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let field = fields.insert(());
        let _ = fields.finish();
        let child = root.field(field);
        let mut state = OwnershipState::default();
        state.declare_initialized(root.clone()).unwrap();

        state.assign(&child).unwrap();

        assert!(!state.has_descendant(&root));
        assert_eq!(
            state.initialization(&child).unwrap(),
            InitializationState::Initialized
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
    fn branch_temporary_joins_as_conditionally_initialized() {
        let mut nodes = ArenaBuilder::<BodyNodeId, _>::new();
        let temporary = nodes.insert(());
        let _ = nodes.finish();
        let entry = OwnershipState::default();
        let mut branch = entry.clone();
        let temporary = TemporaryIdentity::Value(temporary);
        branch.declare_temporary(temporary).unwrap();
        let joined = entry.join_reachable(&[entry.clone(), branch]).unwrap();

        assert_eq!(
            joined.temporary_initialization(temporary),
            InitializationState::MaybeInitialized
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

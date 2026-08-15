use super::BorrowPlace;
use crate::source::ByteSpan;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub(super) struct PlaceStateForest {
    roots: HashMap<crate::resolve::LocalSymbolId, TrackedPlace>,
}

impl PlaceStateForest {
    pub(super) fn define_root(&mut self, symbol: crate::resolve::LocalSymbolId, state: PlaceState) {
        self.roots.insert(symbol, TrackedPlace::new(state));
    }

    pub(super) fn remove_root(&mut self, symbol: crate::resolve::LocalSymbolId) {
        self.roots.remove(&symbol);
    }

    pub(super) fn contains_root(&self, symbol: crate::resolve::LocalSymbolId) -> bool {
        self.roots.contains_key(&symbol)
    }

    pub(super) fn state(&self, place: &BorrowPlace) -> Option<PlaceState> {
        self.roots.get(&place.root)?.state(place.fields.as_deref())
    }

    pub(super) fn invalidate(&mut self, place: &BorrowPlace, state: PlaceState) {
        if let Some(root) = self.roots.get_mut(&place.root) {
            root.set(place.fields.as_deref(), state);
        }
    }

    pub(super) fn initialize(&mut self, place: &BorrowPlace, span: ByteSpan) {
        if let Some(root) = self.roots.get_mut(&place.root) {
            root.initialize(place.fields.as_deref(), span);
        }
    }

    pub(super) fn join_from(&mut self, branches: &[Self]) {
        if branches.is_empty() {
            return;
        }

        for (name, current) in &mut self.roots {
            let branch_places = branches
                .iter()
                .map(|branch| branch.roots.get(name).unwrap_or(current))
                .collect::<Vec<_>>();
            *current = TrackedPlace::join(&branch_places);
        }
    }
}

#[derive(Debug, Clone)]
struct TrackedPlace {
    root: PlaceState,
    descendants: HashMap<Vec<String>, PlaceState>,
}

impl TrackedPlace {
    fn new(root: PlaceState) -> Self {
        Self {
            root,
            descendants: HashMap::new(),
        }
    }

    fn state(&self, fields: Option<&[String]>) -> Option<PlaceState> {
        let Some(fields) = fields else {
            return Some(self.state_with_descendants(&[]));
        };
        Some(self.state_with_descendants(fields))
    }

    fn state_with_descendants(&self, fields: &[String]) -> PlaceState {
        let effective = self.effective_state(fields);
        if matches!(
            effective,
            PlaceState::MaybeInitialized { .. } | PlaceState::PartiallyInitialized { .. }
        ) {
            return effective;
        }

        self.descendants
            .iter()
            .filter(|(path, _)| is_strict_descendant(path, fields))
            .map(|(_, state)| *state)
            .find(|state| state.is_initialized() != effective.is_initialized())
            .map(PlaceState::partially_initialized_from)
            .unwrap_or(effective)
    }

    fn effective_state(&self, fields: &[String]) -> PlaceState {
        for end in (1..=fields.len()).rev() {
            if let Some(state) = self.descendants.get(&fields[..end]) {
                return *state;
            }
        }
        self.root
    }

    fn set(&mut self, fields: Option<&[String]>, state: PlaceState) {
        let Some(fields) = fields else {
            self.root = state;
            self.descendants.clear();
            return;
        };
        if fields.is_empty() {
            self.root = state;
            self.descendants.clear();
            return;
        }

        self.descendants.insert(fields.to_vec(), state);
        self.descendants
            .retain(|path, _| path.as_slice() == fields || !is_strict_descendant(path, fields));
        self.prune_redundant_descendants();
    }

    fn initialize(&mut self, fields: Option<&[String]>, span: ByteSpan) {
        let Some(fields) = fields else {
            self.root = PlaceState::Initialized { span };
            self.descendants.clear();
            return;
        };
        if fields.is_empty() {
            self.root = PlaceState::Initialized { span };
            self.descendants.clear();
            return;
        }

        self.descendants
            .retain(|path, _| path.as_slice() != fields && !is_strict_descendant(path, fields));
        if !self.effective_state(fields).is_initialized() {
            self.descendants
                .insert(fields.to_vec(), PlaceState::Initialized { span });
        }
        self.prune_redundant_descendants();
    }

    fn join(branches: &[&Self]) -> Self {
        debug_assert!(!branches.is_empty());
        let root = branches
            .iter()
            .skip(1)
            .fold(branches[0].root, |state, branch| {
                PlaceState::join(state, branch.root)
            });
        let paths = branches
            .iter()
            .flat_map(|branch| branch.descendants.keys().cloned())
            .collect::<HashSet<_>>();
        let mut joined = Self::new(root);

        for path in paths {
            let state = branches
                .iter()
                .skip(1)
                .fold(branches[0].effective_state(&path), |state, branch| {
                    PlaceState::join(state, branch.effective_state(&path))
                });
            joined.descendants.insert(path, state);
        }
        joined.prune_redundant_descendants();
        joined
    }

    fn prune_redundant_descendants(&mut self) {
        let mut paths = self.descendants.keys().cloned().collect::<Vec<_>>();
        paths.sort_by_key(Vec::len);
        for path in paths {
            let Some(state) = self.descendants.remove(&path) else {
                continue;
            };
            if self.effective_state(&path) != state {
                self.descendants.insert(path, state);
            }
        }
    }
}

fn is_strict_descendant(path: &[String], ancestor: &[String]) -> bool {
    path.len() > ancestor.len() && path.starts_with(ancestor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlaceState {
    Initialized { span: ByteSpan },
    Moved { span: ByteSpan },
    Dropped { span: ByteSpan },
    Uninitialized { span: ByteSpan },
    PartiallyInitialized { span: ByteSpan },
    MaybeInitialized { span: ByteSpan },
}

impl PlaceState {
    pub(super) fn join(left: Self, right: Self) -> Self {
        match (left, right) {
            (Self::Initialized { span }, Self::Initialized { .. }) => Self::Initialized { span },
            (Self::Moved { span }, Self::Moved { .. }) => Self::Moved { span },
            (Self::Dropped { span }, Self::Dropped { .. }) => Self::Dropped { span },
            (Self::Uninitialized { span }, Self::Uninitialized { .. }) => {
                Self::Uninitialized { span }
            }
            (Self::PartiallyInitialized { span }, Self::PartiallyInitialized { .. }) => {
                Self::PartiallyInitialized { span }
            }
            (
                Self::Moved { span } | Self::Dropped { span } | Self::Uninitialized { span },
                Self::Moved { .. } | Self::Dropped { .. } | Self::Uninitialized { .. },
            ) => Self::Uninitialized { span },
            (Self::MaybeInitialized { span }, _)
            | (_, Self::MaybeInitialized { span })
            | (Self::PartiallyInitialized { span }, _)
            | (_, Self::PartiallyInitialized { span }) => Self::MaybeInitialized { span },
            (Self::Initialized { .. }, state) | (state, Self::Initialized { .. }) => {
                Self::MaybeInitialized {
                    span: state.previous_span(),
                }
            }
        }
    }

    pub(super) fn is_initialized(self) -> bool {
        matches!(self, Self::Initialized { .. })
    }

    pub(super) fn previous_action(self) -> &'static str {
        match self {
            Self::Moved { .. } => "moved",
            Self::Dropped { .. } => "dropped",
            Self::Uninitialized { .. } => "uninitialized",
            Self::PartiallyInitialized { .. } => "partially initialized",
            Self::MaybeInitialized { .. } => "maybe uninitialized",
            Self::Initialized { .. } => "initialized",
        }
    }

    pub(super) fn previous_span(self) -> ByteSpan {
        match self {
            Self::Initialized { span }
            | Self::Moved { span }
            | Self::Dropped { span }
            | Self::Uninitialized { span }
            | Self::PartiallyInitialized { span }
            | Self::MaybeInitialized { span } => span,
        }
    }

    fn partially_initialized_from(state: Self) -> Self {
        Self::PartiallyInitialized {
            span: state.previous_span(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    fn span(offset: usize) -> ByteSpan {
        ByteSpan::new(SourceId::new(0), offset, offset + 1)
    }

    fn field(root: &str, fields: &[&str]) -> BorrowPlace {
        BorrowPlace {
            root: crate::resolve::LocalSymbolId::for_test(0),
            root_name: root.to_string(),
            fields: Some(fields.iter().map(|field| (*field).to_string()).collect()),
        }
    }

    fn root(name: &str) -> BorrowPlace {
        BorrowPlace::whole(crate::resolve::LocalSymbolId::for_test(0), name.to_string())
    }

    #[test]
    fn invalidating_field_keeps_sibling_live_and_marks_root_partial() {
        let mut states = PlaceStateForest::default();
        states.define_root(
            crate::resolve::LocalSymbolId::for_test(0),
            PlaceState::Initialized { span: span(0) },
        );
        states.invalidate(
            &field("value", &["left"]),
            PlaceState::Moved { span: span(1) },
        );

        assert_eq!(
            states.state(&field("value", &["left"])),
            Some(PlaceState::Moved { span: span(1) })
        );
        assert!(
            states
                .state(&field("value", &["right"]))
                .is_some_and(PlaceState::is_initialized)
        );
        assert_eq!(
            states.state(&root("value")),
            Some(PlaceState::PartiallyInitialized { span: span(1) })
        );
    }

    #[test]
    fn reinitializing_field_restores_whole_place() {
        let mut states = PlaceStateForest::default();
        states.define_root(
            crate::resolve::LocalSymbolId::for_test(0),
            PlaceState::Initialized { span: span(0) },
        );
        let left = field("value", &["left"]);
        states.invalidate(&left, PlaceState::Moved { span: span(1) });
        states.initialize(&left, span(2));

        assert!(
            states
                .state(&root("value"))
                .is_some_and(PlaceState::is_initialized)
        );
    }

    #[test]
    fn branch_join_marks_moved_field_maybe_initialized() {
        let mut incoming = PlaceStateForest::default();
        incoming.define_root(
            crate::resolve::LocalSymbolId::for_test(0),
            PlaceState::Initialized { span: span(0) },
        );
        let mut moved = incoming.clone();
        moved.invalidate(
            &field("value", &["payload"]),
            PlaceState::Moved { span: span(1) },
        );
        let untouched = incoming.clone();

        incoming.join_from(&[moved, untouched]);

        assert_eq!(
            incoming.state(&field("value", &["payload"])),
            Some(PlaceState::MaybeInitialized { span: span(1) })
        );
    }

    #[test]
    fn nested_invalidation_only_poison_ancestors() {
        let mut states = PlaceStateForest::default();
        states.define_root(
            crate::resolve::LocalSymbolId::for_test(0),
            PlaceState::Initialized { span: span(0) },
        );
        states.invalidate(
            &field("value", &["inner", "payload"]),
            PlaceState::Dropped { span: span(1) },
        );

        assert_eq!(
            states.state(&field("value", &["inner"])),
            Some(PlaceState::PartiallyInitialized { span: span(1) })
        );
        assert!(
            states
                .state(&field("value", &["other"]))
                .is_some_and(PlaceState::is_initialized)
        );
    }

    #[test]
    fn initializing_field_of_dead_root_produces_partial_state() {
        let mut states = PlaceStateForest::default();
        let root = root("value");
        states.define_root(
            crate::resolve::LocalSymbolId::for_test(0),
            PlaceState::Initialized { span: span(0) },
        );
        states.invalidate(&root, PlaceState::Moved { span: span(1) });
        states.initialize(&field("value", &["left"]), span(2));

        assert_eq!(
            states.state(&root),
            Some(PlaceState::PartiallyInitialized { span: span(2) })
        );
        assert!(
            states
                .state(&field("value", &["left"]))
                .is_some_and(PlaceState::is_initialized)
        );
        assert_eq!(
            states.state(&field("value", &["right"])),
            Some(PlaceState::Moved { span: span(1) })
        );
    }

    #[test]
    fn equal_display_names_do_not_merge_distinct_local_symbols() {
        let first = crate::resolve::LocalSymbolId::for_test(0);
        let second = crate::resolve::LocalSymbolId::for_test(1);
        let mut states = PlaceStateForest::default();
        states.define_root(first, PlaceState::Initialized { span: span(0) });
        states.define_root(second, PlaceState::Initialized { span: span(1) });
        states.invalidate(
            &BorrowPlace::whole(first, "value".to_string()),
            PlaceState::Moved { span: span(2) },
        );

        assert_eq!(
            states.state(&BorrowPlace::whole(first, "value".to_string())),
            Some(PlaceState::Moved { span: span(2) })
        );
        assert!(
            states
                .state(&BorrowPlace::whole(second, "value".to_string()))
                .is_some_and(PlaceState::is_initialized)
        );
    }
}

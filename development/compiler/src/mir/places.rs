//! Path-sensitive availability state for locals and projected aggregate places.
//!
//! A local can start wholly initialized and later lose one field. Keeping the
//! invalidated projection separate preserves access to sibling fields without
//! pretending that the whole aggregate is still movable. Explicitly initialized
//! projections cover construction and replacement before the whole root exists.

use super::dataflow::{LocalSet, ProjectionSet};
use super::{Body, LocalId, Place, ProjectionPathId};

pub(super) fn overlap(body: &Body, left: Place, right: Place) -> bool {
    if left.local != right.local {
        return false;
    }
    let (Some(left), Some(right)) = (left.projection, right.projection) else {
        return true;
    };
    if ancestors(body, left).any(|candidate| candidate == right)
        || ancestors(body, right).any(|candidate| candidate == left)
    {
        return true;
    }
    let Some(left_path) = body.projections.get(left.index()) else {
        return true;
    };
    let Some(right_path) = body.projections.get(right.index()) else {
        return true;
    };
    match (&left_path.element, &right_path.element) {
        (
            super::model::ProjectionElement::Field { offset: left },
            super::model::ProjectionElement::Field { offset: right },
        ) if left_path.parent == right_path.parent => left == right,
        (
            super::model::ProjectionElement::ErrorField(left),
            super::model::ProjectionElement::ErrorField(right),
        ) if left_path.parent == right_path.parent => left == right,
        (
            super::model::ProjectionElement::Dereference,
            super::model::ProjectionElement::Dereference,
        ) if left_path.parent == right_path.parent => true,
        // Index projections can alias unless a later representation proves
        // their indices distinct. Conservatism here preserves soundness.
        _ => true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlaceState {
    initialized_roots: LocalSet,
    initialized_projections: ProjectionSet,
    invalidated_projections: ProjectionSet,
}

impl PlaceState {
    pub(super) fn new(body: &Body) -> Self {
        Self {
            initialized_roots: LocalSet::new(body.locals.len()),
            initialized_projections: ProjectionSet::new(body.projections.len()),
            invalidated_projections: ProjectionSet::new(body.projections.len()),
        }
    }

    pub(super) fn initialize(&mut self, body: &Body, place: Place) {
        let Some(projection) = place.projection else {
            self.initialized_roots.insert(place.local);
            self.clear_projection_state_for_local(body, place.local);
            return;
        };
        self.initialized_projections.insert(projection);
        for descendant in descendants(body, projection) {
            self.initialized_projections.insert(descendant);
            self.invalidated_projections.remove(descendant);
        }
        self.invalidated_projections.remove(projection);
    }

    pub(super) fn move_out(&mut self, body: &Body, place: Place) {
        let Some(projection) = place.projection else {
            self.initialized_roots.remove(place.local);
            self.clear_projection_state_for_local(body, place.local);
            return;
        };
        if self.initialized_roots.contains(place.local) {
            self.invalidated_projections.insert(projection);
        }
        self.initialized_projections.remove(projection);
        for descendant in descendants(body, projection) {
            self.initialized_projections.remove(descendant);
            self.invalidated_projections.remove(descendant);
        }
    }

    /// Consolidates initialized child places into their completed aggregate.
    pub(super) fn finish_aggregate(&mut self, body: &Body, place: Place) {
        match place.projection {
            None => {
                self.clear_projection_state_for_local(body, place.local);
                self.initialized_roots.insert(place.local);
            }
            Some(projection) => {
                for descendant in descendants(body, projection) {
                    self.initialized_projections.remove(descendant);
                    self.invalidated_projections.remove(descendant);
                }
                self.initialized_projections.insert(projection);
                self.invalidated_projections.remove(projection);
            }
        }
    }

    pub(super) fn is_available(&self, body: &Body, place: Place) -> bool {
        let Some(projection) = place.projection else {
            return self.initialized_roots.contains(place.local)
                && body.projections.iter().all(|candidate| {
                    candidate.base != place.local
                        || !self.invalidated_projections.contains(candidate.id)
                });
        };
        if ancestors(body, projection)
            .any(|ancestor| self.invalidated_projections.contains(ancestor))
        {
            return false;
        }
        self.initialized_roots.contains(place.local)
            || ancestors(body, projection)
                .any(|ancestor| self.initialized_projections.contains(ancestor))
    }

    pub(super) fn any_available_within(&self, body: &Body, place: Place) -> bool {
        self.is_available(body, place)
            || body.projections.iter().any(|projection| {
                projection.base == place.local
                    && place.projection.is_none_or(|ancestor| {
                        ancestors(body, projection.id).any(|candidate| candidate == ancestor)
                    })
                    && self.is_available(body, Place::projected(place.local, projection.id))
            })
    }

    /// Intersects the places available on both incoming paths and reports a
    /// change. The result is normalized to explicit availability, so branch
    /// history cannot leak into later transfer functions.
    pub(super) fn intersect_with(&mut self, other: &Self, body: &Body) -> bool {
        self.merge_availability(other, body, |left, right| left && right)
    }

    /// Unions places available on either incoming path and reports a change.
    pub(super) fn union_with(&mut self, other: &Self, body: &Body) -> bool {
        self.merge_availability(other, body, |left, right| left || right)
    }

    fn merge_availability(
        &mut self,
        other: &Self,
        body: &Body,
        include: impl Fn(bool, bool) -> bool,
    ) -> bool {
        let mut merged = Self::new(body);
        for index in 0..body.locals.len() {
            let local = LocalId::from_index(index);
            let place = Place::local(local);
            if include(
                self.is_available(body, place),
                other.is_available(body, place),
            ) {
                merged.initialize(body, place);
            }
        }
        for projection in &body.projections {
            let place = Place::projected(projection.base, projection.id);
            if include(
                self.is_available(body, place),
                other.is_available(body, place),
            ) {
                merged.initialized_projections.insert(projection.id);
            }
        }
        let changed = *self != merged;
        *self = merged;
        changed
    }

    fn clear_projection_state_for_local(&mut self, body: &Body, local: LocalId) {
        for projection in body
            .projections
            .iter()
            .filter(|projection| projection.base == local)
        {
            self.initialized_projections.remove(projection.id);
            self.invalidated_projections.remove(projection.id);
        }
    }
}

fn ancestors(body: &Body, start: ProjectionPathId) -> impl Iterator<Item = ProjectionPathId> + '_ {
    std::iter::successors(Some(start), |projection| {
        body.projections.get(projection.index())?.parent
    })
}

fn descendants(
    body: &Body,
    ancestor: ProjectionPathId,
) -> impl Iterator<Item = ProjectionPathId> + '_ {
    body.projections
        .iter()
        .filter(move |candidate| {
            candidate.id != ancestor
                && ancestors(body, candidate.id).any(|parent| parent == ancestor)
        })
        .map(|projection| projection.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{
        BasicBlock, BasicBlockId, Local, LocalOrigin, LocalStorage, OwnershipKind,
        ProjectionElement, ProjectionPath, ReturnMode, Scope, ScopeId, Terminator,
        ValueRepresentation,
    };
    use crate::semantic::{BodyId, TyId};
    use crate::source::{ByteSpan, SourceId};

    fn aggregate_body() -> Body {
        let span = ByteSpan::new(SourceId::new(0), 0, 1);
        let scope = ScopeId::from_index(0);
        let aggregate = LocalId::from_index(1);
        Body {
            source_body: BodyId::from_index(0),
            source_span: span,
            return_local: LocalId::from_index(0),
            return_mode: ReturnMode::Plain,
            outcome_contract: None,
            root_scope: scope,
            scopes: vec![Scope::root(span)],
            locals: vec![
                Local::scalar(
                    TyId::from_index(0),
                    crate::mir::ScalarType::I32,
                    LocalStorage::Return,
                    LocalOrigin::Return,
                    scope,
                ),
                Local::aggregate(
                    TyId::from_index(1),
                    OwnershipKind::Move,
                    LocalStorage::Local,
                    LocalOrigin::Desugared(span),
                    scope,
                ),
            ],
            entry: BasicBlockId::from_index(0),
            blocks: vec![BasicBlock {
                scope,
                statements: Vec::new(),
                terminator: Terminator::Trap,
            }],
            loop_regions: Vec::new(),
            allocation_regions: Vec::new(),
            allocation_overrides: Vec::new(),
            loans: Vec::new(),
            projections: [0, 8]
                .into_iter()
                .enumerate()
                .map(|(index, offset)| ProjectionPath {
                    id: ProjectionPathId::from_index(index),
                    base: aggregate,
                    parent: None,
                    element: ProjectionElement::Field { offset },
                    ty: TyId::from_index(0),
                    representation: ValueRepresentation::Scalar(crate::mir::ScalarType::I32),
                    ownership: OwnershipKind::Copy,
                    drop_plan: None,
                })
                .collect(),
            drop_plans: Vec::new(),
        }
    }

    #[test]
    fn moving_one_field_preserves_its_sibling_but_not_the_whole_root() {
        let body = aggregate_body();
        let aggregate = LocalId::from_index(1);
        let first = Place::projected(aggregate, ProjectionPathId::from_index(0));
        let second = Place::projected(aggregate, ProjectionPathId::from_index(1));
        let mut state = PlaceState::new(&body);
        state.initialize(&body, Place::local(aggregate));

        state.move_out(&body, first);

        assert!(!state.is_available(&body, Place::local(aggregate)));
        assert!(!state.is_available(&body, first));
        assert!(state.is_available(&body, second));
        state.initialize(&body, first);
        assert!(state.is_available(&body, Place::local(aggregate)));
    }

    #[test]
    fn branch_intersection_retains_only_places_available_on_both_paths() {
        let body = aggregate_body();
        let aggregate = LocalId::from_index(1);
        let first = Place::projected(aggregate, ProjectionPathId::from_index(0));
        let second = Place::projected(aggregate, ProjectionPathId::from_index(1));
        let mut left = PlaceState::new(&body);
        left.initialize(&body, Place::local(aggregate));
        let mut right = left.clone();
        left.move_out(&body, first);
        right.move_out(&body, second);

        assert!(left.intersect_with(&right, &body));
        assert!(!left.is_available(&body, Place::local(aggregate)));
        assert!(!left.is_available(&body, first));
        assert!(!left.is_available(&body, second));
    }

    #[test]
    fn field_siblings_are_disjoint_but_each_overlaps_its_root() {
        let body = aggregate_body();
        let aggregate = LocalId::from_index(1);
        let root = Place::local(aggregate);
        let first = Place::projected(aggregate, ProjectionPathId::from_index(0));
        let second = Place::projected(aggregate, ProjectionPathId::from_index(1));

        assert!(overlap(&body, root, first));
        assert!(overlap(&body, first, root));
        assert!(overlap(&body, first, first));
        assert!(!overlap(&body, first, second));
    }
}

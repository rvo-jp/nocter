use nocter_model::{BodyNodeId, PlaceId};

use super::{Analyzer, ReturnEvent};
use crate::provenance::state::ProvenanceState;
use crate::{
    BodyCheckError, BodyCheckInternalError, PlaceProjection, PlaceRoot, ProvenanceProjection,
    ProvenanceSource, ValueProvenance,
};

impl Analyzer<'_, '_> {
    pub(super) fn evaluate_place_indices(
        &mut self,
        place: PlaceId,
        state: &mut ProvenanceState,
    ) -> Result<(), BodyCheckError> {
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let indices = place.evaluation_nodes().collect::<Vec<_>>();
        for index in indices {
            let _ = self.evaluate(index, state)?;
        }
        Ok(())
    }

    pub(super) fn read_place(
        &self,
        place: PlaceId,
        state: &ProvenanceState,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let mut value = self.place_root_value(place.root(), state);
        for projection in place.projections() {
            if let Some(projection) = value_projection(projection) {
                value = value.projected(projection);
            }
        }
        Ok(value)
    }

    pub(super) fn place_storage(
        &self,
        place: PlaceId,
        state: &ProvenanceState,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let mut carried = self.place_root_value(place.root(), state);
        let mut storage = match place.root() {
            PlaceRoot::Local(local) => ValueProvenance::from_source(ProvenanceSource::Local(local)),
            PlaceRoot::Parameter(parameter) => {
                ValueProvenance::from_source(ProvenanceSource::OwnedParameter(parameter))
            }
            PlaceRoot::Capture(_) => self.closure.map_or_else(
                || ValueProvenance::from_source(ProvenanceSource::Unknown),
                |(closure, _)| {
                    ValueProvenance::from_source(ProvenanceSource::ClosureEnvironment(closure))
                },
            ),
            PlaceRoot::Value(value) => self
                .node_values
                .get(&value)
                .cloned()
                .unwrap_or_else(|| ValueProvenance::from_source(ProvenanceSource::Unknown)),
        };
        for projection in place.projections() {
            match projection {
                PlaceProjection::BorrowDeref { .. } => storage = carried.clone(),
                projection => {
                    if let Some(projection) = value_projection(projection) {
                        carried = carried.projected(projection);
                    }
                }
            }
        }
        Ok(storage)
    }

    fn place_root_value(&self, root: PlaceRoot, state: &ProvenanceState) -> ValueProvenance {
        match root {
            PlaceRoot::Value(value) => self
                .node_values
                .get(&value)
                .cloned()
                .unwrap_or_else(|| ValueProvenance::from_source(ProvenanceSource::Unknown)),
            PlaceRoot::Parameter(_) | PlaceRoot::Local(_) | PlaceRoot::Capture(_) => {
                state.value(root)
            }
        }
    }

    pub(super) fn write_place(
        &self,
        place: PlaceId,
        value: ValueProvenance,
        state: &mut ProvenanceState,
    ) -> Result<(), BodyCheckInternalError> {
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let path = place
            .projections()
            .iter()
            .map(value_projection)
            .collect::<Option<Vec<_>>>();
        let Some(path) = path else {
            return Ok(());
        };
        let mut root = state.value(place.root());
        root.replace_projection(&path, value);
        state.set_value(place.root(), root);
        Ok(())
    }

    pub(super) fn remove_place(
        &self,
        place: PlaceId,
        state: &mut ProvenanceState,
    ) -> Result<(), BodyCheckInternalError> {
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let path = place
            .projections()
            .iter()
            .map(value_projection)
            .collect::<Option<Vec<_>>>();
        let Some(path) = path else {
            return Ok(());
        };
        if path.is_empty() {
            state.remove(place.root());
        } else {
            let mut root = state.value(place.root());
            root.remove_projection(&path);
            state.set_value(place.root(), root);
        }
        Ok(())
    }

    pub(super) fn remove_scope_locals(
        &self,
        scope: nocter_model::BodyScopeId,
        state: &mut ProvenanceState,
    ) {
        for (local, declaration) in self.body.locals().iter() {
            if declaration.declaration().scope() == scope {
                state.remove(PlaceRoot::Local(local));
            }
        }
    }

    pub(super) fn record_return(&mut self, node: BodyNodeId, value: ValueProvenance) {
        let ty = self.result_type;
        self.returned.union_with(&value);
        self.return_events.push(ReturnEvent { node, ty, value });
    }
}

fn value_projection(projection: &PlaceProjection) -> Option<ProvenanceProjection> {
    match projection {
        PlaceProjection::Field { field, .. } => Some(ProvenanceProjection::Field(*field)),
        PlaceProjection::TupleElement { index, .. } => {
            Some(ProvenanceProjection::TupleElement(*index))
        }
        PlaceProjection::BuiltinIndex { .. }
        | PlaceProjection::CoercedBuiltinIndex { .. }
        | PlaceProjection::SelectedIndex { .. } => Some(ProvenanceProjection::Element),
        PlaceProjection::BorrowDeref { .. } => None,
    }
}

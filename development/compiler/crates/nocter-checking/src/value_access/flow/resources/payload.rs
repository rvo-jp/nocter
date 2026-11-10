//! Sparse availability and cleanup frontiers. Fully initialized subtrees remain shared schemas.
use super::{Disposal, FieldPath, INVALID, Payload, Place, Program, Result, Step, payload};
use nocter_declarations::value_access::{ArraySchema, PayloadSchema};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct OwnedValue {
    pub payload: Payload,
    // Prefix-free unavailable selections. Do not merge missing siblings into their parent:
    // the parent container still exists and permits restoring an individual child.
    pub missing: BTreeSet<FieldPath>,
}
enum Pending {
    Value(Place, Payload),
    Emit(Step),
}
impl OwnedValue {
    pub fn new(payload: Payload) -> Self {
        Self {
            payload,
            missing: BTreeSet::new(),
        }
    }
    pub fn complete(&self, path: &FieldPath) -> bool {
        !self.missing.iter().any(|hole| hole.overlaps(path))
    }
    pub fn selected(&self, program: &Program, path: &FieldPath) -> Result<Payload> {
        payload(
            program,
            program
                .project_data(self.payload.data(), path)
                .ok_or(INVALID)?,
        )
    }
    pub fn container(&self, program: &Program, path: &FieldPath) -> Result<Payload> {
        let selected = self.selected(program, path)?;
        if self
            .missing
            .iter()
            .any(|hole| hole != path && hole.is_prefix_of(path))
        {
            return Err(INVALID);
        }
        Ok(selected)
    }
    pub fn remove(&mut self, path: &FieldPath) {
        self.missing.retain(|hole| !path.is_prefix_of(hole));
        self.missing.insert(path.clone());
    }
    /// Maximal initialized regions in destruction order, without enumerating intact descendants.
    pub fn cleanup(&self, program: &Program, place: &Place) -> Result<Vec<Step>> {
        let selected = self.container(program, place.fields())?;
        let mut pending = vec![Pending::Value(place.clone(), selected)];
        let mut cleanup = Vec::new();
        while let Some(next) = pending.pop() {
            let (place, selected) = match next {
                Pending::Value(place, selected) => (place, selected),
                Pending::Emit(step) => {
                    cleanup.push(step);
                    continue;
                }
            };
            if self.missing.contains(place.fields()) {
                continue;
            }
            if self.complete(place.fields()) {
                cleanup.push(Step::Dispose {
                    source: place,
                    payload: selected,
                });
                continue;
            }
            if selected.disposal() != Disposal::Aggregate {
                return Err(INVALID);
            }
            // The container ends after surviving regions. Stack insertion follows component
            // order so cleanup visits them in reverse.
            pending.push(Pending::Emit(Step::ReleaseContainer {
                source: place.clone(),
                payload: selected,
            }));
            match program.data_schema(selected.data()).ok_or(INVALID)? {
                PayloadSchema::Aggregate(aggregate) => {
                    for field in aggregate.components() {
                        let child = place.project(field.field);
                        pending.push(Pending::Value(child, payload(program, field.data)?));
                    }
                }
                PayloadSchema::Array(array) => {
                    self.array_frontier(program, &place, selected, array, &mut pending)?;
                }
                PayloadSchema::Builtin(_) | PayloadSchema::Function(_) => return Err(INVALID),
            }
        }
        Ok(cleanup)
    }
    fn array_frontier(
        &self,
        program: &Program,
        place: &Place,
        selected: Payload,
        array: ArraySchema,
        pending: &mut Vec<Pending>,
    ) -> Result<()> {
        let depth = place.fields().fields().len();
        let indices = self
            .missing
            .iter()
            .filter(|hole| place.fields().is_prefix_of(hole))
            .map(|hole| {
                let Some(nocter_model::ValueField::Array(index)) = hole.fields().get(depth) else {
                    return Err(INVALID);
                };
                (*index < array.length()).then_some(*index).ok_or(INVALID)
            })
            .collect::<Result<BTreeSet<_>>>()?;
        let element = payload(program, array.element())?;
        let mut start = 0;
        // The final length boundary closes the last intact span; no array elements are enumerated.
        for index in indices.into_iter().chain(std::iter::once(array.length())) {
            if start < index {
                pending.push(Pending::Emit(Step::DisposeArrayElements {
                    source: place.clone(),
                    payload: selected,
                    start,
                    end: index,
                }));
            }
            if index < array.length() {
                let child = place.project(nocter_model::ValueField::Array(index));
                pending.push(Pending::Value(child, element));
                start = index.checked_add(1).ok_or(INVALID)?;
            }
        }
        Ok(())
    }
}

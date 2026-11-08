//! Sparse availability and cleanup frontiers. Fully initialized subtrees remain shared schemas.
use super::{Disposal, FieldPath, INVALID, Payload, Place, Program, Result, Step, payload};
use nocter_declarations::value_access::PayloadSchema;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct OwnedValue {
    pub payload: Payload,
    // Prefix-free unavailable selections. Do not merge missing siblings into their parent:
    // the parent container still exists and permits restoring an individual child.
    pub missing: BTreeSet<FieldPath>,
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
        enum Pending {
            Value(Place, Payload),
            Container(Place, Payload),
        }
        let selected = self.container(program, place.fields())?;
        let mut pending = vec![Pending::Value(place.clone(), selected)];
        let mut cleanup = Vec::new();
        while let Some(next) = pending.pop() {
            let (place, selected) = match next {
                Pending::Value(place, selected) => (place, selected),
                Pending::Container(source, payload) => {
                    cleanup.push(Step::ReleaseContainer { source, payload });
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
            if selected.disposal() != Disposal::Record {
                return Err(INVALID);
            }
            let PayloadSchema::Record(record) =
                program.data_schema(selected.data()).ok_or(INVALID)?
            else {
                return Err(INVALID);
            };
            // The container ends after surviving fields. Stack insertion follows declaration
            // order; field visitation is reverse declaration order.
            pending.push(Pending::Container(place.clone(), selected));
            for &field in record.fields() {
                let child = place.project(field);
                let data = program.fields().get(field).ok_or(INVALID)?.data();
                pending.push(Pending::Value(child, payload(program, data)?));
            }
        }
        Ok(cleanup)
    }
}

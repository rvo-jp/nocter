//! Sparse availability and cleanup frontiers. Fully initialized subtrees remain shared schemas.
use super::{
    Disposal, INVALID, Index, IndexBacking, Missing, Path, Payload, Place, Program, Result, Step,
    payload,
};
use nocter_declarations::value_access::{ArraySchema, PayloadSchema};
use nocter_model::ValueField;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct OwnedValue {
    pub payload: Payload,
    pub retained_reads: Option<Arc<[super::ActivationRead]>>,
    // Prefix-free unavailable selections. Do not merge missing siblings into their parent:
    // the parent container still exists and permits restoring an individual child.
    pub missing: BTreeSet<Missing>,
}
enum Pending {
    Value(Place, Payload),
    Emit(Step),
}
impl OwnedValue {
    pub fn new(payload: Payload) -> Self {
        Self {
            payload,
            retained_reads: None,
            missing: BTreeSet::new(),
        }
    }
    pub fn complete(&self, path: &Path) -> bool {
        !self.missing.iter().any(|hole| hole.overlaps(path))
    }
    pub fn selected(&self, program: &Program, path: &Path) -> Result<Payload> {
        if path.is_root() && matches!(self.payload.disposal(), Disposal::UnstartedFuture(_)) {
            return Ok(self.payload);
        }
        payload(
            program,
            path.data(program, self.payload.data()).ok_or(INVALID)?,
        )
    }
    pub fn container(&self, program: &Program, path: &Path) -> Result<Payload> {
        let selected = self.selected(program, path)?;
        if self.missing.iter().any(|hole| hole.blocks_container(path)) {
            return Err(INVALID);
        }
        Ok(selected)
    }
    pub fn remove(&mut self, path: &Path) {
        self.missing.retain(|hole| !hole.under(path));
        self.missing.insert(Missing::Value(path.clone()));
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
            if self
                .missing
                .contains(&Missing::Value(place.fields().clone()))
            {
                continue;
            }
            if self.complete(place.fields()) {
                cleanup.push(Step::Dispose {
                    source: place,
                    payload: selected,
                });
                continue;
            }
            // A possible intersection is not proof of a contained hole. In particular, a
            // concrete child may itself be the element already moved through a retained index.
            if self
                .missing
                .iter()
                .any(|hole| hole.overlaps(place.fields()) && !hole.under(place.fields()))
            {
                return Err(INVALID);
            }
            if selected.disposal() != Disposal::Aggregate {
                return Err(INVALID);
            }
            let schema = program.data_schema(selected.data()).ok_or(INVALID)?;
            if matches!(schema, PayloadSchema::Array(_)) {
                let depth = place.fields().fields().len();
                let holes: Vec<_> = self
                    .missing
                    .iter()
                    .filter(|hole| hole.under(place.fields()))
                    .collect();
                if holes.len() > 1
                    && holes.iter().any(|hole| {
                        matches!(
                            hole.anchor().fields().get(depth),
                            Some(nocter_model::ValueProjection::Array(Index::Retained(_)))
                        )
                    })
                {
                    let missing = holes.into_iter().cloned().collect();
                    cleanup.push(Step::DisposeArrayRemainder {
                        source: place,
                        payload: selected,
                        missing,
                    });
                    continue;
                }
            }
            // The container ends after surviving regions. Stack insertion follows component
            // order so cleanup visits them in reverse.
            pending.push(Pending::Emit(Step::ReleaseContainer {
                source: place.clone(),
                payload: selected,
            }));
            match schema {
                PayloadSchema::Aggregate(aggregate) => {
                    for field in aggregate.components() {
                        let child = place.project(field.field);
                        pending.push(Pending::Value(child, payload(program, field.data)?));
                    }
                }
                PayloadSchema::Array(array) => {
                    self.array_frontier(program, &place, selected, array, &mut pending)?;
                }
                PayloadSchema::Optional(data) => {
                    let field = ValueField::Variant(true);
                    pending.push(Pending::Value(
                        place.project(field),
                        payload(program, data)?,
                    ));
                }
                PayloadSchema::Fallible(data) => {
                    let primary = self.active_variant(place.fields())?;
                    let data = if primary {
                        data
                    } else {
                        program
                            .data_type(nocter_model::BuiltinType::Error)
                            .ok_or(INVALID)?
                    };
                    pending.push(Pending::Value(
                        place.project(ValueField::Variant(primary)),
                        payload(program, data)?,
                    ));
                }
                PayloadSchema::Builtin(_)
                | PayloadSchema::Function(_)
                | PayloadSchema::Future(_) => return Err(INVALID),
            }
        }
        Ok(cleanup)
    }
    /// A missing descendant records an actual guarded acquisition from this stored carrier.
    /// Opposite variants overlap physically, but cannot both own a payload on that path.
    fn active_variant(&self, parent: &Path) -> Result<bool> {
        let depth = parent.fields().len();
        let mut active = None;
        for hole in self.missing.iter().filter(|hole| hole.under(parent)) {
            let Some(nocter_model::ValueProjection::Variant(primary)) =
                hole.anchor().fields().get(depth)
            else {
                return Err(INVALID);
            };
            if active.is_some_and(|active| active != *primary) {
                return Err(INVALID);
            }
            active = Some(*primary);
        }
        active.ok_or(INVALID)
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
        let holes: Vec<_> = self
            .missing
            .iter()
            .filter(|hole| hole.under(place.fields()))
            .collect();
        if let Some(frontier) = holes.iter().find_map(|hole| match hole {
            Missing::ArrayPrefix { array, frontier } if array == place.fields() => Some(*frontier),
            _ => None,
        }) {
            if holes.len() != 1 || frontier.bound() != array.length() {
                return Err(INVALID);
            }
            pending.push(Pending::Emit(Step::DisposeArrayElements {
                source: place.clone(),
                payload: selected,
                range: nocter_model::ArrayRange::After(IndexBacking::Frontier(frontier)),
            }));
            return Ok(());
        }
        if let Some(index) =
            holes
                .iter()
                .filter_map(|hole| hole.value())
                .find_map(|hole| match hole.fields().get(depth) {
                    Some(nocter_model::ValueProjection::Array(Index::Retained(index))) => {
                        Some(*index)
                    }
                    _ => None,
                })
        {
            if holes.len() != 1
                || holes[0]
                    .value()
                    .is_none_or(|path| path.fields().len() < depth + 1)
                || index.bound() != array.length()
            {
                return Err(INVALID);
            }
            // Stack order is opposite destruction order. Descend into the selected element
            // between the intact spans; a whole-element hole skips that child entirely.
            pending.push(Pending::Emit(Step::DisposeArrayElements {
                source: place.clone(),
                payload: selected,
                range: nocter_model::ArrayRange::Before(IndexBacking::Selection(index)),
            }));
            pending.push(Pending::Value(
                place.project_retained(index),
                payload(program, array.element())?,
            ));
            pending.push(Pending::Emit(Step::DisposeArrayElements {
                source: place.clone(),
                payload: selected,
                range: nocter_model::ArrayRange::After(IndexBacking::Selection(index)),
            }));
            return Ok(());
        }
        let indices = self
            .missing
            .iter()
            .filter(|hole| hole.under(place.fields()))
            .map(Missing::anchor)
            .map(|hole| {
                let Some(nocter_model::ValueProjection::Array(Index::Constant(index))) =
                    hole.fields().get(depth)
                else {
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
                    range: nocter_model::ArrayRange::Constant { start, end: index },
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

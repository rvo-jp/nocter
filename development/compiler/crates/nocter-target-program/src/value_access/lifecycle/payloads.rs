//! One transition's value transport, without replaying ownership or changing its selected steps.
use super::{Failure, INVALID, Instance, InstanceProgram, Location, Payload, ResourceStep, Step};
#[cfg(test)]
mod tests;
use nocter_checking::value_access::{
    ValueSourceBoundary, ValueSourceSelection, ValueSourceSlot,
    flow::resources::{Holder, PayloadSite, Place},
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{DataInstanceKey, InstanceSchema, PayloadSchema};
use nocter_model::{AccessInstanceId, TypeId, ValueField, ValuePath, ValueProjection};
use std::collections::BTreeMap;

enum Binding {
    Value(DataInstanceKey),
    AfterAction,
}
/// Only data references for materializations in this one ordered transition, never owner state.
pub(super) struct Projection<'a, 'work> {
    instances: &'a InstanceProgram,
    instance: &'a Instance,
    id: AccessInstanceId,
    at: Location,
    work: &'work mut WorkBudget,
    values: BTreeMap<Holder, Binding>,
}
impl<'a, 'work> Projection<'a, 'work> {
    pub(super) fn new(
        instances: &'a InstanceProgram,
        instance: &'a Instance,
        id: AccessInstanceId,
        at: Location,
        work: &'work mut WorkBudget,
    ) -> Self {
        Self {
            instances,
            instance,
            id,
            at,
            work,
            values: BTreeMap::new(),
        }
    }
    pub(super) fn steps(&mut self, steps: &[ResourceStep]) -> Result<Vec<Step>, Failure> {
        steps
            .iter()
            .map(|step| self.step(step).map(Step::Resource))
            .collect()
    }
    fn step(
        &mut self,
        step: &ResourceStep,
    ) -> Result<nocter_checking::value_access::flow::resources::Step<Payload>, Failure> {
        let mut source = None;
        let mut created = None;
        let result = step.try_map_payloads_at(|site, contract| {
            let key = match site {
                PayloadSite::Source(place) => self.source(place, contract.data())?,
                PayloadSite::Destination(holder) => {
                    let key = self.destination(step, holder, contract.data(), source.as_ref())?;
                    created = Some((holder, key.clone()));
                    key
                }
            };
            if matches!(site, PayloadSite::Source(_)) {
                source = Some(key.clone());
            }
            contract
                .instantiate(self.instances.unit().program(), &key)
                .map_err(|_| INVALID)
        })?;
        if let Some((holder, key)) = created {
            self.values.insert(holder, Binding::Value(key));
        }
        match step {
            ResourceStep::Move { destination, .. }
            | ResourceStep::ReplaceReferent { destination, .. } => {
                let key = source.ok_or(INVALID)?;
                self.values.insert(
                    destination.holder(),
                    if destination.fields().is_root() {
                        Binding::Value(key)
                    } else {
                        Binding::AfterAction
                    },
                );
            }
            ResourceStep::MoveArrayElement { destination, .. } => {
                self.values
                    .insert(*destination, Binding::Value(source.ok_or(INVALID)?));
            }
            _ => {}
        }
        Ok(result)
    }
    fn closed(&self, target: TypeId) -> Option<DataInstanceKey> {
        if let Some(key) = self
            .instance
            .data()
            .get(&target)
            .and_then(super::super::DataEvidence::payload)
            .map(|payload| payload.instance().clone())
            && key.data() == target
            && key.is_context_free()
        {
            return Some(key);
        }
        self.instances
            .unit()
            .program()
            .instantiate_closed_data(target)
            .ok()
    }
    fn source(&mut self, place: &Place, target: TypeId) -> Result<DataInstanceKey, Failure> {
        let program = self.instances.unit().program();
        // An independent schema selects metadata only. The matched transition owns availability.
        if let Some(key) = self.closed(target) {
            return Ok(key);
        }
        match self.values.get(&place.holder()) {
            Some(Binding::Value(key)) => {
                let key = key
                    .project_path(program, &metadata_path(place, 0_u64))
                    .map_err(|_| INVALID)?;
                self.receiving(key, target)
            }
            Some(Binding::AfterAction) => self.stored(place, target, true),
            None => self.stored(place, target, false),
        }
    }
    fn destination(
        &mut self,
        step: &ResourceStep,
        holder: Holder,
        target: TypeId,
        source: Option<&DataInstanceKey>,
    ) -> Result<DataInstanceKey, Failure> {
        let program = self.instances.unit().program();
        if let Some(key) = self.closed(target) {
            return Ok(key);
        }
        match step {
            ResourceStep::Capture { .. } => {
                let Some(PayloadSchema::Function(selected)) = program.data_schema(target) else {
                    return Err(INVALID);
                };
                let actual = program
                    .bind_callable_data(selected, source.ok_or(INVALID)?)
                    .map_err(|_| INVALID)?;
                program
                    .instantiate_function(target, &actual)
                    .map_err(|_| INVALID)
            }
            ResourceStep::AdaptFunction { .. } => {
                self.receiving(source.ok_or(INVALID)?.clone(), target)
            }
            ResourceStep::Assemble { fields, .. } => self.assemble(target, fields),
            ResourceStep::Construct { .. } | ResourceStep::Receive { .. } => {
                self.stored(&holder.place(), target, true)
            }
            _ => Err(INVALID),
        }
    }
    fn assemble(
        &mut self,
        target: TypeId,
        fields: &[(ValueField, Place)],
    ) -> Result<DataInstanceKey, Failure> {
        let program = self.instances.unit().program();
        let array = matches!(program.data_schema(target), Some(PayloadSchema::Array(_)));
        let mut values = Vec::new();
        let mut element = None;
        for (field, source) in fields {
            let data = program.component(target, *field).ok_or(INVALID)?.data;
            let key = self.source(source, data)?;
            if array {
                if element.as_ref().is_some_and(|previous| previous != &key) {
                    return Err(INVALID);
                }
                element = Some(key);
            } else {
                values.push((*field, key));
            }
        }
        if let Some(element) = element {
            values.push((ValueField::Array(0), element));
        }
        program
            .instantiate_components(target, values)
            .map_err(|_| INVALID)
    }
    fn receiving(
        &self,
        source: DataInstanceKey,
        target: TypeId,
    ) -> Result<DataInstanceKey, Failure> {
        if source.data() == target {
            return Ok(source);
        }
        let program = self.instances.unit().program();
        match source.schema(program).map_err(|_| INVALID)? {
            InstanceSchema::Function(actual) => program
                .instantiate_function(target, &actual)
                .map_err(|_| INVALID),
            _ => Err(INVALID),
        }
    }
    fn stored(
        &mut self,
        place: &Place,
        target: TypeId,
        after: bool,
    ) -> Result<DataInstanceKey, Failure> {
        let slot = match place.holder() {
            Holder::Local(local) => ValueSourceSlot::Local(local),
            Holder::Expression(expression) => ValueSourceSlot::Expression(expression),
            Holder::Prepared { call, index } | Holder::Temporary { call, index } => {
                ValueSourceSlot::Argument { call, index }
            }
            Holder::Return => ValueSourceSlot::Return,
            Holder::IndexSnapshot(_) | Holder::ArrayFrontier(_) => return Err(INVALID),
        };
        let selected = ValueSourceSelection {
            boundary: ValueSourceBoundary {
                point: self.at.point,
                action: self
                    .at
                    .action
                    .checked_add(usize::from(after))
                    .ok_or(INVALID)?,
            },
            slot,
            path: metadata_path(place, ()),
        };
        self.instances
            .unit()
            .instantiate_stored_data(self.instance.key(), &selected, target, self.work)
            .map_err(|failure| Failure::Payload {
                instance: self.id,
                at: self.at,
                failure,
            })
    }
}
fn metadata_path<I: Copy>(place: &Place, array: I) -> ValuePath<I> {
    ValuePath::new(place.fields().fields().iter().map(|&field| match field {
        ValueProjection::Array(_) => ValueProjection::Array(array),
        ValueProjection::Record(field) => ValueProjection::Record(field),
        ValueProjection::Tuple(index) => ValueProjection::Tuple(index),
        ValueProjection::Variant(primary) => ValueProjection::Variant(primary),
    }))
}

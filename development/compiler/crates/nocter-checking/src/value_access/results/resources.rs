//! Ordered resource data projection shared by checked consumers; never an ownership analysis.
use super::{
    Evidence, InstantiationFailure, ValueSourceBoundary, ValueSourceSelection, ValueSourceSlot,
};
use crate::value_access::flow::{
    self, Location, Operation,
    resources::{Holder, InstantiatedPayload as Payload, PayloadSite, Place, Step as ResourceStep},
};
use crate::value_access::{ArgumentAction, ExpressionKind};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{
    CodeInstanceKey, DataInstanceKey, InstanceSchema, PayloadSchema, Program,
};
use nocter_model::{TypeId, ValueField, ValuePath, ValueProjection};
use std::collections::BTreeMap;
#[cfg(test)]
mod tests;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceDataFailure {
    Integrity,
    Instantiation(InstantiationFailure),
}
const INVALID: ResourceDataFailure = ResourceDataFailure::Integrity;
type Failure = ResourceDataFailure;
#[derive(Clone, Debug, Eq, PartialEq)]
enum Binding {
    Value(DataInstanceKey),
    AfterAction,
}
/// Only data references for materializations in this one ordered transition, never owner state.
pub struct ResourceDataProjection<'a, 'work> {
    program: &'a Program,
    evidence: &'a Evidence,
    body: &'a flow::Query,
    frame: &'a CodeInstanceKey,
    at: Location,
    work: &'work mut WorkBudget,
    values: BTreeMap<Holder, Binding>,
}

mod roots;
pub use roots::{PartialRootContracts, ResourceDataStep};
impl<'a, 'work> ResourceDataProjection<'a, 'work> {
    /// Resolve the matched scoped destination's original containing contracts under this request's
    /// metadata budget. The pair supplies no ownership, physical backing or geometry proof.
    /// # Errors
    /// Rejects unmatched destinations, unavailable contexts and exhausted metadata work.
    pub fn replacement_carriers(
        &mut self,
        destination: &Place,
    ) -> Result<
        (
            crate::value_access::Referent,
            super::ReferentCarrierContracts,
        ),
        Failure,
    > {
        let target = self.replacement_target(destination)?;
        let contracts = super::instantiate_referent_carrier_data(
            self.program,
            self.evidence,
            self.body,
            self.frame,
            self.at,
            target,
            self.work,
        )
        .map_err(Failure::Instantiation)?;
        if contracts.after().is_none() {
            return Err(INVALID);
        }
        Ok((target, contracts))
    }
    /// Match a scoped replacement to its original alias-relative destination. This grants no
    /// ownership, initialized carrier or geometry proof; it selects the frozen source root pair.
    /// # Errors
    /// Rejects a different operation or destination, including a source action with no write.
    pub fn replacement_target(
        &self,
        destination: &Place,
    ) -> Result<crate::value_access::Referent, Failure> {
        let target = self.write_target()?;
        let expected = match target {
            crate::value_access::Referent::Evaluated(id) => Holder::Expression(id).place(),
            crate::value_access::Referent::Place(id) => {
                let place = self.body.typed().body().places().get(id).ok_or(INVALID)?;
                if self
                    .body
                    .typed()
                    .body()
                    .locals()
                    .get(place.local)
                    .ok_or(INVALID)?
                    .kind
                    .access_mode()
                    != nocter_model::ValueAccessMode::Update
                {
                    return Err(INVALID);
                }
                Holder::Local(place.local)
                    .place()
                    .project_path(&place.fields)
            }
        };
        if expected != *destination {
            return Err(INVALID);
        }
        Ok(target)
    }
    fn write_target(&self) -> Result<crate::value_access::Referent, Failure> {
        let operation = &self
            .body
            .graph()
            .points()
            .get(self.at.point)
            .and_then(|block| block.actions.get(self.at.action))
            .ok_or(INVALID)?
            .operation;
        match *operation {
            Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. } => {
                Ok(target)
            }
            _ => Err(INVALID),
        }
    }
    pub(in crate::value_access) fn new(
        program: &'a Program,
        evidence: &'a Evidence,
        body: Option<&'a flow::Query>,
        frame: &'a CodeInstanceKey,
        at: Location,
        work: &'work mut WorkBudget,
    ) -> Result<Self, Failure> {
        let body = body
            .filter(|body| body.typed().code() == frame.code())
            .ok_or(INVALID)?;
        body.graph()
            .points()
            .get(at.point)
            .and_then(|point| point.actions.get(at.action))
            .ok_or(INVALID)?;
        Ok(Self {
            program,
            evidence,
            body,
            frame,
            at,
            work,
            values: BTreeMap::new(),
        })
    }
    /// Project only payload metadata, preserving the supplied operation and place identities.
    /// # Errors
    /// Invalid source data or quota exhaustion publishes no partly projected step list.
    pub fn steps(&mut self, steps: &[ResourceStep]) -> Result<Vec<ResourceStep<Payload>>, Failure> {
        Ok(self
            .project_steps(steps, false, false)?
            .into_iter()
            .map(|step| step.operation)
            .collect())
    }
    /// Project the checked transition's normal post-operation steps. Only this explicit phase
    /// boundary invalidates entry snapshots of edited call temporaries; step names do not.
    /// # Errors
    /// Failure restores all prior projection bindings, including the preparation snapshots.
    pub fn after_steps(
        &mut self,
        steps: &[ResourceStep],
    ) -> Result<Vec<ResourceStep<Payload>>, Failure> {
        Ok(self
            .project_steps(steps, true, false)?
            .into_iter()
            .map(|step| step.operation)
            .collect())
    }
    /// Retain dependent partial-source roots in the same atomic, ordered projection as payloads.
    /// # Errors
    /// Rejects unavailable original roots or exhausted metadata work without publishing a prefix.
    pub fn steps_with_roots(
        &mut self,
        steps: &[ResourceStep],
    ) -> Result<Vec<ResourceDataStep>, Failure> {
        self.project_steps(steps, false, true)
    }
    /// Select normal post-operation roots after completing the checked operation's metadata phase.
    /// # Errors
    /// Failure restores prior bindings; spent work remains spent.
    pub fn after_steps_with_roots(
        &mut self,
        steps: &[ResourceStep],
    ) -> Result<Vec<ResourceDataStep>, Failure> {
        self.project_steps(steps, true, true)
    }
    fn project_steps(
        &mut self,
        steps: &[ResourceStep],
        completed: bool,
        retain_roots: bool,
    ) -> Result<Vec<ResourceDataStep>, Failure> {
        let before = self.values.clone();
        let result = (|| {
            if completed {
                self.complete_operation()?;
            }
            steps
                .iter()
                .map(|step| {
                    let roots = if retain_roots {
                        self.containing_roots(step)?
                    } else {
                        BTreeMap::new()
                    };
                    let destination = if retain_roots {
                        self.partial_destination(step)?.map(Box::new)
                    } else {
                        None
                    };
                    Ok(ResourceDataStep {
                        operation: self.step(step)?,
                        roots,
                        destination,
                    })
                })
                .collect::<Result<_, Failure>>()
        })();
        if result.is_err() {
            self.values = before;
        }
        result
    }
    fn complete_operation(&mut self) -> Result<(), Failure> {
        if let Operation::CommitCall(call) = self
            .body
            .graph()
            .points()
            .get(self.at.point)
            .and_then(|point| point.actions.get(self.at.action))
            .ok_or(INVALID)?
            .operation
        {
            let Some(ExpressionKind::Call { arguments, .. }) = self
                .body
                .typed()
                .body()
                .expressions()
                .get(call)
                .map(|expression| &expression.kind)
            else {
                return Err(INVALID);
            };
            for (index, argument) in arguments.iter().enumerate() {
                if argument.operation != nocter_model::ValueInputOperation::Edit {
                    continue;
                }
                let holder = match argument.action {
                    ArgumentAction::AccessOwnedValue => Holder::Expression(argument.value),
                    ArgumentAction::ReserveTransfer { .. }
                    | ArgumentAction::TakeValue
                    | ArgumentAction::CopyPlace(_) => Holder::Temporary { call, index },
                    _ => continue,
                };
                self.values.insert(holder, Binding::AfterAction);
            }
        }
        Ok(())
    }
    fn step(&mut self, step: &ResourceStep) -> Result<ResourceStep<Payload>, Failure> {
        let mut source = None;
        let mut created = None;
        let result = step.try_map_payloads_at(|site, contract| {
            let key = match site {
                PayloadSite::Source(place) => {
                    if matches!(step, ResourceStep::ReplaceReferent { destination, .. } if destination == place) {
                        self.retired(place, contract.data())?
                    } else {
                        self.source(place, contract.data())?
                    }
                }
                PayloadSite::Destination(holder) => {
                    let key = self.destination(step, holder, contract.data(), source.as_ref())?;
                    created = Some((holder, key.clone()));
                    key
                }
            };
            if matches!(site, PayloadSite::Source(place)
                if !matches!(step, ResourceStep::ReplaceReferent { source, .. } if place != source))
            {
                source = Some(key.clone());
            }
            contract
                .instantiate(self.program, &key)
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
        self.program.instantiate_closed_data(target).ok()
    }
    fn retired(&mut self, place: &Place, data: TypeId) -> Result<DataInstanceKey, Failure> {
        if let Some(key) = self.closed(data) {
            return Ok(key);
        }
        if self.values.contains_key(&place.holder()) {
            return self.source(place, data);
        }
        let target = self.write_target()?;
        let source = self
            .evidence
            .value_sources(self.frame.code())
            .and_then(|graph| graph.flow())
            .and_then(|flow| flow.referent(self.at, target))
            .ok_or(INVALID)?;
        super::instantiate_value_data(
            self.program,
            self.evidence,
            self.frame,
            source,
            data,
            self.work,
        )
        .map_err(Failure::Instantiation)
    }
    fn source(&mut self, place: &Place, target: TypeId) -> Result<DataInstanceKey, Failure> {
        let program = self.program;
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
        let program = self.program;
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
            ResourceStep::Assemble { fields, .. } => self.assemble(holder, target, fields),
            ResourceStep::Construct { .. } | ResourceStep::Receive { .. } => {
                self.stored(&holder.place(), target, true)
            }
            _ => Err(INVALID),
        }
    }
    fn assemble(
        &mut self,
        holder: Holder,
        target: TypeId,
        fields: &[(ValueField, Place)],
    ) -> Result<DataInstanceKey, Failure> {
        let program = self.program;
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
        if matches!(
            program.data_schema(target),
            Some(PayloadSchema::Optional(_) | PayloadSchema::Fallible(_))
        ) {
            // Inactive branches have representation constraints, not extra assembled values.
            // The original completed definition owns those constraints; resource fields still
            // validate only the values actually supplied by this ordered operation.
            let key = self.stored(&holder.place(), target, true)?;
            for (field, actual) in values {
                let expected = key.component(program, field).map_err(|_| INVALID)?;
                if !actual.has_same_representation(program, &expected) {
                    return Err(INVALID);
                }
            }
            return Ok(key);
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
        let program = self.program;
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
        let slot = source_slot(place.holder())?;
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
        super::instantiate_stored_data(
            self.program,
            self.evidence,
            self.frame,
            &selected,
            target,
            self.work,
        )
        .map_err(Failure::Instantiation)
    }
}
fn source_slot(holder: Holder) -> Result<ValueSourceSlot, Failure> {
    Ok(match holder {
        Holder::Local(local) => ValueSourceSlot::Local(local),
        Holder::Expression(expression) => ValueSourceSlot::Expression(expression),
        Holder::Prepared { call, index } | Holder::Temporary { call, index } => {
            ValueSourceSlot::Argument { call, index }
        }
        Holder::Return => ValueSourceSlot::Return,
        Holder::IndexSnapshot(_) | Holder::ArrayFrontier(_) => return Err(INVALID),
    })
}
fn metadata_path<I: Copy>(place: &Place, array: I) -> ValuePath<I> {
    ValuePath::new(place.fields().fields().iter().map(|&field| match field {
        ValueProjection::Array(_) => ValueProjection::Array(array),
        ValueProjection::Record(field) => ValueProjection::Record(field),
        ValueProjection::Tuple(index) => ValueProjection::Tuple(index),
        ValueProjection::Variant(primary) => ValueProjection::Variant(primary),
    }))
}

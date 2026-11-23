//! Project one already checked graph and its resource alternatives; no source/ownership decisions.
mod arrays;
mod invocations;
use invocations::Selection;
mod resources;
mod source;
#[cfg(test)]
mod tests;
use super::super::{
    ArgumentAction, Binary, BoundaryAction, ExpressionKind, Referent, SelectionAction, Statement,
    flow,
};
use flow::resources::{Holder, Step};
use nocter_constant_evaluation::{
    CompileTimeBinaryOperation as B, CompileTimeComparisonOperation as C,
    CompileTimeUnaryOperation as U, state as plan,
};
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::{
    AccessExpressionId, AccessFlowPointId, AccessPlaceId, FieldPath, TypeId, ValueAccessMode,
    ValueInputOperation,
};
pub(super) use source::Source;
use source::{Instances, initializer};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug)]
pub(super) enum Failure {
    Integrity,
    WorkLimit(Option<plan::Site>),
    UnresolvedContext(Option<plan::Site>),
}
impl Failure {
    fn at(self, site: plan::Site) -> Self {
        match self {
            Self::Integrity => Self::Integrity,
            Self::WorkLimit(cause) => Self::WorkLimit(cause.or(Some(site))),
            Self::UnresolvedContext(cause) => Self::UnresolvedContext(cause.or(Some(site))),
        }
    }
    pub(super) fn into_plan(self) -> plan::PlanFailure {
        let (rule, site) = match self {
            Self::Integrity => (plan::Rule::InvalidPlan, None),
            Self::WorkLimit(site) => (plan::Rule::PlanWorkLimit, site),
            Self::UnresolvedContext(site) => (plan::Rule::UnresolvedContext, site),
        };
        plan::PlanFailure { rule, site }
    }
}
impl From<flow::IntegrityError> for Failure {
    fn from(_: flow::IntegrityError) -> Self {
        Self::Integrity
    }
}
impl From<crate::value_access::InstantiationFailure> for Failure {
    fn from(failure: crate::value_access::InstantiationFailure) -> Self {
        match failure {
            crate::value_access::InstantiationFailure::WorkLimit => Self::WorkLimit(None),
            crate::value_access::InstantiationFailure::Unresolved => Self::UnresolvedContext(None),
            _ => Self::Integrity,
        }
    }
}
impl From<crate::value_access::PreparedDataFailure> for Failure {
    fn from(failure: crate::value_access::PreparedDataFailure) -> Self {
        match failure {
            crate::value_access::PreparedDataFailure::Argument { failure, .. }
            | crate::value_access::PreparedDataFailure::Control(failure) => failure.into(),
            crate::value_access::PreparedDataFailure::Body => Self::Integrity,
        }
    }
}
impl From<crate::value_access::ResourceDataFailure> for Failure {
    fn from(failure: crate::value_access::ResourceDataFailure) -> Self {
        match failure {
            crate::value_access::ResourceDataFailure::Integrity => Self::Integrity,
            crate::value_access::ResourceDataFailure::Instantiation(failure) => failure.into(),
        }
    }
}
impl From<crate::value_access::ValueDataFailure> for Failure {
    fn from(failure: crate::value_access::ValueDataFailure) -> Self {
        match failure {
            crate::value_access::ValueDataFailure::Body => Self::Integrity,
            crate::value_access::ValueDataFailure::Value { failure, .. }
            | crate::value_access::ValueDataFailure::Control(failure) => failure.into(),
        }
    }
}
const INVALID: Failure = Failure::Integrity;
type Result<T> = std::result::Result<T, Failure>;
use super::Types;
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Key {
    Resource(Holder),
    EnvironmentBacking,
    Operand(AccessExpressionId, usize),
    Predicate(AccessExpressionId),
}
/// Product evaluation starts only from the exact root accepted by ordinary const admission.
pub(super) fn prepare<'a, 'work, F: Fn(nocter_model::AccessCodeId) -> Option<plan::Value>>(
    admission: crate::value_access::accepted::ConstAdmission<'a>,
    values: F,
    work: &'work mut plan::WorkBudget,
) -> Result<(plan::Program, Source<'a, 'work, F>)> {
    let (checked, root, conditions) = admission.into_parts();
    Source::prepare(checked, root, conditions, values, work)
}
struct Builder<'a, 'b> {
    checked: &'a super::super::accepted::Checked,
    work: &'b mut plan::WorkBudget,
    query: &'a flow::Query,
    program: &'a Program,
    types: &'b mut Types,
    instance: &'a CodeInstanceKey,
    result: nocter_declarations::value_access::DataInstanceKey,
    instances: &'b mut Instances,
    slots: Vec<plan::Slot>,
    keys: BTreeMap<Key, usize>,
    invocations: BTreeMap<AccessExpressionId, Selection>,
    prepared: crate::value_access::PreparedData,
    values: crate::value_access::ValueData,
    resources: BTreeMap<flow::Location, Vec<resources::Projected>>,
    control: crate::value_access::InstanceControl,
    conditions: Box<[plan::InputCondition]>,
}
impl Builder<'_, '_> {
    fn expression(&self, id: AccessExpressionId) -> Result<super::super::ValueType> {
        Ok(self
            .query
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .ty)
    }
    fn data(&self, id: AccessExpressionId) -> Result<TypeId> {
        Ok(self.expression(id)?.access().ok_or(INVALID)?.data())
    }
    fn slot(&mut self, key: Key, data: TypeId, storage: plan::Storage) -> Result<usize> {
        let data =
            match key {
                Key::EnvironmentBacking => {
                    self.types
                        .get_environment(self.program, self.instance, data)?
                }
                Key::Resource(Holder::Local(local))
                    if self
                        .query
                        .typed()
                        .body()
                        .inputs()
                        .ordered()
                        .any(|input| input.local == local) =>
                {
                    let key =
                        self.checked
                            .instantiate_input_data(self.instance, local, self.work)?;
                    self.types.get_instance(self.program, key)?
                }
                Key::Resource(Holder::Expression(id))
                    if self.query.typed().body().expressions().get(id).is_some_and(
                        |expression| matches!(expression.kind, ExpressionKind::Call { .. }),
                    ) =>
                {
                    let data = match self.invocation(id)? {
                        Selection::One(frame) => self
                            .instances
                            .boundary(self.checked, &frame, self.work)?
                            .result
                            .clone(),
                        Selection::Choice { result, .. } => result,
                    };
                    self.types.get_instance(self.program, data)?
                }
                Key::Resource(Holder::Return) => {
                    self.types.get_instance(self.program, self.result.clone())?
                }
                Key::Resource(
                    Holder::Prepared { call, index } | Holder::Temporary { call, index },
                ) => {
                    let data = self.prepared.get(call, index).ok_or(INVALID)?.clone();
                    self.types.get_instance(self.program, data)?
                }
                _ => self.types.get_instance(
                    self.program,
                    self.program
                        .instantiate_closed_data(data)
                        .map_err(|_| INVALID)?,
                )?,
            };
        self.slot_data(key, data, storage)
    }
    fn slot_data(&mut self, key: Key, data: usize, storage: plan::Storage) -> Result<usize> {
        if let Some(&slot) = self.keys.get(&key) {
            let existing = self.slots.get(slot).ok_or(INVALID)?;
            let temporal = matches!(key, Key::Resource(Holder::Local(_)));
            if existing.storage != storage || (existing.data != data && !temporal) {
                return Err(INVALID);
            }
            return Ok(slot);
        }
        let slot = self.slots.len();
        self.slots.push(plan::Slot { data, storage });
        self.keys.insert(key, slot);
        Ok(slot)
    }
    fn holder_contract(&self, holder: Holder) -> Result<(TypeId, plan::Storage)> {
        let (data, storage) = match holder {
            Holder::Local(id) => {
                let local = self.query.typed().body().locals().get(id).ok_or(INVALID)?;
                (
                    local.data,
                    if local.kind.access_mode() == ValueAccessMode::Own {
                        plan::Storage::Owned
                    } else {
                        plan::Storage::Alias
                    },
                )
            }
            Holder::Expression(id) => (
                self.data(id)?,
                if self.expression(id)?.access().ok_or(INVALID)?.mode() == ValueAccessMode::Own {
                    plan::Storage::Owned
                } else {
                    plan::Storage::Alias
                },
            ),
            Holder::Prepared { call, index } | Holder::Temporary { call, index } => {
                let ExpressionKind::Call { arguments, .. } = &self
                    .query
                    .typed()
                    .body()
                    .expressions()
                    .get(call)
                    .ok_or(INVALID)?
                    .kind
                else {
                    return Err(INVALID);
                };
                (
                    self.data(arguments.get(index).ok_or(INVALID)?.value)?,
                    plan::Storage::Owned,
                )
            }
            Holder::IndexSnapshot(index) => {
                if flow::resources::RetainedIndex::for_selection(
                    self.query.typed(),
                    index.selection(),
                ) != Some(index)
                {
                    return Err(INVALID);
                }
                (
                    self.program
                        .data_type(nocter_model::BuiltinType::Usize)
                        .ok_or(INVALID)?,
                    plan::Storage::Owned,
                )
            }
            Holder::ArrayFrontier(frontier) => {
                frontier.array(self.query.typed()).ok_or(INVALID)?;
                (
                    self.program
                        .data_type(nocter_model::BuiltinType::Usize)
                        .ok_or(INVALID)?,
                    plan::Storage::Owned,
                )
            }
            Holder::Return => (self.query.typed().result_data(), plan::Storage::Owned),
        };
        Ok((data, storage))
    }
    fn holder(&mut self, holder: Holder) -> Result<usize> {
        if let Some(&slot) = self.keys.get(&Key::Resource(holder)) {
            return Ok(slot);
        }
        let (data, storage) = self.holder_contract(holder)?;
        self.slot(Key::Resource(holder), data, storage)
    }
    fn place_data(&self, place: &plan::Place) -> Result<usize> {
        Ok(self.types.select_path(
            self.slots.get(place.slot).ok_or(INVALID)?.data,
            &place.fields,
        )?)
    }
    fn holder_from_place(&mut self, holder: Holder, source: &plan::Place) -> Result<usize> {
        let (_, storage) = self.holder_contract(holder)?;
        // A retained original definition already selected this view's context. A backing slot's
        // initial representation cannot overwrite that receipt after an intervening transfer.
        if let Some(&slot) = self.keys.get(&Key::Resource(holder)) {
            if self
                .slots
                .get(slot)
                .is_none_or(|slot| slot.storage != storage)
            {
                return Err(INVALID);
            }
            return Ok(slot);
        }
        let data = self.place_data(source)?;
        self.slot_data(Key::Resource(holder), data, storage)
    }
    fn component_position(&self, field: nocter_model::ValueField) -> Result<usize> {
        match field {
            nocter_model::ValueField::Array(index) => usize::try_from(index).map_err(|_| INVALID),
            _ => self.program.component_position(field).ok_or(INVALID),
        }
    }
    fn fields(&self, fields: &FieldPath) -> Result<plan::ProjectionPath> {
        fields
            .fields()
            .iter()
            .map(|id| Ok(plan::Projection::Field(self.component_position(*id)?)))
            .collect()
    }
    fn referent(&mut self, source: Referent) -> Result<plan::Place> {
        match source {
            Referent::Place(place) => self.selected(place),
            Referent::Evaluated(id)
                if self.query.typed().body().referent_data(source).is_some() =>
            {
                self.value(id)
            }
            Referent::Evaluated(_) => Err(INVALID),
        }
    }
    fn resource(&mut self, place: &flow::resources::Place) -> Result<plan::Place> {
        Ok(plan::Place {
            slot: self.holder(place.holder())?,
            fields: self.resource_fields(place.fields())?,
        })
    }
    fn resource_fields(&mut self, fields: &flow::resources::Path) -> Result<plan::ProjectionPath> {
        fields
            .fields()
            .iter()
            .map(|field| {
                Ok(match *field {
                    nocter_model::ValueProjection::Record(id) => plan::Projection::Field(
                        self.component_position(nocter_model::ValueField::Record(id))?,
                    ),
                    nocter_model::ValueProjection::Tuple(index) => plan::Projection::Field(index),
                    nocter_model::ValueProjection::Variant(primary) => {
                        plan::Projection::Field(usize::from(!primary))
                    }
                    nocter_model::ValueProjection::EnumField { .. } => return Err(INVALID),
                    nocter_model::ValueProjection::Array(flow::resources::Index::Constant(
                        index,
                    )) => plan::Projection::Field(usize::try_from(index).map_err(|_| INVALID)?),
                    nocter_model::ValueProjection::Array(flow::resources::Index::Retained(
                        index,
                    )) => plan::Projection::Index {
                        slot: self.holder(Holder::IndexSnapshot(index))?,
                        bound: index.bound(),
                    },
                })
            })
            .collect()
    }
    fn missing(&mut self, missing: &flow::resources::Missing) -> Result<plan::Missing> {
        Ok(match missing {
            flow::resources::Missing::Value(fields) => {
                plan::Missing::Value(self.resource_fields(fields)?)
            }
            flow::resources::Missing::ArrayPrefix { array, frontier } => {
                plan::Missing::ArrayPrefix {
                    base: self.resource_fields(array)?,
                    endpoint: self.holder(Holder::ArrayFrontier(*frontier))?,
                    bound: frontier.bound(),
                }
            }
        })
    }
    fn selected(&mut self, id: AccessPlaceId) -> Result<plan::Place> {
        let place = self.query.typed().body().places().get(id).ok_or(INVALID)?;
        Ok(plan::Place {
            slot: self.holder(Holder::Local(place.local))?,
            fields: self.fields(&place.fields)?,
        })
    }
    fn value(&mut self, id: AccessExpressionId) -> Result<plan::Place> {
        if let ExpressionKind::Place(place) = self
            .query
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .kind
        {
            self.selected(place)
        } else {
            Ok(plan::Place::root(self.holder(Holder::Expression(id))?))
        }
    }
    fn aux(&mut self, key: Key, expression: AccessExpressionId) -> Result<plan::Place> {
        Ok(plan::Place::root(self.slot(
            key,
            self.data(expression)?,
            plan::Storage::Auxiliary,
        )?))
    }
    #[allow(clippy::too_many_lines)]
    fn effect(&mut self, step: &Step) -> Result<plan::Effect> {
        Ok(match step {
            Step::Construct { destination, .. } => {
                plan::Effect::Construct(self.holder(*destination)?)
            }
            Step::Receive { result, .. } => {
                plan::Effect::Construct(self.holder(Holder::Expression(*result))?)
            }
            Step::ConstructActivation { .. } | Step::DriveFuture { .. } => return Err(INVALID),
            Step::Move {
                source,
                destination,
                ..
            } => plan::Effect::Move {
                source: self.resource(source)?,
                destination: self.resource(destination)?,
            },
            Step::MoveArrayElement {
                array,
                index,
                destination,
                ..
            } => plan::Effect::MoveArrayElement {
                array: self.resource(array)?,
                selected: plan::Place::root(self.holder(Holder::Expression(index.selection()))?),
                destination: self.holder(*destination)?,
            },
            Step::ConsolidateArrayPrefix {
                array, frontier, ..
            } => plan::Effect::ConsolidateArrayPrefix {
                source: self.resource(array)?,
                endpoint: self.holder(Holder::ArrayFrontier(*frontier))?,
            },
            Step::Dispose { source, .. } => plan::Effect::Dispose(self.resource(source)?),
            Step::DisposeArrayRemainder { source, .. } => {
                plan::Effect::DisposeArrayRemainder(self.resource(source)?)
            }
            Step::ReleaseContainer { source, .. } => plan::Effect::Release(self.resource(source)?),
            Step::Invoke {
                source,
                call,
                index,
                ..
            } => {
                let source = self.resource(source)?;
                let offset = self.call_input_offset(*call)?;
                let inputs = if let Some(position) = index.checked_sub(offset) {
                    let mut payload = source.clone();
                    if offset == 0 && *index == 0 && self.captured_call(*call)? {
                        payload.fields = payload
                            .fields
                            .iter()
                            .copied()
                            .chain([plan::Projection::Field(0)])
                            .collect();
                    }
                    vec![(position, payload)].into()
                } else {
                    Box::new([]) as Box<[_]>
                };
                plan::Effect::Invoke { source, inputs }
            }
            Step::ReplaceReferent {
                destination,
                source,
                ..
            } => plan::Effect::Replace {
                source: self.resource(source)?,
                destination: self.resource(destination)?,
            },
            Step::Assemble {
                destination,
                fields,
                ..
            } => plan::Effect::Assemble {
                destination: self.holder(*destination)?,
                fields: fields
                    .iter()
                    .map(|(field, source)| {
                        Ok((self.component_position(*field)?, self.resource(source)?))
                    })
                    .collect::<Result<_>>()?,
            },
            Step::DisposeArrayElements { source, range, .. } => {
                plan::Effect::DisposeArrayElements {
                    source: self.resource(source)?,
                    range: range.try_map_index(|index| self.holder(index.holder()))?,
                }
            }
            Step::AdaptFunction {
                source,
                destination,
                from,
                payload,
            } => {
                if !self.program.function_binding(from.data(), payload.data()) {
                    return Err(INVALID);
                }
                plan::Effect::AdaptFunction {
                    source: self.resource(source)?,
                    destination: plan::Place::root(self.holder(*destination)?),
                }
            }
            Step::Transport {
                source,
                destination,
                from,
                payload,
            } => {
                if !self
                    .query
                    .typed()
                    .result_evidence
                    .as_ref()
                    .is_some_and(|evidence| {
                        evidence.permits_transport(self.program, from.data(), payload.data())
                    })
                {
                    return Err(INVALID);
                }
                plan::Effect::Transport {
                    source: self.resource(source)?,
                    destination: plan::Place::root(self.holder(*destination)?),
                }
            }
            Step::Capture {
                source,
                destination,
                environment,
                payload,
            } => {
                let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
                    self.program.data_schema(payload.data())
                else {
                    return Err(INVALID);
                };
                let nocter_model::ValueFunctionOrigin::Closure(code) = witness.origin else {
                    return Err(INVALID);
                };
                if self.program.closure(code).ok_or(INVALID)?.environment() != environment.data() {
                    return Err(INVALID);
                }
                plan::Effect::Assemble {
                    destination: self.holder(*destination)?,
                    fields: Box::new([(0, self.resource(source)?)]),
                }
            }
        })
    }
    fn computation(&mut self, op: &flow::Operation) -> Result<plan::Operation> {
        Ok(match *op {
            flow::Operation::Evaluate(id) => return self.evaluate(id),
            flow::Operation::PrepareOperand {
                expression,
                operand,
                value,
            } => plan::Operation::Copy {
                source: self.value(value)?,
                destination: self.aux(Key::Operand(expression, operand), value)?,
            },
            flow::Operation::Condition(value) => plan::Operation::Copy {
                source: self.value(value)?,
                destination: self.aux(Key::Predicate(value), value)?,
            },
            flow::Operation::BooleanConstant { result, value } => plan::Operation::Literal {
                destination: self.holder(Holder::Expression(result))?,
                value: plan::Value::boolean(value),
            },
            flow::Operation::BooleanAnswer { source, result } => plan::Operation::Copy {
                source: self.value(source)?,
                destination: plan::Place::root(self.holder(Holder::Expression(result))?),
            },
            flow::Operation::Forward { source, result, .. } => {
                if self.expression(result)?.access().ok_or(INVALID)?.mode() == ValueAccessMode::Own
                {
                    plan::Operation::None
                } else {
                    let source = self.value(source)?;
                    plan::Operation::Alias {
                        destination: self.holder_from_place(Holder::Expression(result), &source)?,
                        source,
                    }
                }
            }
            flow::Operation::Bind { local, value } => {
                if self
                    .query
                    .typed()
                    .body()
                    .locals()
                    .get(local)
                    .ok_or(INVALID)?
                    .kind
                    .access_mode()
                    == ValueAccessMode::Own
                {
                    plan::Operation::None
                } else {
                    let source = self.value(value)?;
                    plan::Operation::Alias {
                        destination: self.holder_from_place(Holder::Local(local), &source)?,
                        source,
                    }
                }
            }
            flow::Operation::CompoundAssign {
                statement,
                target,
                value,
            } => {
                let Statement::CompoundAssign { operation, .. } = self
                    .query
                    .typed()
                    .body()
                    .statements()
                    .get(statement)
                    .ok_or(INVALID)?
                else {
                    return Err(INVALID);
                };
                let target = self.referent(target)?;
                binary(*operation, target.clone(), target, self.value(value)?)?
            }
            flow::Operation::PrepareArgument {
                call,
                argument: index,
            } => return self.argument(call, index),
            flow::Operation::CommitCall(id) => return self.call(id),
            flow::Operation::ConstructFuture(_) | flow::Operation::DriveFuture { .. } => {
                return Err(INVALID);
            }
            flow::Operation::IndexFailure(_) => plan::Operation::BoundsTrap,
            flow::Operation::BeginEvaluation(_)
            | flow::Operation::EndEvaluation(_)
            | flow::Operation::Assign { .. }
            | flow::Operation::InspectEnum { .. }
            | flow::Operation::Discard(_)
            | flow::Operation::Drop(_)
            | flow::Operation::StageReturn(_)
            | flow::Operation::ExitScope { .. }
            | flow::Operation::CancelCall { .. } => plan::Operation::None,
        })
    }
    fn argument(&mut self, call: AccessExpressionId, index: usize) -> Result<plan::Operation> {
        let ExpressionKind::Call { arguments, .. } = &self
            .query
            .typed()
            .body()
            .expressions()
            .get(call)
            .ok_or(INVALID)?
            .kind
        else {
            return Err(INVALID);
        };
        Ok(
            if let ArgumentAction::CopyPlace(place) = arguments.get(index).ok_or(INVALID)?.action {
                plan::Operation::Copy {
                    source: self.referent(place)?,
                    destination: plan::Place::root(self.holder(Holder::Prepared { call, index })?),
                }
            } else {
                plan::Operation::None
            },
        )
    }
    fn captured_call(&mut self, id: AccessExpressionId) -> Result<bool> {
        Ok(matches!(
            self.invocation(id)?,
            Selection::One(frame) if matches!(frame.code(), nocter_model::ValueCode::Closure(_))
        ))
    }
    fn call_input_offset(&mut self, call: AccessExpressionId) -> Result<usize> {
        if matches!(self.invocation(call)?, Selection::Choice { .. }) {
            return Ok(1);
        }
        if self.captured_call(call)? {
            return Ok(0);
        }
        let ExpressionKind::Call {
            kind, arguments, ..
        } = &self
            .query
            .typed()
            .body()
            .expressions()
            .get(call)
            .ok_or(INVALID)?
            .kind
        else {
            return Err(INVALID);
        };
        Ok(arguments.len() - kind.explicit_arguments(arguments).ok_or(INVALID)?.len())
    }
    fn call(&mut self, id: AccessExpressionId) -> Result<plan::Operation> {
        let key = match self.invocation(id)? {
            Selection::One(frame) => frame,
            selection @ Selection::Choice { .. } => return self.selected_call(id, selection),
        };
        let target = key.code();
        let captured = matches!(target, nocter_model::ValueCode::Closure(_));
        let arguments = self
            .query
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?;
        let ExpressionKind::Call { arguments, .. } = &arguments.kind else {
            return Err(INVALID);
        };
        let target_inputs = self
            .instances
            .boundary(self.checked, &key, self.work)?
            .inputs
            .clone();
        let target_instance = self.instances.intern(key.clone())?;
        let arguments = arguments.clone();
        let mut inputs = Vec::new();
        let offset = self.call_input_offset(id)?;
        if arguments.len().checked_sub(offset) != Some(target_inputs.len()) {
            return Err(INVALID);
        }
        for (index, argument) in arguments.iter().enumerate().skip(offset) {
            let input = target_inputs.get(index - offset).ok_or(INVALID)?;
            // A taking callable view can lend its retained environment to a read/edit body.
            // The receiver adapter owns that difference; explicit inputs retain exact operations.
            if !(captured && index == 0) && input.operation != argument.operation {
                return Err(INVALID);
            }
            inputs.push(self.argument_input(
                id,
                index,
                argument,
                &input.data,
                captured && index == 0,
            )?);
        }
        Ok(plan::Operation::Call {
            destination: self.holder(Holder::Expression(id))?,
            target: target_instance,
            inputs: inputs.into(),
        })
    }
    fn argument_input(
        &mut self,
        id: AccessExpressionId,
        index: usize,
        argument: &super::super::Argument,
        data: &nocter_declarations::value_access::DataInstanceKey,
        environment: bool,
    ) -> Result<plan::Input> {
        if argument.operation == ValueInputOperation::Take {
            return Ok(plan::Input::Take(
                self.types.get_instance(self.program, data.clone())?,
            ));
        }
        let (id, index, action) = match argument.action {
            ArgumentAction::ReuseRead { call, index }
                if argument.operation == ValueInputOperation::Read && !environment =>
            {
                let original = self
                    .query
                    .typed()
                    .body()
                    .call_arguments(call)
                    .filter(|arguments| arguments.retains_reads())
                    .and_then(|arguments| arguments.get(index))
                    .ok_or(INVALID)?;
                (call, index, &original.action)
            }
            _ => (id, index, &argument.action),
        };
        let mut place = match action {
            ArgumentAction::ReserveRead(place) | ArgumentAction::ReserveUpdate(place) => {
                self.referent(*place)?
            }
            ArgumentAction::ReserveTransfer { .. }
            | ArgumentAction::AccessOwnedValue
            | ArgumentAction::CopyPlace(_)
            | ArgumentAction::TakeValue => {
                plan::Place::root(self.holder(Holder::Temporary { call: id, index })?)
            }
            ArgumentAction::ReuseRead { .. }
            | ArgumentAction::PendingValue(_)
            | ArgumentAction::Unproduced
            | ArgumentAction::Interrupted => return Err(INVALID),
        };
        if environment {
            place.fields = place
                .fields
                .iter()
                .copied()
                .chain([plan::Projection::Field(0)])
                .collect();
        }
        Ok(match argument.operation {
            ValueInputOperation::Read => plan::Input::Read(place),
            ValueInputOperation::Edit => plan::Input::Edit(place),
            ValueInputOperation::Take => unreachable!(),
        })
    }
    #[allow(clippy::too_many_lines)]
    fn evaluate(&mut self, id: AccessExpressionId) -> Result<plan::Operation> {
        let kind = self
            .query
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .kind
            .clone();
        let selected = match &kind {
            ExpressionKind::Place(place) => Some(self.selected(*place)?),
            ExpressionKind::Boundary {
                source,
                action:
                    BoundaryAction::ReadPlace
                    | BoundaryAction::DelegateUpdate
                    | BoundaryAction::TemporaryPlace
                    | BoundaryAction::CopyPayload,
            }
            | ExpressionKind::RequirePrimary { source } => Some(self.value(*source)?),
            // These checks retain the whole carrier; a later Select extracts the payload.
            ExpressionKind::GuardView { source, .. } => Some(self.referent(*source)?),
            ExpressionKind::Select {
                source,
                fields,
                action:
                    SelectionAction::Access
                    | SelectionAction::PreparedAccess
                    | SelectionAction::CopyPayload,
            } => {
                let mut place = self.value(*source)?;
                place.fields = place
                    .fields
                    .iter()
                    .copied()
                    .chain(self.fields(fields)?.iter().copied())
                    .collect();
                Some(place)
            }
            _ => None,
        };
        let destination = if let Some(source) = selected {
            self.holder_from_place(Holder::Expression(id), &source)?
        } else if let ExpressionKind::Index { base, .. } = kind {
            let mut source = self.value(base)?;
            // Use an existing original view receipt before consulting the source slot's initial
            // context. Element zero represents homogeneous metadata, not the evaluated index.
            source.fields = source
                .fields
                .iter()
                .copied()
                .chain([plan::Projection::Field(0)])
                .collect();
            self.holder_from_place(Holder::Expression(id), &source)?
        } else {
            self.holder(Holder::Expression(id))?
        };
        Ok(match kind {
            ExpressionKind::Constant(ref value) => plan::Operation::Literal {
                destination,
                value: plan::Value::from_frozen(value).ok_or(INVALID)?,
            },
            ExpressionKind::Integer(v) => plan::Operation::Literal {
                destination,
                value: plan::Value::integer(v),
            },
            ExpressionKind::Boolean(v) => plan::Operation::Literal {
                destination,
                value: plan::Value::boolean(v),
            },
            ExpressionKind::Function(_) | ExpressionKind::Unit => plan::Operation::Literal {
                destination,
                value: plan::Value::unit(),
            },
            ExpressionKind::PrimaryCondition { source } => plan::Operation::PrimaryCondition {
                source: self.referent(source)?,
                destination,
            },
            ExpressionKind::EnumCondition { source, tag, .. } => plan::Operation::EnumCondition {
                source: self.value(source)?,
                tag,
                destination,
            },
            ExpressionKind::GuardView { source, primary } => plan::Operation::RequireVariant {
                source: self.referent(source)?,
                primary,
                view: Some(destination),
            },
            ExpressionKind::RequirePrimary { source } => plan::Operation::RequireVariant {
                source: self.value(source)?,
                primary: true,
                view: None,
            },
            ExpressionKind::Repeat { seed } => self.repeat_operation(id, seed, destination)?,
            ExpressionKind::Index {
                base,
                index,
                action,
                ..
            } => self.index_operation(destination, base, index, action)?,
            ExpressionKind::Place(place) => plan::Operation::Alias {
                destination,
                source: self.selected(place)?,
            },
            ExpressionKind::Boundary {
                source,
                action:
                    BoundaryAction::ReadPlace
                    | BoundaryAction::DelegateUpdate
                    | BoundaryAction::TemporaryPlace,
            } => plan::Operation::Alias {
                destination,
                source: self.value(source)?,
            },
            ExpressionKind::Boundary {
                source,
                action: BoundaryAction::CopyPayload,
            } => plan::Operation::Copy {
                destination: plan::Place::root(destination),
                source: self.value(source)?,
            },
            ExpressionKind::Boundary {
                action: BoundaryAction::TakeValue,
                ..
            }
            | ExpressionKind::Move(_)
            | ExpressionKind::AdaptFunction { .. }
            | ExpressionKind::Closure { .. }
            | ExpressionKind::Aggregate { .. }
            | ExpressionKind::Select {
                action: SelectionAction::MovePayload,
                ..
            } => plan::Operation::None,
            ExpressionKind::Select {
                source,
                fields,
                action:
                    action @ (SelectionAction::CopyPayload
                    | SelectionAction::Access
                    | SelectionAction::PreparedAccess),
            } => self.component_operation(destination, source, &fields, action)?,
            ExpressionKind::Unary { operation, operand } => plan::Operation::Unary {
                destination,
                operation: match operation {
                    super::super::Unary::Not => U::LogicalNot,
                    super::super::Unary::CheckedNegate => U::Negate,
                },
                operand: self.aux(Key::Operand(id, 0), operand)?,
            },
            ExpressionKind::Binary {
                operation,
                left,
                right,
            } => binary(
                operation,
                plan::Place::root(destination),
                self.aux(Key::Operand(id, 0), left)?,
                self.aux(Key::Operand(id, 1), right)?,
            )?,
            _ => return Err(INVALID),
        })
    }
    fn component_operation(
        &mut self,
        destination: usize,
        source: AccessExpressionId,
        fields: &FieldPath,
        action: SelectionAction,
    ) -> Result<plan::Operation> {
        let mut source = self.value(source)?;
        let mut path = source.fields.to_vec();
        path.extend(self.fields(fields)?.iter().copied());
        source.fields = path.into();
        Ok(match action {
            SelectionAction::Access | SelectionAction::PreparedAccess => plan::Operation::Alias {
                destination,
                source,
            },
            SelectionAction::CopyPayload => plan::Operation::Copy {
                destination: plan::Place::root(destination),
                source,
            },
            SelectionAction::MovePayload | SelectionAction::PendingPayload => return Err(INVALID),
        })
    }

    fn static_views(&self) -> Box<[(usize, nocter_model::ValueStatic)]> {
        self.query
            .typed()
            .body()
            .static_views()
            .filter_map(|(local, id)| {
                self.keys
                    .get(&Key::Resource(Holder::Local(local)))
                    .map(|&slot| (slot, id))
            })
            .collect()
    }
    fn environment_backing(&mut self) -> Result<Option<usize>> {
        self.query
            .typed()
            .body()
            .inputs()
            .environment()
            .filter(|input| input.operation != ValueInputOperation::Take)
            .map(|input| {
                let data = self
                    .query
                    .typed()
                    .body()
                    .locals()
                    .get(input.local)
                    .ok_or(INVALID)?
                    .data;
                self.slot(Key::EnvironmentBacking, data, plan::Storage::Auxiliary)
            })
            .transpose()
    }
    fn site(&self, location: flow::Location) -> plan::Site {
        plan::Site {
            code: self.instance.code(),
            point: location.point,
            action: location.action,
        }
    }
    fn action(&mut self, location: flow::Location, action: &flow::Action) -> Result<plan::Action> {
        let mut alternatives = Vec::new();
        let projected = self.resources.remove(&location).ok_or(INVALID)?;
        if projected.len() != self.query.access().resources().at(location).count() {
            return Err(INVALID);
        }
        for (transition, projected) in self.query.access().resources().at(location).zip(projected) {
            let owned = transition
                .condition()
                .owned()
                .map(|(holder, holes)| {
                    Ok((
                        self.holder(holder)?,
                        holes
                            .iter()
                            .map(|path| self.missing(path))
                            .collect::<Result<_>>()?,
                    ))
                })
                .collect::<Result<_>>()?;
            let (mut before, mut completion) = self.source_contexts(&projected)?;
            before.extend(self.effects(
                transition.before_steps(),
                &projected.before,
                &projected.roots,
            )?);
            let mut after =
                self.effects(transition.after_steps(), &projected.after, &projected.roots)?;
            after.append(&mut completion);
            alternatives.push(plan::Alternative {
                owned,
                before: before.into(),
                after: after.into(),
            });
        }
        Ok(plan::Action {
            operation: self.computation(&action.operation)?,
            alternatives: alternatives.into(),
        })
    }
    fn point(&mut self, id: AccessFlowPointId, point: &flow::Point) -> Result<plan::Point> {
        let reachable = self.control.reachable(id);
        let mut actions = Vec::new();
        if reachable {
            for (index, action) in point.actions.iter().enumerate() {
                let location = flow::Location {
                    point: id,
                    action: index,
                };
                let site = self.site(location);
                actions.push(
                    self.action(location, action)
                        .map_err(|failure| failure.at(site))?,
                );
            }
        }
        let terminator = if reachable {
            match self.control.terminator(id).ok_or(INVALID)? {
                flow::Terminator::Jump(next) => plan::Terminator::Jump(next),
                flow::Terminator::Branch { condition, yes, no } => plan::Terminator::Branch {
                    condition: self.aux(Key::Predicate(condition), condition)?,
                    yes,
                    no,
                },
                flow::Terminator::EnumSwitch {
                    source,
                    cases,
                    fallback,
                } => plan::Terminator::EnumSwitch {
                    source: self.value(source)?,
                    cases,
                    fallback,
                },
                flow::Terminator::Return => {
                    plan::Terminator::Return(plan::Place::root(self.holder(Holder::Return)?))
                }
                flow::Terminator::Unreachable => plan::Terminator::Unreachable,
            }
        } else {
            plan::Terminator::Unreachable
        };
        Ok(plan::Point {
            actions: actions.into(),
            terminator,
        })
    }
    fn build(mut self) -> Result<plan::Plan> {
        let inputs = self
            .query
            .typed()
            .body()
            .inputs()
            .ordered()
            .map(|i| Ok((self.holder(Holder::Local(i.local))?, i.operation)))
            .collect::<Result<_>>()?;
        // Qualify post-call environments before slot registration observes future cleanup
        // contracts. A changed materialized receiver must produce a source diagnostic, not
        // an inconsistent pair of immutable temporary-slot types.
        for (point, block) in self.query.graph().points().iter() {
            if self.control.reachable(point) {
                for (action, operation) in block.actions.iter().enumerate() {
                    if let flow::Operation::CommitCall(call) = operation.operation {
                        let at = flow::Location { point, action };
                        let site = self.site(at);
                        self.check_captured_completion(at, call)
                            .map_err(|failure| failure.at(site))?;
                    }
                }
            }
        }
        self.prepare_value_data()?;
        self.prepare_resource_data()?;
        let mut points = nocter_model::ArenaBuilder::new();
        for (id, point) in self.query.graph().points().iter() {
            if points.insert(self.point(id, point)?) != id {
                return Err(INVALID);
            }
        }
        if !self.resources.is_empty() {
            return Err(INVALID);
        }
        let statics = self.static_views();
        let environment_backing = self.environment_backing()?;
        Ok(plan::Plan {
            conditions: self.conditions,
            environment_backing,
            statics,
            inputs,
            slots: self.slots.into(),
            result: self.types.get_instance(self.program, self.result.clone())?,
            initializer: initializer(self.program, self.query.typed().code())?,
            entry: self.query.graph().entry(),
            points: points.finish(),
        })
    }
}
fn binary(
    operation: Binary,
    destination: plan::Place,
    left: plan::Place,
    right: plan::Place,
) -> Result<plan::Operation> {
    if let Some(value) = operation.fixed_boolean_result() {
        if !destination.fields.is_empty() {
            return Err(INVALID);
        }
        return Ok(plan::Operation::Literal {
            destination: destination.slot,
            value: plan::Value::boolean(value),
        });
    }
    let comparison = match operation {
        Binary::Equal => Some(C::Equal),
        Binary::NotEqual => Some(C::NotEqual),
        Binary::Less => Some(C::Less),
        Binary::LessEqual => Some(C::LessEqual),
        Binary::Greater => Some(C::Greater),
        Binary::GreaterEqual => Some(C::GreaterEqual),
        _ => None,
    };
    if let Some(operation) = comparison {
        if !destination.fields.is_empty() {
            return Err(INVALID);
        }
        return Ok(plan::Operation::Compare {
            destination: destination.slot,
            operation,
            left,
            right,
        });
    }
    let operation = match operation {
        Binary::CheckedAdd => B::Add,
        Binary::CheckedSubtract => B::Subtract,
        Binary::CheckedMultiply => B::Multiply,
        Binary::CheckedDivide => B::Divide,
        Binary::CheckedRemainder => B::Remainder,
        _ => return Err(INVALID),
    };
    Ok(plan::Operation::Binary {
        destination,
        operation,
        left,
        right,
    })
}

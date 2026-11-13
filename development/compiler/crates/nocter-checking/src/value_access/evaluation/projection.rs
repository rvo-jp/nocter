//! Project one already checked graph and its resource alternatives; no source/ownership decisions.
mod arrays;
use super::super::{
    ArgumentAction, Binary, BoundaryAction, ExpressionKind, Referent, SelectionAction, Statement,
    flow,
};
use flow::resources::{Holder, Step};
use nocter_constant_evaluation::{
    CompileTimeBinaryOperation as B, CompileTimeComparisonOperation as C,
    CompileTimeUnaryOperation as U, state as plan,
};
use nocter_declarations::value_access::Program;
use nocter_model::{
    AccessCodeId, AccessExpressionId, AccessFlowPointId, AccessPlaceId, FieldPath, TypeId,
    ValueAccessMode, ValueInputOperation,
};
use std::collections::BTreeMap;
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, flow::IntegrityError>;
use super::Types;
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Key {
    Resource(Holder),
    EnvironmentBacking,
    Operand(AccessExpressionId, usize),
    Predicate(AccessExpressionId),
}
struct Builder<'a, 'b> {
    query: &'a flow::Query,
    program: &'a Program,
    types: &'b mut Types,
    slots: Vec<plan::Slot>,
    keys: BTreeMap<Key, usize>,
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
        let data = self.types.get(self.program, data)?;
        if let Some(&slot) = self.keys.get(&key) {
            if self.slots.get(slot) != Some(&plan::Slot { data, storage }) {
                return Err(INVALID);
            }
            return Ok(slot);
        }
        let slot = self.slots.len();
        self.slots.push(plan::Slot { data, storage });
        self.keys.insert(key, slot);
        Ok(slot)
    }
    fn holder(&mut self, holder: Holder) -> Result<usize> {
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
        self.slot(Key::Resource(holder), data, storage)
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
            Step::Construct { destination, .. } | Step::Receive { destination, .. } => {
                plan::Effect::Construct(self.holder(*destination)?)
            }
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
                if !self.program.function_adapter(from.data(), payload.data()) {
                    return Err(INVALID);
                }
                plan::Effect::AdaptFunction {
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
                    plan::Operation::Alias {
                        destination: self.holder(Holder::Expression(result))?,
                        source: self.value(source)?,
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
                    plan::Operation::Alias {
                        destination: self.holder(Holder::Local(local))?,
                        source: self.value(value)?,
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
            flow::Operation::IndexFailure(_) => plan::Operation::BoundsTrap,
            flow::Operation::BeginEvaluation(_)
            | flow::Operation::EndEvaluation(_)
            | flow::Operation::Assign { .. }
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
    fn captured_call(&self, id: AccessExpressionId) -> Result<bool> {
        let ExpressionKind::Call { target, kind, .. } = &self
            .query
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .kind
        else {
            return Err(INVALID);
        };
        Ok(
            matches!(target, nocter_model::ValueFunctionOrigin::Closure(_))
                && *kind == super::super::CallKind::Value,
        )
    }
    fn call_input_offset(&self, call: AccessExpressionId) -> Result<usize> {
        let ExpressionKind::Call {
            kind,
            target,
            ref arguments,
        } = self
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
        match target {
            nocter_model::ValueFunctionOrigin::Closure(_)
                if kind == super::super::CallKind::Value =>
            {
                Ok(0)
            }
            nocter_model::ValueFunctionOrigin::Declaration(_) => {
                Ok(arguments.len() - kind.explicit_arguments(arguments).ok_or(INVALID)?.len())
            }
            _ => Err(INVALID),
        }
    }
    fn call(&mut self, id: AccessExpressionId) -> Result<plan::Operation> {
        let ExpressionKind::Call {
            target,
            arguments,
            kind,
        } = &self
            .query
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .kind
        else {
            return Err(INVALID);
        };
        let (target, captured) = match *target {
            nocter_model::ValueFunctionOrigin::Declaration(code) => {
                (nocter_model::ValueCode::Declaration(code), false)
            }
            nocter_model::ValueFunctionOrigin::Closure(code)
                if *kind == super::super::CallKind::Value =>
            {
                (nocter_model::ValueCode::Closure(code), true)
            }
            _ => return Err(INVALID),
        };
        let arguments = arguments.clone();
        let mut inputs = Vec::new();
        let offset = self.call_input_offset(id)?;
        for (index, argument) in arguments.iter().enumerate().skip(offset) {
            if argument.operation == ValueInputOperation::Take {
                let data = if captured && index == 0 {
                    let nocter_model::ValueCode::Closure(code) = target else {
                        return Err(INVALID);
                    };
                    self.program.closure(code).ok_or(INVALID)?.environment()
                } else {
                    self.data(argument.value)?
                };
                inputs.push(plan::Input::Take(self.types.get(self.program, data)?));
                continue;
            }
            let mut place = match &argument.action {
                ArgumentAction::ReserveRead(place) | ArgumentAction::ReserveUpdate(place) => {
                    self.referent(*place)?
                }
                ArgumentAction::ReserveTransfer { .. }
                | ArgumentAction::AccessOwnedValue
                | ArgumentAction::CopyPlace(_)
                | ArgumentAction::TakeValue => {
                    plan::Place::root(self.holder(Holder::Temporary { call: id, index })?)
                }
                ArgumentAction::PendingValue(_)
                | ArgumentAction::Unproduced
                | ArgumentAction::Interrupted => return Err(INVALID),
            };
            if captured && index == 0 {
                place.fields = place
                    .fields
                    .iter()
                    .copied()
                    .chain([plan::Projection::Field(0)])
                    .collect();
            }
            inputs.push(match argument.operation {
                ValueInputOperation::Read => plan::Input::Read(place),
                ValueInputOperation::Edit => plan::Input::Edit(place),
                ValueInputOperation::Take => unreachable!(),
            });
        }
        Ok(plan::Operation::Call {
            destination: self.holder(Holder::Expression(id))?,
            target: target.into(),
            inputs: inputs.into(),
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
        let destination = self.holder(Holder::Expression(id))?;
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
                action: action @ (SelectionAction::CopyPayload | SelectionAction::Access),
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
            SelectionAction::Access => plan::Operation::Alias {
                destination,
                source,
            },
            SelectionAction::CopyPayload => plan::Operation::Copy {
                destination: plan::Place::root(destination),
                source,
            },
            SelectionAction::MovePayload => return Err(INVALID),
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
    fn point(&mut self, id: AccessFlowPointId, point: &flow::Point) -> Result<plan::Point> {
        let reachable = self.query.initialization().reachable(id);
        let mut actions = Vec::new();
        if reachable {
            for (index, action) in point.actions.iter().enumerate() {
                let location = flow::Location {
                    point: id,
                    action: index,
                };
                let mut alternatives = Vec::new();
                for transition in self.query.access().resources().at(location) {
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
                    alternatives.push(plan::Alternative {
                        owned,
                        before: transition
                            .before_steps()
                            .iter()
                            .map(|step| self.effect(step))
                            .collect::<Result<_>>()?,
                        after: transition
                            .after_steps()
                            .iter()
                            .map(|step| self.effect(step))
                            .collect::<Result<_>>()?,
                    });
                }
                actions.push(plan::Action {
                    operation: self.computation(&action.operation)?,
                    alternatives: alternatives.into(),
                });
            }
        }
        let terminator = if reachable {
            match point.terminator {
                flow::Terminator::Jump(next) => plan::Terminator::Jump(next),
                flow::Terminator::Branch { condition, yes, no } => plan::Terminator::Branch {
                    condition: self.aux(Key::Predicate(condition), condition)?,
                    yes,
                    no,
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
        let mut points = nocter_model::ArenaBuilder::new();
        for (id, point) in self.query.graph().points().iter() {
            if points.insert(self.point(id, point)?) != id {
                return Err(INVALID);
            }
        }
        let statics = self.static_views();
        let environment_backing = self.environment_backing()?;
        Ok(plan::Plan {
            environment_backing,
            statics,
            inputs,
            slots: self.slots.into(),
            result: self
                .types
                .get(self.program, self.query.typed().result_data())?,
            initializer: match self.query.typed().code() {
                nocter_model::ValueCode::Declaration(id) => self
                    .program
                    .codes()
                    .get(id)
                    .ok_or(INVALID)?
                    .kind()
                    .is_initializer(),
                nocter_model::ValueCode::Expression(_) => true,
                nocter_model::ValueCode::Closure(_) => false,
            },
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
pub(super) fn prepare(
    checked: &super::super::accepted::Checked,
    root: nocter_model::ValueCodeRoot,
    values: impl Fn(AccessCodeId) -> Option<plan::Value>,
) -> Result<plan::Program> {
    let program = &checked.program;
    let mut pending = vec![root.code()];
    let mut types = Types::default();
    let mut plans = BTreeMap::new();
    while let Some(id) = pending.pop() {
        if plans.contains_key(&id.into()) {
            continue;
        }
        let query = checked.code(id).ok_or(INVALID)?;
        let plan = Builder {
            query,
            program,
            types: &mut types,
            slots: Vec::new(),
            keys: BTreeMap::new(),
        }
        .build()?;
        for (_, point) in plan.points.iter() {
            for action in &point.actions {
                if let plan::Operation::Call { target, .. } = action.operation {
                    pending.push(target.source);
                }
            }
        }
        plans.insert(id.into(), plan);
    }
    let mut statics = BTreeMap::new();
    for plan in plans.values() {
        for &(_, id) in &plan.statics {
            if statics.contains_key(&id) {
                continue;
            }
            let data = program.static_declaration(id).ok_or(INVALID)?.result();
            let value = values(id.initializer).ok_or(INVALID)?;
            statics.insert(
                id,
                plan::Static {
                    data: types.get(program, data)?,
                    value,
                },
            );
        }
    }
    plan::Program::new(types.finish(), plans, statics).map_err(|_| INVALID)
}

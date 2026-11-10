//! Project one already checked graph and its resource alternatives; no source/ownership decisions.
use super::super::{
    ArgumentAction, Binary, BoundaryAction, ExpressionKind, SelectionAction, Statement, flow,
};
use flow::resources::{Holder, Step};
use nocter_constant_evaluation::{
    CompileTimeBinaryOperation as B, CompileTimeComparisonOperation as C,
    CompileTimeUnaryOperation as U, state as plan,
};
use nocter_declarations::value_access::{CallableKind, PayloadSchema, Program};
use nocter_model::{
    AccessCallableId, AccessExpressionId, AccessPlaceId, FieldPath, TypeId, ValueAccessMode,
    ValueInputOperation,
};
use std::collections::BTreeMap;
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, flow::IntegrityError>;
#[derive(Default)]
struct Types {
    values: Vec<plan::Type>,
    ids: BTreeMap<TypeId, usize>,
}
impl Types {
    fn get(&mut self, program: &Program, data: TypeId) -> Result<usize> {
        if let Some(&id) = self.ids.get(&data) {
            return Ok(id);
        }
        let mut pending = vec![(data, false)];
        while let Some((data, expanded)) = pending.pop() {
            if self.ids.contains_key(&data) {
                continue;
            }
            let value = match program.data_schema(data).ok_or(INVALID)? {
                PayloadSchema::Builtin(builtin) => plan::Type::builtin(builtin).ok_or(INVALID)?,
                PayloadSchema::Record(record) => {
                    let children = record
                        .fields()
                        .iter()
                        .map(|field| {
                            program
                                .fields()
                                .get(*field)
                                .map(|field| field.data())
                                .ok_or(INVALID)
                        })
                        .collect::<Result<Vec<_>>>()?;
                    if !expanded {
                        pending.push((data, true));
                        pending.extend(children.into_iter().rev().map(|child| (child, false)));
                        continue;
                    }
                    plan::Type::Record(
                        children
                            .into_iter()
                            .map(|child| self.ids.get(&child).copied().ok_or(INVALID))
                            .collect::<Result<_>>()?,
                    )
                }
                PayloadSchema::Function(_) => return Err(INVALID),
            };
            let id = self.values.len();
            self.values.push(value);
            self.ids.insert(data, id);
        }
        self.ids.get(&data).copied().ok_or(INVALID)
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Key {
    Resource(Holder),
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
            Holder::Return => (self.query.typed().result_data(), plan::Storage::Owned),
        };
        self.slot(Key::Resource(holder), data, storage)
    }
    fn fields(&self, fields: &FieldPath) -> Result<Box<[usize]>> {
        fields
            .fields()
            .iter()
            .map(|id| Ok(self.program.fields().get(*id).ok_or(INVALID)?.position()))
            .collect()
    }
    fn resource(&mut self, place: &flow::resources::Place) -> Result<plan::Place> {
        Ok(plan::Place {
            slot: self.holder(place.holder())?,
            fields: self.fields(place.fields())?,
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
            Step::Dispose { source, .. } => plan::Effect::Dispose(self.resource(source)?),
            Step::ReleaseContainer { source, .. } => plan::Effect::Release(self.resource(source)?),
            Step::Invoke { source, .. } => plan::Effect::Invoke(self.resource(source)?),
            Step::ReplaceInput {
                local,
                fields,
                source,
                ..
            } => plan::Effect::Replace {
                source: self.resource(source)?,
                destination: plan::Place {
                    slot: self.holder(Holder::Local(*local))?,
                    fields: self.fields(fields)?,
                },
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
                        Ok((
                            self.program.fields().get(*field).ok_or(INVALID)?.position(),
                            self.resource(source)?,
                        ))
                    })
                    .collect::<Result<_>>()?,
            },
            Step::Capture { .. } | Step::AdaptFunction { .. } => return Err(INVALID),
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
                value: plan::Value::Bool(value),
            },
            flow::Operation::BooleanAnswer { source, result } => plan::Operation::Copy {
                source: self.value(source)?,
                destination: plan::Place::root(self.holder(Holder::Expression(result))?),
            },
            flow::Operation::Forward { source, result } => {
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
                binary(
                    *operation,
                    self.selected(target)?,
                    self.selected(target)?,
                    self.value(value)?,
                )?
            }
            flow::Operation::PrepareArgument {
                call,
                argument: index,
            } => return self.argument(call, index),
            flow::Operation::CommitCall(id) => return self.call(id),
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
                    source: self.selected(place)?,
                    destination: plan::Place::root(self.holder(Holder::Prepared { call, index })?),
                }
            } else {
                plan::Operation::None
            },
        )
    }
    fn call(&mut self, id: AccessExpressionId) -> Result<plan::Operation> {
        let ExpressionKind::Call {
            target: nocter_model::ValueFunctionOrigin::Declaration(target),
            arguments,
            ..
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
        let target = *target;
        let arguments = arguments.clone();
        let mut inputs = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let place = match &argument.action {
                ArgumentAction::ReserveRead(place) | ArgumentAction::ReserveUpdate(place) => {
                    self.selected(*place)?
                }
                ArgumentAction::ReserveTransfer { place, adapters }
                    if adapters.is_empty() && argument.operation == ValueInputOperation::Take =>
                {
                    self.selected(*place)?
                }
                ArgumentAction::ReserveTransfer { adapters, .. } if !adapters.is_empty() => {
                    return Err(INVALID);
                }
                ArgumentAction::ReserveTransfer { .. }
                | ArgumentAction::AccessOwnedValue
                | ArgumentAction::CopyPlace(_)
                | ArgumentAction::TakeValue => plan::Place::root(self.holder(
                    if argument.operation == ValueInputOperation::Take {
                        Holder::Prepared { call: id, index }
                    } else {
                        Holder::Temporary { call: id, index }
                    },
                )?),
                ArgumentAction::PendingValue(_)
                | ArgumentAction::Unproduced
                | ArgumentAction::Interrupted => return Err(INVALID),
            };
            inputs.push(plan::Input {
                operation: argument.operation,
                place,
            });
        }
        Ok(plan::Operation::Call {
            destination: self.holder(Holder::Expression(id))?,
            target,
            inputs: inputs.into(),
        })
    }
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
            ExpressionKind::Integer(v) => plan::Operation::Literal {
                destination,
                value: plan::Value::I32(v),
            },
            ExpressionKind::Boolean(v) => plan::Operation::Literal {
                destination,
                value: plan::Value::Bool(v),
            },
            ExpressionKind::Unit => plan::Operation::Literal {
                destination,
                value: plan::Value::Unit,
            },
            ExpressionKind::Place(place) => plan::Operation::Alias {
                destination,
                source: self.selected(place)?,
            },
            ExpressionKind::Boundary {
                source,
                action: BoundaryAction::ReadPlace | BoundaryAction::DelegateUpdate,
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
            | ExpressionKind::Record { .. }
            | ExpressionKind::Select {
                action: SelectionAction::MovePayload,
                ..
            } => plan::Operation::None,
            ExpressionKind::Select {
                source,
                fields,
                action: SelectionAction::CopyPayload,
            } => {
                let mut source = self.value(source)?;
                let mut path = source.fields.to_vec();
                path.extend(self.fields(&fields)?);
                source.fields = path.into();
                plan::Operation::Copy {
                    destination: plan::Place::root(destination),
                    source,
                }
            }
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
    fn build(mut self) -> Result<plan::Plan> {
        let inputs = self
            .query
            .typed()
            .body()
            .inputs()
            .explicit()
            .iter()
            .map(|i| Ok((self.holder(Holder::Local(i.local))?, i.operation)))
            .collect::<Result<_>>()?;
        let mut points = nocter_model::ArenaBuilder::new();
        for (id, point) in self.query.graph().points().iter() {
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
                                        .map(|path| self.fields(path))
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
            if points.insert(plan::Point {
                actions: actions.into(),
                terminator,
            }) != id
            {
                return Err(INVALID);
            }
        }
        Ok(plan::Plan {
            inputs,
            slots: self.slots.into(),
            result: self
                .types
                .get(self.program, self.query.typed().result_data())?,
            initializer: self
                .program
                .callables()
                .get(self.query.typed().source_owner())
                .ok_or(INVALID)?
                .kind()
                == CallableKind::ConstantInitializer,
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
            value: plan::Value::Bool(value),
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
    program: &Program,
    bodies: &BTreeMap<AccessCallableId, flow::Query>,
    root: AccessCallableId,
) -> Result<plan::Program> {
    let mut pending = vec![root];
    let mut types = Types::default();
    let mut plans = BTreeMap::new();
    while let Some(id) = pending.pop() {
        if plans.contains_key(&id) {
            continue;
        }
        let query = bodies.get(&id).ok_or(INVALID)?;
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
                    pending.push(target);
                }
            }
        }
        plans.insert(id, plan);
    }
    plan::Program::new(types.values, plans).map_err(|_| INVALID)
}

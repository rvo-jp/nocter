//! Lower selected operations once; no source lookup, type inference or ownership selection.
use super::lower::Builder;
use super::{
    Call, CallInput, Constant, Effect, Failure, Operation, Place, Projection, SlotKey, Storage,
};
use crate::{
    MirArithmeticCheck as Check, MirBinaryOperation as Binary, MirUnaryOperation as Unary,
};
use nocter_checking::value_access::flow::resources::Holder;
use nocter_checking::value_access::{
    ArgumentAction, BoundaryAction, ExpressionKind, Referent, SelectionAction, Statement, flow,
};
use nocter_model::{AccessExpressionId, ValueAccessMode};
use nocter_target_program::value_access::{Environment, storage};

impl Builder<'_> {
    fn value(&mut self, id: AccessExpressionId) -> Result<Place, Failure> {
        let item = self
            .checked
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or_else(|| self.invalid())?;
        if let ExpressionKind::Place(place) = item.kind {
            self.selected(place, self.expression(id)?)
        } else {
            self.root(Holder::Expression(id), self.expression(id)?, false)
        }
    }
    pub(super) fn referent(
        &mut self,
        source: Referent,
        data: nocter_model::TypeId,
    ) -> Result<Place, Failure> {
        match source {
            Referent::Place(place) => self.selected(place, data),
            Referent::Evaluated(id)
                if self.checked.typed().body().referent_data(source).is_some() =>
            {
                let place = self.value(id)?;
                if place.data != data {
                    return Err(self.invalid());
                }
                Ok(place)
            }
            Referent::Evaluated(_) => Err(self.invalid()),
        }
    }
    fn result(&mut self, id: AccessExpressionId) -> Result<Place, Failure> {
        self.root(Holder::Expression(id), self.expression(id)?, false)
    }
    fn snapshot(&mut self, key: SlotKey, data: nocter_model::TypeId) -> Result<Place, Failure> {
        let slot = self.slot(key, data, Storage::Payload)?;
        Ok(Place {
            slot,
            root: data,
            data,
            projections: Box::new([]),
        })
    }
    pub(super) fn predicate(&mut self, value: AccessExpressionId) -> Result<Place, Failure> {
        self.snapshot(SlotKey::Predicate(value), self.expression(value)?)
    }
    fn operand(
        &mut self,
        id: AccessExpressionId,
        index: usize,
        value: AccessExpressionId,
    ) -> Result<Place, Failure> {
        self.snapshot(
            SlotKey::Operand {
                expression: id,
                index,
            },
            self.expression(value)?,
        )
    }
    fn alias(source: Place, destination: nocter_model::MirLocalId) -> Operation {
        Operation::Storage(Effect::BindAddress {
            destination,
            source,
        })
    }
    fn evaluate(&mut self, id: AccessExpressionId) -> Result<Vec<Operation>, Failure> {
        let kind = self
            .checked
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or_else(|| self.invalid())?
            .kind
            .clone();
        let destination = self.result(id)?;
        let operation = match kind {
            ExpressionKind::Constant(value) => Operation::Frozen { destination, value },
            ExpressionKind::Integer(value) => Operation::Constant {
                destination,
                value: Constant::Integer(value),
            },
            ExpressionKind::Boolean(value) => Operation::Constant {
                destination,
                value: Constant::Boolean(value),
            },
            ExpressionKind::Unit | ExpressionKind::Function(_) => Operation::Constant {
                destination,
                value: Constant::Zero,
            },
            ExpressionKind::Index { base, .. } => {
                // Runtime array geometry is gated by the storage owner before MIR. Keep an
                // explicit unsupported-shape failure if this operation reaches lowering.
                return Err(Failure::Storage(storage::Failure::UnsupportedArray(
                    self.expression(base)?,
                )));
            }
            ExpressionKind::Place(place) => {
                let source = self.selected(place, destination.data)?;
                Self::alias(source, destination.slot)
            }
            ExpressionKind::Boundary { source, action } => match action {
                BoundaryAction::ReadPlace | BoundaryAction::DelegateUpdate => {
                    let source = self.value(source)?;
                    Self::alias(source, destination.slot)
                }
                BoundaryAction::CopyPayload => Operation::Copy {
                    source: self.value(source)?,
                    destination,
                },
                BoundaryAction::TakeValue => return Ok(Vec::new()),
                BoundaryAction::PendingValue | BoundaryAction::Unproduced => {
                    return Err(self.invalid());
                }
            },
            ExpressionKind::Unary { operation, operand } => {
                let operand = self.operand(id, 0, operand)?;
                let (operation, check) = match operation {
                    nocter_checking::value_access::Unary::Not => {
                        (Unary::LogicalNot, Check::NotRequired)
                    }
                    nocter_checking::value_access::Unary::CheckedNegate => {
                        (Unary::Negate, Check::Required)
                    }
                };
                Operation::Unary {
                    operation,
                    operand,
                    destination,
                    check,
                }
            }
            ExpressionKind::Binary {
                operation,
                left,
                right,
            } => {
                let left = self.operand(id, 0, left)?;
                let right = self.operand(id, 1, right)?;
                binary(operation, left, right, destination).ok_or_else(|| self.invalid())?
            }
            ExpressionKind::Select {
                source,
                fields,
                action: SelectionAction::CopyPayload,
            } => {
                let mut source = self.value(source)?;
                let mut projections = source.projections.to_vec();
                projections.extend(fields.fields().iter().copied().map(Projection::Field));
                source.projections = projections.into();
                source.data = destination.data;
                Operation::Copy {
                    source,
                    destination,
                }
            }
            ExpressionKind::Move(_)
            | ExpressionKind::Closure { .. }
            | ExpressionKind::AdaptFunction { .. }
            | ExpressionKind::Aggregate { .. }
            | ExpressionKind::Select {
                action: SelectionAction::MovePayload,
                ..
            } => return Ok(Vec::new()),
            ExpressionKind::If { .. } | ExpressionKind::Call { .. } => return Err(self.invalid()),
        };
        Ok(vec![operation])
    }
    fn call(&mut self, id: AccessExpressionId) -> Result<Call, Failure> {
        let selected = self
            .instance
            .calls()
            .get(&id)
            .cloned()
            .ok_or_else(|| self.invalid())?;
        let mut inputs = Vec::new();
        if let Environment::Capture {
            argument,
            input,
            payload,
            ..
        } = selected.environment()
        {
            let mut source = self.argument(id, argument)?;
            source.projections = Box::new([Projection::Environment]);
            source.data = payload.data();
            inputs.push(CallInput {
                source,
                operation: input.operation,
            });
        }
        for binding in selected.inputs() {
            inputs.push(CallInput {
                source: self.argument(id, binding.argument)?,
                operation: binding.input.operation,
            });
        }
        let data = selected.result().data();
        let answer = self.snapshot(SlotKey::Answer(id), data)?;
        Ok(Call {
            target: selected.target(),
            inputs: inputs.into(),
            answer,
        })
    }
    fn prepare_argument(
        &mut self,
        call: AccessExpressionId,
        index: usize,
    ) -> Result<Option<Operation>, Failure> {
        let argument = self
            .arguments(call)?
            .get(index)
            .cloned()
            .ok_or_else(|| self.invalid())?;
        match argument.action {
            ArgumentAction::CopyPlace(place) => {
                let data = self.expression(argument.value)?;
                let source = self.referent(place, data)?;
                let destination = self.root(Holder::Prepared { call, index }, data, false)?;
                Ok(Some(Operation::Copy {
                    source,
                    destination,
                }))
            }
            ArgumentAction::ReserveRead(_)
            | ArgumentAction::ReserveUpdate(_)
            | ArgumentAction::ReserveTransfer { .. }
            | ArgumentAction::AccessOwnedValue
            | ArgumentAction::TakeValue => Ok(None),
            ArgumentAction::PendingValue(_)
            | ArgumentAction::Unproduced
            | ArgumentAction::Interrupted => Err(self.invalid()),
        }
    }
    pub(super) fn computation(
        &mut self,
        operation: &flow::Operation,
    ) -> Result<Box<[Operation]>, Failure> {
        let operation = match *operation {
            flow::Operation::Evaluate(id) => return self.evaluate(id).map(Vec::into_boxed_slice),
            flow::Operation::PrepareOperand {
                expression,
                operand,
                value,
            } => Operation::Copy {
                source: self.value(value)?,
                destination: self.operand(expression, operand, value)?,
            },
            flow::Operation::Condition(value) => Operation::Copy {
                source: self.value(value)?,
                destination: self.predicate(value)?,
            },
            flow::Operation::BooleanConstant { result, value } => Operation::Constant {
                destination: self.result(result)?,
                value: Constant::Boolean(value),
            },
            flow::Operation::BooleanAnswer { source, result } => Operation::Copy {
                source: self.value(source)?,
                destination: self.result(result)?,
            },
            flow::Operation::Forward { source, result } => {
                let destination = self.result(result)?;
                let mode = self
                    .checked
                    .typed()
                    .body()
                    .expressions()
                    .get(result)
                    .and_then(|e| e.ty.normal())
                    .ok_or_else(|| self.invalid())?
                    .mode();
                if mode == ValueAccessMode::Own {
                    return Ok(Box::new([]));
                }
                let source = self.value(source)?;
                Self::alias(source, destination.slot)
            }
            flow::Operation::PrepareArgument { call, argument } => {
                return self
                    .prepare_argument(call, argument)
                    .map(|operation| operation.into_iter().collect());
            }
            flow::Operation::CommitCall(id) => Operation::Call(self.call(id)?),
            flow::Operation::Bind { local, value } => {
                let item = self
                    .checked
                    .typed()
                    .body()
                    .locals()
                    .get(local)
                    .ok_or_else(|| self.invalid())?;
                if item.access().mode() == ValueAccessMode::Own {
                    return Ok(Box::new([]));
                }
                let data = self.data(item.data)?;
                let destination = self.root(Holder::Local(local), data, false)?;
                let source = self.value(value)?;
                Self::alias(source, destination.slot)
            }
            flow::Operation::CompoundAssign {
                statement,
                target,
                value,
            } => {
                let Some(Statement::CompoundAssign { operation, .. }) =
                    self.checked.typed().body().statements().get(statement)
                else {
                    return Err(self.invalid());
                };
                let operation = *operation;
                let data = self.expression(value)?;
                let left = self.selected(target, data)?;
                let right = self.value(value)?;
                binary(operation, left.clone(), right, left).ok_or_else(|| self.invalid())?
            }
            flow::Operation::BeginEvaluation(_)
            | flow::Operation::EndEvaluation(_)
            | flow::Operation::CancelCall { .. }
            | flow::Operation::Assign { .. }
            | flow::Operation::Discard(_)
            | flow::Operation::Drop(_)
            | flow::Operation::StageReturn(_)
            | flow::Operation::ExitScope { .. } => return Ok(Box::new([])),
        };
        Ok(Box::new([operation]))
    }
}

fn binary(
    operation: nocter_checking::value_access::Binary,
    mut left: Place,
    mut right: Place,
    destination: Place,
) -> Option<Operation> {
    use nocter_checking::value_access::Binary as Source;
    let (operation, invert, check) = match operation {
        Source::CheckedAdd => (Binary::Add, false, Check::Required),
        Source::CheckedSubtract => (Binary::Subtract, false, Check::Required),
        Source::CheckedMultiply => (Binary::Multiply, false, Check::Required),
        Source::CheckedDivide => (Binary::Divide, false, Check::Required),
        Source::CheckedRemainder => (Binary::Remainder, false, Check::NonZeroDivisor),
        Source::UnitEqual | Source::UnitNotEqual => {
            return Some(Operation::Constant {
                destination,
                value: Constant::Boolean(operation.fixed_boolean_result()?),
            });
        }
        Source::Equal => (Binary::Equal, false, Check::NotRequired),
        Source::NotEqual => (Binary::Equal, true, Check::NotRequired),
        Source::Less => (Binary::Less, false, Check::NotRequired),
        Source::Greater => {
            std::mem::swap(&mut left, &mut right);
            (Binary::Less, false, Check::NotRequired)
        }
        Source::LessEqual => {
            std::mem::swap(&mut left, &mut right);
            (Binary::Less, true, Check::NotRequired)
        }
        Source::GreaterEqual => (Binary::Less, true, Check::NotRequired),
        Source::And | Source::Or => return None,
    };
    Some(Operation::Binary {
        operation,
        left,
        right,
        destination,
        invert,
        check,
    })
}

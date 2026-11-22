//! Lower selected operations once; no source lookup, type inference or ownership selection.
use super::lower::Builder;
use super::{
    Call, CallInput, Constant, Effect, Failure, Operation, Place, Projection, SlotKey, Storage,
};
use crate::{
    MirArithmeticCheck as Check, MirBinaryOperation as Binary, MirUnaryOperation as Unary,
};
use nocter_checking::value_access::ValueSourceSlot;
use nocter_checking::value_access::flow::resources::Holder;
use nocter_checking::value_access::{
    ArgumentAction, BoundaryAction, ExpressionKind, Referent, SelectionAction, Statement, flow,
};
use nocter_model::{AccessExpressionId, ValueAccessMode};
use nocter_target_program::value_access::Environment;

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
        data: nocter_runtime_contract::RuntimeDataId,
    ) -> Result<Place, Failure> {
        match source {
            Referent::Place(place) => self.selected(place, data),
            Referent::Evaluated(id) => {
                let raw = self
                    .checked
                    .typed()
                    .body()
                    .referent_data(source)
                    .ok_or_else(|| self.invalid())?;
                // The caller already selected this original referent's concrete data. At a
                // later invocation commit the expression is not a freshly evaluated operand;
                // querying its raw type again would discard a contextual callable witness.
                // Independent shapes also serve compiler-retained index references that have
                // no new source-value occurrence at this resource phase.
                let actual = self
                    .closed_data(raw)
                    .or_else(|_| self.referent_root(source))?;
                if actual != data {
                    return Err(self.invalid());
                }
                self.root(Holder::Expression(id), data, false)
            }
        }
    }
    fn result(&mut self, id: AccessExpressionId) -> Result<Place, Failure> {
        self.root(Holder::Expression(id), self.receiving_data(id)?, false)
    }
    pub(super) fn snapshot(
        &mut self,
        key: SlotKey,
        data: nocter_runtime_contract::RuntimeDataId,
    ) -> Result<Place, Failure> {
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
    #[allow(clippy::too_many_lines)]
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
        if self.local_future_expression(id) {
            return match kind {
                ExpressionKind::Move(_)
                | ExpressionKind::Place(_)
                | ExpressionKind::Boundary {
                    action: BoundaryAction::TakeValue,
                    ..
                } => Ok(Vec::new()),
                _ => Err(self.invalid()),
            };
        }
        let destination = self.result(id)?;
        let operation = match kind {
            ExpressionKind::PrimaryCondition { source } => {
                let data = self.referent_data(source)?;
                Operation::PrimaryCondition {
                    source: self.referent(source, data)?,
                    destination,
                }
            }
            ExpressionKind::GuardView { source, primary } => {
                let source = self.referent(source, destination.data)?;
                return Ok(vec![
                    Operation::RequireVariant {
                        source: source.clone(),
                        primary,
                    },
                    Operation::Storage(Effect::BindAddress {
                        destination: destination.slot,
                        source,
                    }),
                ]);
            }
            ExpressionKind::RequirePrimary { source } => Operation::RequireVariant {
                source: self.value(source)?,
                primary: true,
            },
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
            ExpressionKind::Index {
                base,
                index,
                action,
                bounds,
            } => Operation::Index {
                base: self.value(base)?,
                index: self.value(index)?,
                destination,
                copy: action == nocter_checking::value_access::IndexAction::CopyPayload,
                bounds,
            },
            ExpressionKind::Repeat { seed } => return self.repetition(seed, destination),
            ExpressionKind::Place(place) => {
                let source = self.selected(place, destination.data)?;
                Self::alias(source, destination.slot)
            }
            ExpressionKind::Boundary { source, action } => match action {
                BoundaryAction::ReadPlace
                | BoundaryAction::DelegateUpdate
                | BoundaryAction::TemporaryPlace => {
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
                action: action @ (SelectionAction::CopyPayload | SelectionAction::Access),
            } => self.selected_component(source, &fields, destination, action)?,
            ExpressionKind::Move(_)
            | ExpressionKind::Closure { .. }
            | ExpressionKind::AdaptFunction { .. }
            | ExpressionKind::Aggregate { .. }
            | ExpressionKind::Select {
                action: SelectionAction::MovePayload,
                ..
            } => return Ok(Vec::new()),
            ExpressionKind::Scoped { .. }
            | ExpressionKind::Block { .. }
            | ExpressionKind::If { .. }
            | ExpressionKind::Call { .. }
            | ExpressionKind::Await { .. } => return Err(self.invalid()),
        };
        Ok(vec![operation])
    }
    fn repetition(
        &mut self,
        seed: AccessExpressionId,
        destination: Place,
    ) -> Result<Vec<Operation>, Failure> {
        let Some((_, length)) = self
            .runtime
            .types()
            .get(destination.data)
            .and_then(nocter_runtime_contract::RuntimeType::array)
        else {
            return Err(self.invalid());
        };
        if length <= 1 {
            // The ordinary resource handoff disposes or assembles the one seed.
            return Ok(Vec::new());
        }
        Ok(vec![Operation::Repeat {
            source: self.value(seed)?,
            destination,
        }])
    }
    fn selected_component(
        &mut self,
        source: AccessExpressionId,
        fields: &nocter_model::FieldPath,
        destination: Place,
        action: SelectionAction,
    ) -> Result<Operation, Failure> {
        let source = self.component(source, fields, destination.data)?;
        Ok(if action == SelectionAction::Access {
            Self::alias(source, destination.slot)
        } else {
            Operation::Copy {
                source,
                destination,
            }
        })
    }
    fn component(
        &mut self,
        source: AccessExpressionId,
        fields: &nocter_model::FieldPath,
        data: nocter_runtime_contract::RuntimeDataId,
    ) -> Result<Place, Failure> {
        let mut source = self.value(source)?;
        let mut projections = source.projections.to_vec();
        projections.extend(fields.fields().iter().copied().map(Projection::Field));
        source.projections = projections.into();
        source.data = data;
        Ok(source)
    }
    fn call(&mut self, id: AccessExpressionId) -> Result<Operation, Failure> {
        let selected = self
            .instance
            .calls()
            .get(&id)
            .cloned()
            .ok_or_else(|| self.invalid())?;
        let mut inputs = Vec::new();
        if let Environment::Capture {
            argument, input, ..
        } = selected.environment()
        {
            let mut source = self.argument(id, argument)?;
            source.projections = Box::new([Projection::Environment]);
            source.data = self.capture_data(source.data)?;
            inputs.push(CallInput {
                source,
                operation: input.operation,
            });
        }
        for binding in selected.inputs() {
            inputs.push(CallInput {
                source: self.argument(id, binding.argument)?,
                operation: binding.operation,
            });
        }
        let data = self.expression(id)?;
        let answer = self.snapshot(SlotKey::Answer(id), data)?;
        if selected.target() == super::CallTarget::Selected {
            let carrier = self.argument(id, 0)?;
            let cases = selected
                .cases()
                .iter()
                .map(|case| {
                    let super::CallTarget::Body(target) = case.target else {
                        return Err(self.invalid());
                    };
                    let data = self.case_data(case)?;
                    Ok(super::SelectedCallee { data, target })
                })
                .collect::<Result<Box<[_]>, Failure>>()?;
            Ok(Operation::SelectedCall(super::SelectedCall {
                carrier,
                cases,
                inputs: inputs.into(),
                answer,
            }))
        } else {
            Ok(Operation::Call(Call {
                target: selected.target(),
                inputs: inputs.into(),
                answer,
            }))
        }
    }
    fn drive(
        &mut self,
        future: AccessExpressionId,
        result: AccessExpressionId,
    ) -> Result<Operation, Failure> {
        use nocter_target_program::value_access::Drive;
        let target = self.drive_target()?.clone();
        let data = self.expression(result)?;
        let answer = self.snapshot(SlotKey::Answer(result), data)?;
        let source = Holder::Expression(future);
        let inputs = self
            .future_input_types(source)?
            .iter()
            .copied()
            .enumerate()
            .map(|(index, data)| {
                Ok(CallInput {
                    source: self.future_input(source, index, data)?,
                    operation: nocter_model::ValueInputOperation::Take,
                })
            })
            .collect::<Result<Box<[_]>, Failure>>()?;
        Ok(match target {
            Drive::Direct(child) => Operation::Call(Call {
                target: super::CallTarget::Body(child),
                inputs,
                answer,
            }),
            Drive::Tagged(cases) => Operation::SelectedDrive(super::SelectedDrive {
                selector: self.future_selector(source)?,
                cases,
                inputs,
                answer,
            }),
        })
    }
    fn construct_future(&mut self, id: AccessExpressionId) -> Result<Option<Operation>, Failure> {
        if !self.local_future_expression(id) {
            return Err(self.invalid());
        }
        self.instances
            .unit()
            .future_construction_tag(self.instance.key().code(), id)
            .map(|tag| {
                Ok(Operation::Constant {
                    destination: self.future_selector(Holder::Expression(id))?,
                    value: Constant::Integer(i128::from(tag)),
                })
            })
            .transpose()
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
            flow::Operation::IndexFailure(_) => Operation::BoundsTrap,
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
            flow::Operation::Forward { source, result, .. } => {
                if self.local_future_expression(result) {
                    return Ok(Box::new([]));
                }
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
            flow::Operation::CommitCall(id) => self.call(id)?,
            flow::Operation::DriveFuture { future, result } => self.drive(future, result)?,
            flow::Operation::ConstructFuture(id) => {
                return self
                    .construct_future(id)
                    .map(|operation| operation.into_iter().collect());
            }
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
                let data = self.definition(ValueSourceSlot::Local(local))?;
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
                let left = self.referent(target, data)?;
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

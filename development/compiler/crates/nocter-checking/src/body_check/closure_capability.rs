use std::collections::HashSet;

use nocter_model::{BodyNodeId, BorrowCapability, CallableCapability, PlaceId};

use crate::body_check::error::BodyCheckInternalError;
use crate::checked::CheckedBodyBuilder;
use crate::{
    AggregateConstruction, AllocationSelection, CallTarget, CheckedControl, CheckedOperation,
    CheckedOutcome, LoopKind, PlaceRoot, PrimitiveOperation, ReceiverPreparation,
};

/// Derives invocation authority from environment access in one closure execution root.
///
/// This traversal follows runtime operands but deliberately treats nested closure bodies as
/// separate roots. Their capture initializers remain ordinary operands of the enclosing root.
pub(super) fn infer(
    builder: &CheckedBodyBuilder,
    root: BodyNodeId,
) -> Result<CallableCapability, BodyCheckInternalError> {
    let mut capability = CallableCapability::Readonly;
    let mut visited = HashSet::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if !visited.insert(node) {
            continue;
        }
        let checked = builder
            .node(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))?;
        match checked.operation() {
            CheckedOperation::Move(place)
            | CheckedOperation::Control(CheckedControl::Drop(place)) => {
                raise_for_place(builder, *place, CallableCapability::Owned, &mut capability)?;
            }
            CheckedOperation::Borrow {
                capability: BorrowCapability::ReadWrite,
                place,
            } => {
                raise_for_place(
                    builder,
                    *place,
                    CallableCapability::ReadWrite,
                    &mut capability,
                )?;
            }
            CheckedOperation::Control(
                CheckedControl::Assign { target, .. }
                | CheckedControl::CompoundAssign { target, .. },
            ) => {
                raise_for_place(
                    builder,
                    *target,
                    CallableCapability::ReadWrite,
                    &mut capability,
                )?;
            }
            CheckedOperation::Call(call) => {
                if let CallTarget::CallableValue {
                    value,
                    capability: required,
                    ..
                }
                | CallTarget::ClosureValue {
                    value,
                    capability: required,
                    ..
                } = call.target()
                    && node_has_capture_root(builder, *value)?
                {
                    raise(&mut capability, *required);
                }
                if let Some(receiver) = call.receiver()
                    && matches!(
                        receiver.preparation(),
                        ReceiverPreparation::BorrowPlace(BorrowCapability::ReadWrite)
                            | ReceiverPreparation::BorrowTemporary(BorrowCapability::ReadWrite)
                            | ReceiverPreparation::PreserveBorrow(BorrowCapability::ReadWrite)
                    )
                    && node_has_capture_root(builder, receiver.value())?
                {
                    raise(&mut capability, CallableCapability::ReadWrite);
                }
            }
            CheckedOperation::Complete
            | CheckedOperation::Constant(_)
            | CheckedOperation::Place(_)
            | CheckedOperation::Copy(_)
            | CheckedOperation::Borrow {
                capability: BorrowCapability::Readonly,
                ..
            }
            | CheckedOperation::BorrowConversion(_)
            | CheckedOperation::CallableGuaranteeErasure(_)
            | CheckedOperation::OpaqueWitness(_)
            | CheckedOperation::Comparison(_)
            | CheckedOperation::Primitive(_)
            | CheckedOperation::Aggregate(_)
            | CheckedOperation::Outcome(_)
            | CheckedOperation::Closure(_)
            | CheckedOperation::ArgumentPackLength(_)
            | CheckedOperation::IteratorAcquisition(_)
            | CheckedOperation::PackLiteral(_)
            | CheckedOperation::StringLiteral { .. }
            | CheckedOperation::Interpolation(_)
            | CheckedOperation::Control(_) => {}
        }
        append_operands(builder, checked.operation(), &mut pending)?;
    }
    Ok(capability)
}

fn raise_for_place(
    builder: &CheckedBodyBuilder,
    place: PlaceId,
    required: CallableCapability,
    capability: &mut CallableCapability,
) -> Result<(), BodyCheckInternalError> {
    let place = builder
        .place(place)
        .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
    if matches!(place.root(), PlaceRoot::Capture(_)) {
        raise(capability, required);
    }
    Ok(())
}

fn node_has_capture_root(
    builder: &CheckedBodyBuilder,
    node: BodyNodeId,
) -> Result<bool, BodyCheckInternalError> {
    let checked = builder
        .node(node)
        .ok_or(BodyCheckInternalError::MissingNode(node))?;
    let place = match checked.operation() {
        CheckedOperation::Place(place)
        | CheckedOperation::Copy(place)
        | CheckedOperation::Move(place)
        | CheckedOperation::Borrow { place, .. } => *place,
        _ => return Ok(false),
    };
    let place = builder
        .place(place)
        .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
    Ok(matches!(place.root(), PlaceRoot::Capture(_)))
}

fn raise(current: &mut CallableCapability, required: CallableCapability) {
    if rank(required) > rank(*current) {
        *current = required;
    }
}

const fn rank(capability: CallableCapability) -> u8 {
    match capability {
        CallableCapability::Readonly => 0,
        CallableCapability::ReadWrite => 1,
        CallableCapability::Owned => 2,
    }
}

fn append_operands(
    builder: &CheckedBodyBuilder,
    operation: &CheckedOperation,
    pending: &mut Vec<BodyNodeId>,
) -> Result<(), BodyCheckInternalError> {
    if let CheckedOperation::Control(control) = operation {
        return append_control_operands(builder, control, pending);
    }
    match operation {
        CheckedOperation::Complete
        | CheckedOperation::Constant(_)
        | CheckedOperation::Place(_)
        | CheckedOperation::Copy(_)
        | CheckedOperation::Move(_)
        | CheckedOperation::Borrow { .. }
        | CheckedOperation::ArgumentPackLength(_)
        | CheckedOperation::Outcome(CheckedOutcome::Absent) => {}
        CheckedOperation::BorrowConversion(conversion) => pending.push(conversion.value()),
        CheckedOperation::CallableGuaranteeErasure(value) => pending.push(*value),
        CheckedOperation::OpaqueWitness(witness) => pending.push(witness.value()),
        CheckedOperation::Primitive(
            PrimitiveOperation::Unary { operand, .. }
            | PrimitiveOperation::IntegerConversion { operand, .. },
        ) => {
            pending.push(*operand);
        }
        CheckedOperation::Primitive(PrimitiveOperation::Binary { left, right, .. }) => {
            pending.extend([*right, *left]);
        }
        CheckedOperation::Comparison(comparison) => {
            pending.extend([comparison.right().value(), comparison.left().value()]);
        }
        CheckedOperation::Call(call) => {
            if let Some(pack) = call.pack() {
                pending.extend(
                    pack.segments()
                        .iter()
                        .rev()
                        .flat_map(|segment| segment.operands().rev()),
                );
            }
            pending.extend(call.arguments().iter().rev().copied());
            if let Some(receiver) = call.receiver() {
                pending.push(receiver.value());
            }
            if let CallTarget::CallableValue { value, .. }
            | CallTarget::ClosureValue { value, .. } = call.target()
            {
                pending.push(*value);
            }
        }
        CheckedOperation::Aggregate(AggregateConstruction::Struct { fields, .. }) => {
            pending.extend(fields.iter().rev().map(|(_, value)| *value));
        }
        CheckedOperation::Aggregate(
            AggregateConstruction::Enum { payload, .. }
            | AggregateConstruction::FixedArray(payload),
        ) => pending.extend(payload.iter().rev().copied()),
        CheckedOperation::Outcome(
            CheckedOutcome::Inject { payload, .. }
            | CheckedOutcome::Failure(payload)
            | CheckedOutcome::Propagate {
                operand: payload, ..
            }
            | CheckedOutcome::Force {
                operand: payload, ..
            },
        ) => pending.push(*payload),
        CheckedOperation::Outcome(CheckedOutcome::Recover {
            operand, fallback, ..
        }) => pending.extend([*fallback, *operand]),
        CheckedOperation::Closure(closure) => pending.extend(
            closure
                .captures()
                .iter()
                .rev()
                .map(|capture| capture.initializer()),
        ),
        CheckedOperation::IteratorAcquisition(acquisition) => {
            pending.push(acquisition.source().value());
        }
        CheckedOperation::PackLiteral(sequence) => {
            for element in sequence.pack().segments().iter().rev() {
                pending.extend(element.operands().rev());
            }
            if let AllocationSelection::Explicit(allocator) = sequence.allocation() {
                pending.push(allocator);
            }
        }
        CheckedOperation::StringLiteral { allocation, .. } => {
            if let AllocationSelection::Explicit(allocator) = allocation {
                pending.push(*allocator);
            }
        }
        CheckedOperation::Interpolation(interpolation) => {
            append_interpolation_operands(interpolation, pending);
        }
        CheckedOperation::Control(_) => unreachable!("control operations return above"),
    }
    Ok(())
}

fn append_interpolation_operands(
    interpolation: &crate::CheckedInterpolation,
    pending: &mut Vec<BodyNodeId>,
) {
    pending.extend(
        interpolation
            .parts()
            .iter()
            .rev()
            .filter_map(|part| match part {
                crate::InterpolationPart::Text(_) => None,
                crate::InterpolationPart::Formatted { operand, .. } => Some(operand.value()),
                crate::InterpolationPart::Diverging(value) => Some(*value),
            }),
    );
    if let AllocationSelection::Explicit(allocator) = interpolation.allocation() {
        pending.push(allocator);
    }
}

fn append_control_operands(
    builder: &CheckedBodyBuilder,
    control: &CheckedControl,
    pending: &mut Vec<BodyNodeId>,
) -> Result<(), BodyCheckInternalError> {
    match control {
        CheckedControl::Block {
            statements, result, ..
        } => {
            if let Some(result) = result {
                pending.push(*result);
            }
            pending.extend(statements.iter().rev().copied());
        }
        CheckedControl::Bind { initializer, .. }
        | CheckedControl::Discard(initializer)
        | CheckedControl::Unreachable(initializer) => pending.push(*initializer),
        CheckedControl::Assign { target, value }
        | CheckedControl::CompoundAssign { target, value, .. } => {
            pending.push(*value);
            append_place_evaluation(builder, *target, pending)?;
        }
        CheckedControl::Return(value) => pending.extend(value.iter().copied()),
        CheckedControl::Break(_) | CheckedControl::Continue(_) | CheckedControl::Drop(_) => {}
        CheckedControl::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.extend(else_branch.iter().copied());
            pending.extend([*then_branch, *condition]);
        }
        CheckedControl::Logical { left, right, .. } => pending.extend([*right, *left]),
        CheckedControl::Pattern {
            subject,
            arms,
            fallback,
            ..
        } => {
            pending.extend(fallback.iter().map(|fallback| fallback.body()));
            pending.extend(arms.iter().rev().map(crate::CheckedPatternArm::body));
            pending.push(subject.value());
        }
        CheckedControl::Loop(loop_) => {
            let loop_ = builder
                .loop_definition(*loop_)
                .ok_or(BodyCheckInternalError::UnknownLoop(*loop_))?;
            pending.push(loop_.body());
            match loop_.kind() {
                LoopKind::Infinite
                | LoopKind::ArgumentPack { .. }
                | LoopKind::KeyedArgumentPack { .. } => {}
                LoopKind::While { condition } => pending.push(*condition),
                LoopKind::For { iteration, .. } => pending.push(iteration.iterator()),
                LoopKind::Range { start, end, .. } => pending.extend([*end, *start]),
            }
        }
        CheckedControl::Region {
            allocator, body, ..
        } => pending.extend([*body, *allocator]),
    }
    Ok(())
}

fn append_place_evaluation(
    builder: &CheckedBodyBuilder,
    place: PlaceId,
    pending: &mut Vec<BodyNodeId>,
) -> Result<(), BodyCheckInternalError> {
    let place = builder
        .place(place)
        .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
    pending.extend(place.evaluation_nodes());
    Ok(())
}

use nocter_model::{BorrowCapability, BuiltinType, MirOperationId, TypeKind};

use crate::validation_call::validate_call;
use crate::validation_destruction::validate_destruction_plan;
use crate::{
    MirBody, MirCall, MirCallTarget, MirPackArgument, MirPackContribution, MirPackSegment,
    MirValidationEnvironment, MirValidationError,
};

pub(crate) fn validate_call_pack(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirBody,
    operation: MirOperationId,
    call: &MirCall,
) -> Result<(), MirValidationError> {
    let expected = match call.target() {
        MirCallTarget::Direct(item) => environment.item_pack_input(*item),
        MirCallTarget::StandardPrimitive { .. } | MirCallTarget::Structural(_) => None,
    };
    match (expected, call.pack()) {
        (None, None) => Ok(()),
        (Some(expected), Some(pack)) if expected == (pack.element(), pack.next()) => {
            validate_pack(environment, function, operation, call, pack)
        }
        _ => Err(MirValidationError::OperationType(operation)),
    }
}

fn validate_pack(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirBody,
    operation: MirOperationId,
    call: &MirCall,
    pack: &MirPackArgument,
) -> Result<(), MirValidationError> {
    let invalid = || MirValidationError::OperationType(operation);
    let types = environment.types();
    if !call.arguments().is_empty()
        || function
            .values()
            .get(pack.length())
            .copied()
            .map(crate::MirValue::ty)
            != Some(types.builtin(BuiltinType::Usize))
        || !matches!(types.get(pack.next()), Some(TypeKind::Optional(payload)) if *payload == pack.element())
    {
        return Err(invalid());
    }
    for segment in pack.segments() {
        match segment {
            MirPackSegment::Value { value, destruction } => {
                if function
                    .values()
                    .get(*value)
                    .copied()
                    .map(crate::MirValue::ty)
                    != Some(pack.element())
                {
                    return Err(invalid());
                }
                validate_segment_destruction(environment, destruction.as_ref(), pack.element())?;
            }
            MirPackSegment::Spread(spread) => {
                let iterator_ty = function
                    .places()
                    .get(spread.iterator())
                    .map(crate::MirPlace::ty)
                    .ok_or(MirValidationError::UnknownPlace(spread.iterator()))?;
                if function
                    .values()
                    .get(spread.remaining())
                    .copied()
                    .map(crate::MirValue::ty)
                    != Some(types.builtin(BuiltinType::Usize))
                    || !matches!(
                        types.get(spread.next_result()),
                        Some(TypeKind::Optional(payload)) if *payload == spread.item()
                    )
                {
                    return Err(invalid());
                }
                let next = MirCall::new(spread.next_target().clone(), [spread.receiver()]);
                validate_call(
                    environment,
                    function,
                    operation,
                    &next,
                    spread.next_result(),
                )?;
                let valid_contribution = match spread.contribution() {
                    MirPackContribution::Direct => spread.item() == pack.element(),
                    MirPackContribution::CopyBorrowed => matches!(
                        types.get(spread.item()),
                        Some(TypeKind::Borrow {
                            capability: BorrowCapability::Readonly,
                            referent,
                        }) if *referent == pack.element()
                    ),
                };
                if !valid_contribution {
                    return Err(invalid());
                }
                validate_segment_destruction(environment, spread.destruction(), iterator_ty)?;
            }
        }
    }
    Ok(())
}

fn validate_segment_destruction(
    environment: &(impl MirValidationEnvironment + ?Sized),
    plan: Option<&crate::MirDestructionPlan>,
    expected: nocter_model::TypeId,
) -> Result<(), MirValidationError> {
    let Some(plan) = plan else {
        return Ok(());
    };
    if plan.ty() != expected {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    }
    validate_destruction_plan(environment, plan)
}

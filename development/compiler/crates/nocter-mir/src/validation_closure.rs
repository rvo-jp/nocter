use nocter_model::{
    BorrowCapability, CallableCapability, ExecutableItemId, MirOperationId, TypeId, TypeKind,
    TypeStore,
};
use nocter_target_program::ExecutableClosureLayout;

use crate::{
    MirClosureCapture, MirFunction, MirLocal, MirValidationEnvironment, MirValidationError,
};

/// Validates one closure aggregate against the frozen executable layout selected for its body.
pub(super) fn validate_closure_aggregate(
    environment: &(impl MirValidationEnvironment + ?Sized),
    operation: MirOperationId,
    result: TypeId,
    body: ExecutableItemId,
    captures: &[MirClosureCapture],
    capture_types: &[TypeId],
) -> Result<(), MirValidationError> {
    let invalid = || MirValidationError::OperationType(operation);
    let layout = environment.closure_layout(body).ok_or_else(invalid)?;
    let Some(TypeKind::Closure { definition, .. }) = environment.types().get(result) else {
        return Err(invalid());
    };
    if layout.ty() != result
        || layout.closure() != *definition
        || captures.len() != layout.captures().len()
        || capture_types.len() != captures.len()
    {
        return Err(invalid());
    }
    for ((capture, actual), expected) in captures
        .iter()
        .copied()
        .zip(capture_types.iter().copied())
        .zip(layout.captures().iter().copied())
    {
        if capture.binding() != expected.binding() || actual != expected.ty() {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Checks the hidden environment input of a lowered closure body against its executable layout.
pub(super) fn has_valid_closure_environment_signature(
    function: &MirFunction,
    layout: &ExecutableClosureLayout,
    types: &TypeStore,
) -> bool {
    let environment = function
        .parameters()
        .first()
        .and_then(|parameter| function.locals().get(*parameter))
        .copied()
        .map(MirLocal::ty);
    match layout.capability() {
        CallableCapability::Owned => environment == Some(layout.ty()),
        CallableCapability::Readonly => matches!(
            environment.and_then(|ty| types.get(ty)),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) if *referent == layout.ty()
        ),
        CallableCapability::ReadWrite => matches!(
            environment.and_then(|ty| types.get(ty)),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::ReadWrite,
                referent,
            }) if *referent == layout.ty()
        ),
    }
}

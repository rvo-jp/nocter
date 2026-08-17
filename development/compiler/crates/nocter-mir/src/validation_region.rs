use nocter_declarations::StandardDeclarationRole;
use nocter_model::{MirLocalId, MirOperationId, MirValueId, TypeId, TypeKind};

use crate::{MirBody, MirLocalKind, MirValidationEnvironment, MirValidationError};

pub(crate) fn validate_region_creation(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirBody,
    operation: MirOperationId,
    parent: MirValueId,
    result: TypeId,
) -> Result<(), MirValidationError> {
    let context = environment
        .standard_nominal(StandardDeclarationRole::AllocationContext)
        .ok_or(MirValidationError::OperationType(operation))?;
    let allocator = environment
        .standard_nominal(StandardDeclarationRole::AbortingAllocator)
        .ok_or(MirValidationError::OperationType(operation))?;
    if !is_nominal(environment, result, context) {
        return Err(MirValidationError::OperationType(operation));
    }
    let parent = function
        .values()
        .get(parent)
        .ok_or(MirValidationError::UnknownValue(parent))?
        .ty();
    let Some(TypeKind::Borrow { referent, .. }) = environment.types().get(parent) else {
        return Err(MirValidationError::OperationType(operation));
    };
    if !is_nominal(environment, *referent, allocator)
        && !is_nominal(environment, *referent, context)
    {
        return Err(MirValidationError::OperationType(operation));
    }
    Ok(())
}

pub(crate) fn validate_region_release(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirBody,
    operation: MirOperationId,
    region: MirLocalId,
    result: Option<TypeId>,
) -> Result<(), MirValidationError> {
    let context = environment
        .standard_nominal(StandardDeclarationRole::AllocationContext)
        .ok_or(MirValidationError::OperationType(operation))?;
    let local = function
        .locals()
        .get(region)
        .ok_or(MirValidationError::UnknownLocal(region))?;
    if result.is_some()
        || local.kind() != MirLocalKind::Region
        || !is_nominal(environment, local.ty(), context)
    {
        return Err(MirValidationError::OperationType(operation));
    }
    Ok(())
}

fn is_nominal(
    environment: &(impl MirValidationEnvironment + ?Sized),
    ty: TypeId,
    expected: nocter_model::NominalTypeId,
) -> bool {
    matches!(
        environment.types().get(ty),
        Some(TypeKind::Nominal {
            definition,
            arguments,
        }) if *definition == expected && arguments.is_empty()
    )
}

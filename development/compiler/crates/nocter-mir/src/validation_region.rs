use nocter_model::{MirBlockId, MirLocalId, MirOperationId, MirValueId, TypeId, TypeKind};

use std::collections::{BTreeMap, VecDeque};

use crate::{MirBody, MirLocalKind, MirValidationEnvironment, MirValidationError};

/// Verifies the lexical resource stack over the complete CFG.
///
/// A region is current only after its creation and until its matching release. Every merge must
/// receive the same ordered stack, so inner-first cleanup and lexical restoration are structural
/// MIR invariants rather than assumptions made by a target backend.
pub(crate) fn validate_region_flow(function: &MirBody) -> Result<(), MirValidationError> {
    let mut incoming = BTreeMap::<MirBlockId, Vec<MirLocalId>>::new();
    let mut pending = VecDeque::from([function.entry()]);
    incoming.insert(function.entry(), Vec::new());
    while let Some(block_id) = pending.pop_front() {
        let block = function
            .blocks()
            .get(block_id)
            .ok_or(MirValidationError::UnknownBlock(block_id))?;
        let mut active =
            incoming
                .get(&block_id)
                .cloned()
                .ok_or(MirValidationError::InvalidRegionFlow {
                    block: block_id,
                    region: None,
                })?;
        for operation in block.operations() {
            let operation = function
                .operations()
                .get(*operation)
                .ok_or(MirValidationError::UnknownOperation(*operation))?;
            match operation.kind() {
                crate::MirOperationKind::CreateRegion { region, .. } => {
                    if active.contains(region) {
                        return Err(flow_error(block_id, Some(*region)));
                    }
                    active.push(*region);
                }
                crate::MirOperationKind::ReleaseRegion { region } => {
                    if active.pop() != Some(*region) {
                        return Err(flow_error(block_id, Some(*region)));
                    }
                }
                crate::MirOperationKind::Call(call) => {
                    validate_current_selection(function, block_id, call.allocation(), &active)?;
                }
                crate::MirOperationKind::InvokeDrop { allocation, .. } => {
                    validate_current_selection(function, block_id, *allocation, &active)?;
                }
                _ => {}
            }
        }
        if matches!(
            block.terminator(),
            crate::MirTerminator::Return(_) | crate::MirTerminator::Exit(_)
        ) && !active.is_empty()
        {
            return Err(flow_error(block_id, active.last().copied()));
        }
        for target in crate::validation_graph::successors(block.terminator()) {
            let target = target.block();
            match incoming.get(&target) {
                Some(expected) if expected != &active => {
                    return Err(flow_error(target, active.last().copied()));
                }
                Some(_) => {}
                None => {
                    incoming.insert(target, active.clone());
                    pending.push_back(target);
                }
            }
        }
    }
    Ok(())
}

fn validate_current_selection(
    function: &MirBody,
    block: MirBlockId,
    allocation: crate::MirCallAllocation,
    active: &[MirLocalId],
) -> Result<(), MirValidationError> {
    match allocation {
        crate::MirCallAllocation::Region(region) if active.last().copied() != Some(region) => {
            return Err(flow_error(block, Some(region)));
        }
        crate::MirCallAllocation::Explicit(place) => {
            let place = function
                .places()
                .get(place)
                .ok_or(MirValidationError::UnknownPlace(place))?;
            if let crate::MirPlaceRoot::Local(local) = place.root()
                && function
                    .locals()
                    .get(local)
                    .is_some_and(|local| local.kind() == MirLocalKind::Region)
                && (!place.projections().is_empty() || !active.contains(&local))
            {
                return Err(flow_error(block, Some(local)));
            }
        }
        crate::MirCallAllocation::Inherit | crate::MirCallAllocation::Region(_) => {}
    }
    Ok(())
}

const fn flow_error(block: MirBlockId, region: Option<MirLocalId>) -> MirValidationError {
    MirValidationError::InvalidRegionFlow { block, region }
}

pub(crate) fn validate_region_creation(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirBody,
    operation: MirOperationId,
    parent: MirValueId,
    region: MirLocalId,
    result: Option<TypeId>,
) -> Result<(), MirValidationError> {
    let context = environment
        .allocation_context_nominal()
        .ok_or(MirValidationError::OperationType(operation))?;
    let allocator = environment
        .aborting_allocator_nominal()
        .ok_or(MirValidationError::OperationType(operation))?;
    if result.is_some() {
        return Err(MirValidationError::OperationType(operation));
    }
    validate_region_selection(environment, function, operation, region)?;
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
    if result.is_some() {
        return Err(MirValidationError::OperationType(operation));
    }
    validate_region_selection(environment, function, operation, region)
}

pub(crate) fn validate_region_selection(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirBody,
    operation: MirOperationId,
    region: MirLocalId,
) -> Result<(), MirValidationError> {
    let context = environment
        .allocation_context_nominal()
        .ok_or(MirValidationError::OperationType(operation))?;
    let local = function
        .locals()
        .get(region)
        .ok_or(MirValidationError::UnknownLocal(region))?;
    if local.kind() != MirLocalKind::Region || !is_nominal(environment, local.ty(), context) {
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

use nocter_mir::{
    MirCall, MirCallAllocation, MirCallSignature, MirCallTarget, MirPrimitiveDependency,
};
use nocter_model::MirOperationId;
use nocter_runtime_contract::{PrimitiveRole, RuntimeType};

use super::MachineProgramError;
use super::body::BodyIdentities;
use super::context::ProgramLoweringContext;
use super::structural::lower_structural;
use crate::{
    MachineCall, MachineCallAllocation, MachineCallPack, MachineCallTarget, MachineImportedTarget,
    MachineOperationKind, MachinePrimitiveDependency, MachinePrimitiveTarget,
};

pub(super) fn lower_call(
    operation: MirOperationId,
    call: &MirCall,
    context: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
) -> Result<MachineOperationKind, MachineProgramError> {
    if let MirCallTarget::Structural(target) = call.target() {
        return lower_structural(
            operation,
            target,
            call.arguments(),
            context.types,
            context.layouts,
            ids,
        );
    }
    let target = lower_call_target(operation, call.target(), context, ids)?;
    let arguments = call
        .arguments()
        .iter()
        .map(|argument| ids.value(*argument))
        .collect::<Result<Vec<_>, _>>()?;
    let allocation = match call.allocation() {
        MirCallAllocation::Inherit => MachineCallAllocation::Inherit,
        MirCallAllocation::Region(region) => MachineCallAllocation::Lexical(ids.stack(region)?),
        MirCallAllocation::Explicit(place) => MachineCallAllocation::Explicit(ids.address(place)?),
    };
    let pack = call
        .pack()
        .map(|pack| match pack {
            nocter_mir::MirCallPack::Prepared(_) => {
                ids.pack(operation).map(MachineCallPack::Prepared)
            }
            nocter_mir::MirCallPack::Forwarded(_) => Ok(MachineCallPack::Forwarded),
        })
        .transpose()?;
    Ok(MachineOperationKind::Call(MachineCall::new(
        target, arguments, allocation, pack,
    )))
}

pub(super) fn lower_call_target(
    operation: MirOperationId,
    target: &MirCallTarget,
    context: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
) -> Result<MachineCallTarget, MachineProgramError> {
    match target {
        MirCallTarget::Direct(target) => context
            .functions
            .for_item(*target)
            .map(MachineCallTarget::Direct)
            .ok_or(MachineProgramError::MissingItemFunction(*target)),
        MirCallTarget::StandardPrimitive {
            role,
            type_arguments,
            signature,
            dependency,
        } => {
            if *role == nocter_runtime_contract::PrimitiveRole::DropValueAtPointer
                && matches!(
                    dependency,
                    MirPrimitiveDependency::Destruction { plan: Some(_), .. }
                )
            {
                let destruction = context.destructions.call(ids.owner(), operation).ok_or(
                    MachineProgramError::MissingGeneratedDestruction(ids.owner(), operation),
                )?;
                let function = context
                    .functions
                    .for_destruction(destruction)
                    .ok_or(MachineProgramError::MissingDestruction(destruction))?;
                return Ok(MachineCallTarget::Direct(function));
            }
            let abi = context
                .abi
                .runtime_call_signature_id(signature)
                .ok_or(MachineProgramError::MissingPrimitiveAbi(operation))?;
            let dependency = match dependency {
                MirPrimitiveDependency::None if *role == PrimitiveRole::TaskJoin => {
                    MachinePrimitiveDependency::AsyncJoin(async_join_plan(
                        operation, signature, context,
                    )?)
                }
                MirPrimitiveDependency::None => MachinePrimitiveDependency::None,
                MirPrimitiveDependency::Destruction { subject, plan } => {
                    if plan.is_some() {
                        return Err(MachineProgramError::MissingGeneratedDestruction(
                            ids.owner(),
                            operation,
                        ));
                    }
                    MachinePrimitiveDependency::NoopDestruction { subject: *subject }
                }
            };
            Ok(MachineCallTarget::Primitive(MachinePrimitiveTarget::new(
                *role,
                type_arguments.clone(),
                abi,
                dependency,
            )))
        }
        MirCallTarget::TargetService {
            descriptor,
            signature,
        } => {
            let abi = context
                .abi
                .runtime_call_signature_id(signature)
                .ok_or(MachineProgramError::MissingRuntimeCallAbi(operation))?;
            let import = context
                .imports
                .id(descriptor)
                .ok_or(MachineProgramError::MissingTargetServiceImport(operation))?;
            Ok(MachineCallTarget::Imported(MachineImportedTarget::new(
                import, abi,
            )))
        }
        MirCallTarget::Structural(_) => Err(MachineProgramError::InvalidPackTarget {
            owner: ids.owner(),
            operation,
        }),
    }
}

fn async_join_plan(
    operation: MirOperationId,
    signature: &MirCallSignature,
    context: ProgramLoweringContext<'_>,
) -> Result<crate::MachineAsyncJoinPlan, MachineProgramError> {
    let Some(RuntimeType::Async(output)) = context.types.get(signature.result()) else {
        return Err(MachineProgramError::InvalidAsyncJoin(operation));
    };
    let Some(RuntimeType::Tuple(elements)) = context.types.get(*output) else {
        return Err(MachineProgramError::InvalidAsyncJoin(operation));
    };
    let [first, second] = elements.as_ref() else {
        return Err(MachineProgramError::InvalidAsyncJoin(operation));
    };
    let Some(layout) = context.layouts.get(*output) else {
        return Err(MachineProgramError::MissingStoredLayout(*output));
    };
    let crate::MachineLayoutKind::Tuple { elements: placed } = layout.kind() else {
        return Err(MachineProgramError::InvalidAsyncJoin(operation));
    };
    let [first_placed, second_placed] = placed.as_ref() else {
        return Err(MachineProgramError::InvalidAsyncJoin(operation));
    };
    if first_placed.ty() != *first || second_placed.ty() != *second {
        return Err(MachineProgramError::InvalidAsyncJoin(operation));
    }
    Ok(crate::MachineAsyncJoinPlan::new(
        first_placed.offset(),
        second_placed.offset(),
    ))
}

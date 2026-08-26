use nocter_mir::{MirCall, MirCallAllocation, MirCallTarget, MirPrimitiveDependency};
use nocter_model::MirOperationId;

use super::MachineProgramError;
use super::body::BodyIdentities;
use super::context::ProgramLoweringContext;
use super::structural::lower_structural;
use crate::{
    MachineCall, MachineCallAllocation, MachineCallPack, MachineCallTarget, MachineOperationKind,
    MachinePrimitiveDependency, MachinePrimitiveTarget,
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
            let abi = crate::transport::plan_signature(
                context.types,
                context.layouts,
                signature.parameters(),
                signature.result(),
                None,
            )?;
            let dependency = match dependency {
                MirPrimitiveDependency::None => MachinePrimitiveDependency::None,
                MirPrimitiveDependency::Destruction { subject, plan } => {
                    if plan.is_some() {
                        return Err(MachineProgramError::MissingGeneratedDestruction(
                            ids.owner(),
                            operation,
                        ));
                    }
                    MachinePrimitiveDependency::Destruction {
                        subject: *subject,
                        plan: None,
                    }
                }
            };
            Ok(MachineCallTarget::Primitive(MachinePrimitiveTarget::new(
                *role,
                type_arguments.clone(),
                abi,
                dependency,
            )))
        }
        MirCallTarget::Structural(_) => Err(MachineProgramError::InvalidPackTarget {
            owner: ids.owner(),
            operation,
        }),
    }
}

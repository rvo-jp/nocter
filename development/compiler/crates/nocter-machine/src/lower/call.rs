use std::collections::BTreeMap;

use nocter_mir::{MirCall, MirCallAllocation, MirCallTarget};
use nocter_model::{ExecutableItemId, MirOperationId, TypeStore};

use super::MachineProgramError;
use super::body::BodyIdentities;
use super::structural::lower_structural;
use crate::{
    MachineCall, MachineCallAllocation, MachineCallTarget, MachineFunctionId, MachineLayoutStore,
    MachineOperationKind, MachinePrimitiveTarget,
};

pub(super) fn lower_call(
    operation: MirOperationId,
    call: &MirCall,
    types: &TypeStore,
    layouts: &MachineLayoutStore,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    ids: &BodyIdentities,
) -> Result<MachineOperationKind, MachineProgramError> {
    if let MirCallTarget::Structural(target) = call.target() {
        return lower_structural(operation, target, call.arguments(), types, layouts, ids);
    }
    let target = lower_call_target(operation, call.target(), types, layouts, functions, ids)?;
    let arguments = call
        .arguments()
        .iter()
        .map(|argument| ids.value(*argument))
        .collect::<Result<Vec<_>, _>>()?;
    let allocation = match call.allocation() {
        MirCallAllocation::Inherit => MachineCallAllocation::Inherit,
        MirCallAllocation::Explicit(place) => MachineCallAllocation::Explicit(ids.address(place)?),
    };
    let pack = call.pack().map(|_| ids.pack(operation)).transpose()?;
    Ok(MachineOperationKind::Call(MachineCall::new(
        target, arguments, allocation, pack,
    )))
}

pub(super) fn lower_call_target(
    operation: MirOperationId,
    target: &MirCallTarget,
    types: &TypeStore,
    layouts: &MachineLayoutStore,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    ids: &BodyIdentities,
) -> Result<MachineCallTarget, MachineProgramError> {
    match target {
        MirCallTarget::Direct(target) => functions
            .get(target)
            .copied()
            .map(MachineCallTarget::Direct)
            .ok_or(MachineProgramError::MissingItemFunction(*target)),
        MirCallTarget::StandardPrimitive {
            role,
            type_arguments,
            signature,
        } => {
            let abi = crate::transport::plan_signature(
                types,
                layouts,
                signature.parameters(),
                signature.result(),
                None,
            )?;
            Ok(MachineCallTarget::Primitive(MachinePrimitiveTarget::new(
                *role,
                type_arguments.clone(),
                abi,
            )))
        }
        MirCallTarget::Structural(_) => Err(MachineProgramError::InvalidPackTarget {
            owner: ids.owner(),
            operation,
        }),
    }
}

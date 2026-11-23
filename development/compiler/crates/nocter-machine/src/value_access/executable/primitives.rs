//! Physical primitive call plans from the sole closed value signature, without source bodies.
use super::Failure;
use crate::identity::{MachineId, MachineRuntimeCallAbiId};
use crate::transport::{MachineRuntimeCallAbiTable, plan_signature};
use crate::{
    MachineCallableAbi, MachineLayoutPlan, MachinePrimitiveDependency, MachinePrimitiveTarget,
};
use nocter_mir::value_access::{CallTarget, Operation, Program};
use nocter_runtime_contract::{PrimitiveRole, RuntimePrimitive};
use std::collections::{BTreeMap, btree_map::Entry};

type Plans = (
    BTreeMap<PrimitiveRole, MachinePrimitiveTarget>,
    MachineRuntimeCallAbiTable,
);
pub(super) fn prepare(program: &Program, layouts: &MachineLayoutPlan) -> Result<Plans, Failure> {
    let calls = program
        .bodies()
        .values()
        .flat_map(|body| body.actions().values())
        .flat_map(|action| action.operations.iter())
        .filter_map(|operation| match operation {
            Operation::Call(call) => match call.target {
                CallTarget::Primitive(role) => Some((role, call.answer.data)),
                CallTarget::Body(_) | CallTarget::Selected => None,
            },
            _ => None,
        });
    let runtime = program.runtime();
    let mut results = BTreeMap::new();
    for (role, result) in calls {
        match results.entry(role) {
            Entry::Vacant(site) => {
                site.insert(result);
            }
            Entry::Occupied(site) if runtime.has_same_value_representation(*site.get(), result) => {
            }
            Entry::Occupied(_) => return Err(Failure::InvalidRoot),
        }
    }
    let mut targets = BTreeMap::new();
    let mut abis = Vec::<MachineCallableAbi>::new();
    for (role, result) in results {
        let signature = role.value_signature().ok_or(Failure::InvalidRoot)?;
        if !signature.result().matches_runtime(runtime.types(), result) {
            return Err(Failure::InvalidRoot);
        }
        let data = |builtin| {
            runtime
                .types()
                .primitive(RuntimePrimitive::for_builtin(builtin))
                .ok_or(Failure::InvalidRoot)
        };
        let inputs = signature
            .inputs()
            .iter()
            .map(|input| data(input.data()))
            .collect::<Result<Vec<_>, _>>()?;
        let abi = plan_signature(runtime.types(), layouts, &inputs, result, None)
            .map_err(Failure::Abi)?;
        let index = if let Some(index) = abis.iter().position(|existing| existing == &abi) {
            index
        } else {
            let index = abis.len();
            abis.push(abi);
            index
        };
        targets.insert(
            role,
            MachinePrimitiveTarget::new(
                role,
                Vec::new(),
                MachineRuntimeCallAbiId::new(index),
                MachinePrimitiveDependency::None,
            ),
        );
    }
    Ok((targets, MachineRuntimeCallAbiTable::from_values(abis)))
}

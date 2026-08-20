use std::collections::BTreeMap;

use nocter_mir::{MirBody, MirCallTarget, MirOperationKind, MirPrimitiveDependency, MirProgram};
use nocter_model::{ExecutableItemId, MirOperationId};
use nocter_target_program::PrimitiveRole;

use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineCallableAbi, MachineDestructionId, MachineDestructionPlan, MachineFunctionId,
    MachineLayoutStore, MachineLinkageId, MachineLinkageKey, MachineLinkageTable,
    MachineProgramError,
};

/// One canonical compiler-generated destruction function contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineDestruction {
    plan: MachineDestructionPlan,
    abi: MachineCallableAbi,
}

impl MachineDestruction {
    #[must_use]
    pub const fn plan(&self) -> &MachineDestructionPlan {
        &self.plan
    }

    #[must_use]
    pub const fn abi(&self) -> &MachineCallableAbi {
        &self.abi
    }
}

/// Deterministic identity and ABI authority for concrete destruction work reached by machine
/// primitives. Discovery order never becomes generated function identity.
#[derive(Debug)]
pub struct MachineDestructionTable {
    entries: MachineTable<MachineDestructionId, MachineDestruction>,
    ids: BTreeMap<MachineDestructionPlan, MachineDestructionId>,
    calls: BTreeMap<(MachineLinkageId, MirOperationId), MachineDestructionId>,
}

impl MachineDestructionTable {
    pub(crate) fn build(
        program: &MirProgram,
        layouts: &MachineLayoutStore,
        linkage: &MachineLinkageTable,
        functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    ) -> Result<Self, MachineProgramError> {
        let mut plans = BTreeMap::<MachineDestructionPlan, MachineCallableAbi>::new();
        let mut calls = BTreeMap::new();
        for (item, function) in program.functions().iter() {
            let owner = require_linkage(linkage, MachineLinkageKey::Item(item))?;
            collect_body(
                owner,
                function.body(),
                program,
                layouts,
                functions,
                &mut plans,
                &mut calls,
            )?;
        }
        match program.root() {
            nocter_mir::MirRoot::Process(root) => collect_body(
                require_linkage(linkage, MachineLinkageKey::ProcessRoot(root.target()))?,
                root.body(),
                program,
                layouts,
                functions,
                &mut plans,
                &mut calls,
            )?,
            nocter_mir::MirRoot::Tests { cases, .. } => {
                for case in cases {
                    collect_body(
                        require_linkage(linkage, MachineLinkageKey::TestRoot(case.declaration()))?,
                        case.body(),
                        program,
                        layouts,
                        functions,
                        &mut plans,
                        &mut calls,
                    )?;
                }
            }
        }

        let mut ids = BTreeMap::new();
        let entries = plans
            .into_iter()
            .enumerate()
            .map(|(index, (plan, abi))| {
                let id = MachineDestructionId::new(index);
                ids.insert(plan.clone(), id);
                MachineDestruction { plan, abi }
            })
            .collect::<Vec<_>>();
        let calls = calls
            .into_iter()
            .map(|(call, plan)| {
                ids.get(&plan)
                    .copied()
                    .map(|destruction| (call, destruction))
                    .ok_or(MachineProgramError::MissingGeneratedDestruction(
                        call.0, call.1,
                    ))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self {
            entries: MachineTable::from_values(entries),
            ids,
            calls,
        })
    }

    #[must_use]
    pub fn get(&self, id: MachineDestructionId) -> Option<&MachineDestruction> {
        self.entries.get(id)
    }

    #[must_use]
    pub fn id(&self, plan: &MachineDestructionPlan) -> Option<MachineDestructionId> {
        self.ids.get(plan).copied()
    }

    #[must_use]
    pub(crate) fn call(
        &self,
        owner: MachineLinkageId,
        operation: MirOperationId,
    ) -> Option<MachineDestructionId> {
        self.calls.get(&(owner, operation)).copied()
    }

    #[must_use]
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineDestructionId, &MachineDestruction)> {
        self.entries.iter()
    }
}

fn collect_body(
    owner: MachineLinkageId,
    body: &MirBody,
    program: &MirProgram,
    layouts: &MachineLayoutStore,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    plans: &mut BTreeMap<MachineDestructionPlan, MachineCallableAbi>,
    calls: &mut BTreeMap<(MachineLinkageId, MirOperationId), MachineDestructionPlan>,
) -> Result<(), MachineProgramError> {
    for (operation, value) in body.operations().iter() {
        let MirOperationKind::Call(call) = value.kind() else {
            continue;
        };
        let MirCallTarget::StandardPrimitive {
            role: PrimitiveRole::DropValueAtPointer,
            signature,
            dependency:
                MirPrimitiveDependency::Destruction {
                    plan: Some(plan), ..
                },
            ..
        } = call.target()
        else {
            continue;
        };
        let plan = crate::lower::destruction::lower_destruction(
            plan, owner, operation, layouts, functions,
        )?;
        let abi = crate::transport::plan_signature(
            program.executable().types(),
            layouts,
            signature.parameters(),
            signature.result(),
            None,
        )?;
        if calls.insert((owner, operation), plan.clone()).is_some() {
            return Err(MachineProgramError::DuplicateDestructionCall(
                owner, operation,
            ));
        }
        if let Some(previous) = plans.insert(plan.clone(), abi.clone())
            && previous != abi
        {
            return Err(MachineProgramError::ConflictingDestructionAbi(plan.ty()));
        }
    }
    Ok(())
}

fn require_linkage(
    linkage: &MachineLinkageTable,
    key: MachineLinkageKey,
) -> Result<MachineLinkageId, MachineProgramError> {
    linkage
        .id(key)
        .ok_or(MachineProgramError::MissingLinkageKey(key))
}

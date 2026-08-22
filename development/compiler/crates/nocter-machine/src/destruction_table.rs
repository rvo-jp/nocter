use std::collections::{BTreeMap, BTreeSet};

use nocter_mir::{
    MirBody, MirCallTarget, MirOperationKind, MirPackSegment, MirPrimitiveDependency, MirProgram,
};
use nocter_model::{BuiltinType, ExecutableItemId, MirOperationId, TypeKind};
use nocter_runtime_contract::PrimitiveRole;

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

/// Deterministic identity and ABI authority for every concrete destruction plan reached by a
/// pointer primitive or literal-pack owner. Discovery order never becomes generated identity.
#[derive(Debug)]
pub struct MachineDestructionTable {
    entries: MachineTable<MachineDestructionId, MachineDestruction>,
    ids: BTreeMap<MachineDestructionPlan, MachineDestructionId>,
    calls: BTreeMap<(MachineLinkageId, MirOperationId), MachineDestructionId>,
    pack_segments: BTreeMap<(MachineLinkageId, MirOperationId, usize), MachineDestructionId>,
}

impl MachineDestructionTable {
    pub(crate) fn build(
        program: &MirProgram,
        layouts: &MachineLayoutStore,
        linkage: &MachineLinkageTable,
        functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    ) -> Result<Self, MachineProgramError> {
        let mut plans = BTreeSet::new();
        let mut calls = BTreeMap::new();
        let mut pack_segments = BTreeMap::new();
        for (item, function) in program.functions().iter() {
            let owner = require_linkage(linkage, MachineLinkageKey::Item(item))?;
            collect_body(
                owner,
                function.body(),
                layouts,
                functions,
                &mut plans,
                &mut calls,
                &mut pack_segments,
            )?;
        }
        match program.root() {
            nocter_mir::MirRoot::Process(root) => collect_body(
                require_linkage(linkage, MachineLinkageKey::ProcessRoot(root.target()))?,
                root.body(),
                layouts,
                functions,
                &mut plans,
                &mut calls,
                &mut pack_segments,
            )?,
            nocter_mir::MirRoot::Tests { cases, .. } => {
                for case in cases {
                    collect_body(
                        require_linkage(linkage, MachineLinkageKey::TestRoot(case.declaration()))?,
                        case.body(),
                        layouts,
                        functions,
                        &mut plans,
                        &mut calls,
                        &mut pack_segments,
                    )?;
                }
            }
        }

        let mut ids = BTreeMap::new();
        let entries = if plans.is_empty() {
            Vec::new()
        } else {
            let abi = destruction_abi(program, layouts)?;
            plans
                .into_iter()
                .enumerate()
                .map(|(index, plan)| {
                    let id = MachineDestructionId::new(index);
                    ids.insert(plan.clone(), id);
                    MachineDestruction {
                        plan,
                        abi: abi.clone(),
                    }
                })
                .collect::<Vec<_>>()
        };
        let calls = close_edges(calls, &ids, |(owner, operation)| {
            MachineProgramError::MissingGeneratedDestruction(owner, operation)
        })?;
        let pack_segments = close_edges(pack_segments, &ids, |(owner, operation, segment)| {
            MachineProgramError::MissingPackDestruction {
                owner,
                operation,
                segment,
            }
        })?;
        Ok(Self {
            entries: MachineTable::from_values(entries),
            ids,
            calls,
            pack_segments,
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
    pub(crate) fn pack_segment(
        &self,
        owner: MachineLinkageId,
        operation: MirOperationId,
        segment: usize,
    ) -> Option<MachineDestructionId> {
        self.pack_segments
            .get(&(owner, operation, segment))
            .copied()
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
    layouts: &MachineLayoutStore,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    plans: &mut BTreeSet<MachineDestructionPlan>,
    calls: &mut BTreeMap<(MachineLinkageId, MirOperationId), MachineDestructionPlan>,
    pack_segments: &mut BTreeMap<(MachineLinkageId, MirOperationId, usize), MachineDestructionPlan>,
) -> Result<(), MachineProgramError> {
    for (operation, value) in body.operations().iter() {
        let MirOperationKind::Call(call) = value.kind() else {
            continue;
        };
        if let MirCallTarget::StandardPrimitive {
            role: PrimitiveRole::DropValueAtPointer,
            dependency:
                MirPrimitiveDependency::Destruction {
                    plan: Some(plan), ..
                },
            ..
        } = call.target()
        {
            let plan = lower_plan(plan, owner, operation, layouts, functions)?;
            if calls.insert((owner, operation), plan.clone()).is_some() {
                return Err(MachineProgramError::DuplicateDestructionCall(
                    owner, operation,
                ));
            }
            plans.insert(plan);
        }
        if let Some(pack) = call.pack() {
            for (segment, source) in pack.segments().iter().enumerate() {
                let destruction = match source {
                    MirPackSegment::Value { destruction, .. } => destruction.as_ref(),
                    MirPackSegment::Spread(spread) => spread.destruction(),
                };
                let Some(destruction) = destruction else {
                    continue;
                };
                let plan = lower_plan(destruction, owner, operation, layouts, functions)?;
                if pack_segments
                    .insert((owner, operation, segment), plan.clone())
                    .is_some()
                {
                    return Err(MachineProgramError::DuplicatePackDestruction {
                        owner,
                        operation,
                        segment,
                    });
                }
                plans.insert(plan);
            }
        }
    }
    Ok(())
}

fn lower_plan(
    plan: &nocter_mir::MirDestructionPlan,
    owner: MachineLinkageId,
    operation: MirOperationId,
    layouts: &MachineLayoutStore,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
) -> Result<MachineDestructionPlan, MachineProgramError> {
    crate::lower::destruction::lower_destruction(plan, owner, operation, layouts, functions)
}

fn destruction_abi(
    program: &MirProgram,
    layouts: &MachineLayoutStore,
) -> Result<MachineCallableAbi, MachineProgramError> {
    // Concrete source pointee types are deliberately erased here. Every generated cleanup body
    // accepts one byte-address lane plus a byte offset, so pointer primitives and heterogeneous
    // pack state can share the same ordinary call boundary.
    let types = program.executable().types();
    let byte = types.builtin(BuiltinType::U8);
    let pointer = types
        .iter()
        .find_map(|(ty, kind)| (kind == &TypeKind::Pointer(byte)).then_some(ty))
        .ok_or(MachineProgramError::MissingBytePointerType)?;
    crate::transport::plan_signature(
        types,
        layouts,
        &[pointer, types.builtin(BuiltinType::Usize)],
        types.builtin(BuiltinType::Void),
        None,
    )
    .map_err(MachineProgramError::from)
}

fn close_edges<K: Copy + Ord>(
    edges: BTreeMap<K, MachineDestructionPlan>,
    ids: &BTreeMap<MachineDestructionPlan, MachineDestructionId>,
    missing: impl Fn(K) -> MachineProgramError,
) -> Result<BTreeMap<K, MachineDestructionId>, MachineProgramError> {
    edges
        .into_iter()
        .map(|(edge, plan)| {
            ids.get(&plan)
                .copied()
                .map(|destruction| (edge, destruction))
                .ok_or_else(|| missing(edge))
        })
        .collect()
}

fn require_linkage(
    linkage: &MachineLinkageTable,
    key: MachineLinkageKey,
) -> Result<MachineLinkageId, MachineProgramError> {
    linkage
        .id(key)
        .ok_or(MachineProgramError::MissingLinkageKey(key))
}

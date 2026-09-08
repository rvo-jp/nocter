use std::collections::{BTreeMap, BTreeSet};

use nocter_mir::{
    MirBody, MirCallTarget, MirCancellationAction, MirOperationKind, MirPackSegment,
    MirPrimitiveDependency, MirProgram,
};
use nocter_model::{MirBlockId, MirOperationId};
use nocter_runtime_contract::{PrimitiveRole, RuntimePrimitive, RuntimeType};

use crate::identity::{MachineId, MachineTable};
use crate::linkage::MachineLinkagePlan;
use crate::{
    MachineCallableAbi, MachineDestructionId, MachineDestructionPlan, MachineLayoutPlan,
    MachineLinkageId, MachineLinkageKey, MachineProgramError,
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
/// pointer primitive or argument-pack owner. Discovery order never becomes generated identity.
#[derive(Debug)]
pub(crate) struct MachineDestructionPlanTable {
    entries: MachineTable<MachineDestructionId, MachineDestruction>,
    calls: BTreeMap<(MachineLinkageId, MirOperationId), MachineDestructionId>,
    pack_segments:
        BTreeMap<(MachineLinkageId, MirOperationId, usize, PackComponent), MachineDestructionId>,
    async_sites: BTreeMap<(MachineLinkageId, AsyncDestructionSite), MachineDestructionId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum PackComponent {
    Value,
    Key,
    MappedValue,
}

/// Stable source occurrence of a cleanup owned by one deferred body.
///
/// The table closes these occurrences before Machine identities exist, so async-frame lowering can
/// refer to an ordinary generated function without lowering the same destruction recipe again.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum AsyncDestructionSite {
    Initial(usize),
    Suspension { block: MirBlockId, action: usize },
    Completed,
}

impl MachineDestructionPlanTable {
    pub(crate) fn build(
        program: &MirProgram,
        layouts: &MachineLayoutPlan,
        linkage: &MachineLinkagePlan,
        functions: crate::function_domain::MachineFunctionDomain<'_>,
    ) -> Result<Self, MachineProgramError> {
        let mut plans = BTreeSet::new();
        let mut calls = BTreeMap::new();
        let mut pack_segments = BTreeMap::new();
        let mut async_sites = BTreeMap::new();
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
            if let Some(frame) = function.async_frame() {
                collect_async_frame(
                    owner,
                    frame,
                    layouts,
                    functions,
                    &mut plans,
                    &mut async_sites,
                )?;
            }
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
        let pack_segments = close_edges(pack_segments, &ids, |(owner, operation, segment, _)| {
            MachineProgramError::MissingPackDestruction {
                owner,
                operation,
                segment,
            }
        })?;
        let async_sites = close_edges(async_sites, &ids, |(owner, _)| {
            MachineProgramError::MissingAsyncDestruction(owner)
        })?;
        Ok(Self {
            entries: MachineTable::from_values(entries),
            calls,
            pack_segments,
            async_sites,
        })
    }

    #[must_use]
    pub fn get(&self, id: MachineDestructionId) -> Option<&MachineDestruction> {
        self.entries.get(id)
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
        component: PackComponent,
    ) -> Option<MachineDestructionId> {
        self.pack_segments
            .get(&(owner, operation, segment, component))
            .copied()
    }

    #[must_use]
    pub(crate) fn async_site(
        &self,
        owner: MachineLinkageId,
        site: AsyncDestructionSite,
    ) -> Option<MachineDestructionId> {
        self.async_sites.get(&(owner, site)).copied()
    }

    #[must_use]
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineDestructionId, &MachineDestruction)> {
        self.entries.iter()
    }
}

fn collect_async_frame(
    owner: MachineLinkageId,
    frame: &nocter_mir::MirAsyncFrame,
    layouts: &MachineLayoutPlan,
    functions: crate::function_domain::MachineFunctionDomain<'_>,
    plans: &mut BTreeSet<MachineDestructionPlan>,
    sites: &mut BTreeMap<(MachineLinkageId, AsyncDestructionSite), MachineDestructionPlan>,
) -> Result<(), MachineProgramError> {
    collect_async_actions(
        owner,
        frame.initial().cancellation(),
        AsyncDestructionSite::Initial,
        layouts,
        functions,
        plans,
        sites,
    )?;
    for state in frame.states() {
        collect_async_actions(
            owner,
            state.cancellation(),
            |action| AsyncDestructionSite::Suspension {
                block: state.suspend(),
                action,
            },
            layouts,
            functions,
            plans,
            sites,
        )?;
    }
    if let Some(source) = frame.completed_destruction() {
        insert_async_plan(
            owner,
            AsyncDestructionSite::Completed,
            source,
            layouts,
            functions,
            plans,
            sites,
        )?;
    }
    Ok(())
}

fn collect_async_actions(
    owner: MachineLinkageId,
    actions: &[MirCancellationAction],
    site: impl Fn(usize) -> AsyncDestructionSite,
    layouts: &MachineLayoutPlan,
    functions: crate::function_domain::MachineFunctionDomain<'_>,
    plans: &mut BTreeSet<MachineDestructionPlan>,
    sites: &mut BTreeMap<(MachineLinkageId, AsyncDestructionSite), MachineDestructionPlan>,
) -> Result<(), MachineProgramError> {
    for (index, action) in actions.iter().enumerate() {
        let MirCancellationAction::Destroy { plan, .. } = action else {
            continue;
        };
        insert_async_plan(owner, site(index), plan, layouts, functions, plans, sites)?;
    }
    Ok(())
}

fn insert_async_plan(
    owner: MachineLinkageId,
    site: AsyncDestructionSite,
    source: &nocter_mir::MirDestructionPlan,
    layouts: &MachineLayoutPlan,
    functions: crate::function_domain::MachineFunctionDomain<'_>,
    plans: &mut BTreeSet<MachineDestructionPlan>,
    sites: &mut BTreeMap<(MachineLinkageId, AsyncDestructionSite), MachineDestructionPlan>,
) -> Result<(), MachineProgramError> {
    let plan =
        crate::lower::destruction::lower_async_destruction(source, owner, layouts, functions)?;
    if sites.insert((owner, site), plan.clone()).is_some() {
        return Err(MachineProgramError::DuplicateAsyncDestruction(owner));
    }
    plans.insert(plan);
    Ok(())
}

fn collect_body(
    owner: MachineLinkageId,
    body: &MirBody,
    layouts: &MachineLayoutPlan,
    functions: crate::function_domain::MachineFunctionDomain<'_>,
    plans: &mut BTreeSet<MachineDestructionPlan>,
    calls: &mut BTreeMap<(MachineLinkageId, MirOperationId), MachineDestructionPlan>,
    pack_segments: &mut BTreeMap<
        (MachineLinkageId, MirOperationId, usize, PackComponent),
        MachineDestructionPlan,
    >,
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
        if let Some(nocter_mir::MirCallPack::Prepared(pack)) = call.pack() {
            for (segment, source) in pack.segments().iter().enumerate() {
                let destructions = match source {
                    MirPackSegment::Value { destruction, .. } => [
                        (PackComponent::Value, destruction.as_ref()),
                        (PackComponent::Key, None),
                    ],
                    MirPackSegment::KeyedValue {
                        key_destruction,
                        value_destruction,
                        ..
                    } => [
                        (PackComponent::Key, key_destruction.as_ref()),
                        (PackComponent::MappedValue, value_destruction.as_ref()),
                    ],
                    MirPackSegment::Spread(spread) => [
                        (PackComponent::Value, spread.destruction()),
                        (PackComponent::Key, None),
                    ],
                };
                for (component, destruction) in destructions {
                    let Some(destruction) = destruction else {
                        continue;
                    };
                    let plan = lower_plan(destruction, owner, operation, layouts, functions)?;
                    if pack_segments
                        .insert((owner, operation, segment, component), plan.clone())
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
    }
    Ok(())
}

fn lower_plan(
    plan: &nocter_mir::MirDestructionPlan,
    owner: MachineLinkageId,
    operation: MirOperationId,
    layouts: &MachineLayoutPlan,
    functions: crate::function_domain::MachineFunctionDomain<'_>,
) -> Result<MachineDestructionPlan, MachineProgramError> {
    crate::lower::destruction::lower_destruction(plan, owner, operation, layouts, functions)
}

fn destruction_abi(
    program: &MirProgram,
    layouts: &MachineLayoutPlan,
) -> Result<MachineCallableAbi, MachineProgramError> {
    // Concrete source pointee types are deliberately erased here. Every generated cleanup body
    // accepts one byte-address lane plus a byte offset, so pointer primitives and heterogeneous
    // pack state can share the same ordinary call boundary.
    let types = program.types();
    let byte = types.primitive(RuntimePrimitive::Unsigned(8)).ok_or(
        MachineProgramError::MissingRuntimePrimitive(RuntimePrimitive::Unsigned(8)),
    )?;
    let pointer = types
        .iter()
        .find_map(|(ty, kind)| (kind == &RuntimeType::Pointer(byte)).then_some(ty))
        .ok_or(MachineProgramError::MissingBytePointerType)?;
    let usize_ = types.primitive(RuntimePrimitive::Usize).ok_or(
        MachineProgramError::MissingRuntimePrimitive(RuntimePrimitive::Usize),
    )?;
    let void = types.primitive(RuntimePrimitive::Void).ok_or(
        MachineProgramError::MissingRuntimePrimitive(RuntimePrimitive::Void),
    )?;
    crate::transport::plan_signature(types, layouts, &[pointer, usize_], void, None)
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
    linkage: &MachineLinkagePlan,
    key: MachineLinkageKey,
) -> Result<MachineLinkageId, MachineProgramError> {
    linkage
        .id(key)
        .ok_or(MachineProgramError::MissingLinkageKey(key))
}

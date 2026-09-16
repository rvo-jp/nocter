use std::collections::BTreeMap;

use nocter_mir::{MirBody, MirOperationKind, MirProgram};
use nocter_model::{CallableCapability, ExecutableItemId, MirOperationId, TypeId};

use crate::identity::{MachineErasedAdapterId, MachineId, MachineTable};
use crate::{
    MachineCallableAbi, MachineDestructionId, MachineLayoutPlan, MachineLinkageId,
    MachineLinkageKey, MachineProgramError,
};

/// One consuming erased-call adapter selected before source bodies become Machine operations.
#[derive(Clone, Debug)]
pub(crate) struct MachineErasedAdapter {
    target: ExecutableItemId,
    environment: TypeId,
    target_environment: TypeId,
    source_capability: CallableCapability,
    destruction: Option<MachineDestructionId>,
    mapped_size: u64,
    abi: MachineCallableAbi,
}

impl MachineErasedAdapter {
    pub(crate) const fn target(&self) -> ExecutableItemId {
        self.target
    }

    pub(crate) const fn environment(&self) -> TypeId {
        self.environment
    }

    pub(crate) const fn target_environment(&self) -> TypeId {
        self.target_environment
    }

    pub(crate) const fn source_capability(&self) -> CallableCapability {
        self.source_capability
    }

    pub(crate) const fn destruction(&self) -> Option<MachineDestructionId> {
        self.destruction
    }

    pub(crate) const fn mapped_size(&self) -> u64 {
        self.mapped_size
    }

    pub(crate) const fn abi(&self) -> &MachineCallableAbi {
        &self.abi
    }
}

/// Dense adapter identities and their exact owning erasure sites.
pub(crate) struct MachineErasedAdapterPlan {
    entries: MachineTable<MachineErasedAdapterId, MachineErasedAdapter>,
    sites: BTreeMap<(MachineLinkageId, MirOperationId), MachineErasedAdapterId>,
}

impl MachineErasedAdapterPlan {
    pub(crate) fn build(
        program: &MirProgram,
        layouts: &MachineLayoutPlan,
        linkage: &crate::linkage::MachineLinkagePlan,
        destructions: &crate::destruction_table::MachineDestructionPlanTable,
    ) -> Result<Self, MachineProgramError> {
        let mut drafts = BTreeMap::new();
        for (item, function) in program.functions().iter() {
            collect_body(
                require_linkage(linkage, MachineLinkageKey::Item(item))?,
                function.body(),
                program,
                layouts,
                destructions,
                &mut drafts,
            )?;
        }
        match program.root() {
            nocter_mir::MirRoot::Process(root) => collect_body(
                require_linkage(linkage, MachineLinkageKey::ProcessRoot(root.target()))?,
                root.body(),
                program,
                layouts,
                destructions,
                &mut drafts,
            )?,
            nocter_mir::MirRoot::Tests { cases, .. } => {
                for case in cases {
                    collect_body(
                        require_linkage(linkage, MachineLinkageKey::TestRoot(case.declaration()))?,
                        case.body(),
                        program,
                        layouts,
                        destructions,
                        &mut drafts,
                    )?;
                }
            }
        }

        let mut sites = BTreeMap::new();
        let entries = drafts
            .into_iter()
            .enumerate()
            .map(|(index, (site, adapter))| {
                let id = MachineErasedAdapterId::new(index);
                sites.insert(site, id);
                adapter
            })
            .collect::<Vec<_>>();
        Ok(Self {
            entries: MachineTable::from_values(entries),
            sites,
        })
    }

    pub(crate) fn at(
        &self,
        owner: MachineLinkageId,
        operation: MirOperationId,
    ) -> Option<MachineErasedAdapterId> {
        self.sites.get(&(owner, operation)).copied()
    }

    pub(crate) fn get(&self, id: MachineErasedAdapterId) -> Option<&MachineErasedAdapter> {
        self.entries.get(id)
    }

    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineErasedAdapterId, &MachineErasedAdapter)> {
        self.entries.iter()
    }
}

fn collect_body(
    owner: MachineLinkageId,
    body: &MirBody,
    program: &MirProgram,
    layouts: &MachineLayoutPlan,
    destructions: &crate::destruction_table::MachineDestructionPlanTable,
    drafts: &mut BTreeMap<(MachineLinkageId, MirOperationId), MachineErasedAdapter>,
) -> Result<(), MachineProgramError> {
    for (operation, value) in body.operations().iter() {
        let MirOperationKind::EraseCallable(erasure) = value.kind() else {
            continue;
        };
        if erasure.capability() != CallableCapability::Owned {
            continue;
        }
        let target = program
            .functions()
            .get(erasure.body())
            .ok_or(MachineProgramError::MissingItem(erasure.body()))?;
        let target_environment = target
            .parameters()
            .first()
            .and_then(|local| target.locals().get(*local))
            .copied()
            .map(nocter_mir::MirLocal::ty)
            .ok_or(MachineProgramError::InvalidErasedAdapter(owner, operation))?;
        let environment_layout = layouts.get(erasure.environment_ty()).ok_or(
            MachineProgramError::MissingStoredLayout(erasure.environment_ty()),
        )?;
        let destruction = if erasure.source_capability() == CallableCapability::Owned {
            None
        } else {
            erasure
                .environment_destruction()
                .map(|_| {
                    destructions.erased_environment(owner, operation).ok_or(
                        MachineProgramError::MissingGeneratedDestruction(owner, operation),
                    )
                })
                .transpose()?
        };
        let adapter = MachineErasedAdapter {
            target: erasure.body(),
            environment: erasure.environment_ty(),
            target_environment,
            source_capability: erasure.source_capability(),
            destruction,
            mapped_size: environment_layout.size().max(1),
            abi: crate::transport::plan_erased_signature(
                erasure.signature(),
                program.types(),
                layouts,
            )?,
        };
        if drafts.insert((owner, operation), adapter).is_some() {
            return Err(MachineProgramError::DuplicateErasedAdapter(
                owner, operation,
            ));
        }
    }
    Ok(())
}

fn require_linkage(
    linkage: &crate::linkage::MachineLinkagePlan,
    key: MachineLinkageKey,
) -> Result<MachineLinkageId, MachineProgramError> {
    linkage
        .id(key)
        .ok_or(MachineProgramError::MissingLinkageKey(key))
}

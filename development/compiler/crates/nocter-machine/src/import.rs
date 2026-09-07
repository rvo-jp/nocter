use nocter_mir::{MirBody, MirCallTarget, MirOperationKind, MirProgram, MirRoot};
use nocter_runtime_contract::TargetServiceDescriptor;

use crate::identity::{MachineId, MachineImportId, MachineTable};

/// Temporary canonical target-service import domain built once from closed MIR call plans.
#[derive(Debug)]
pub(crate) struct MachineImportPlan {
    descriptors: Vec<TargetServiceDescriptor>,
}

impl MachineImportPlan {
    #[must_use]
    pub(crate) fn build(program: &MirProgram) -> Self {
        let mut descriptors = Vec::new();
        for (_, function) in program.functions().iter() {
            collect_body(function.body(), &mut descriptors);
        }
        match program.root() {
            MirRoot::Process(root) => collect_body(root.body(), &mut descriptors),
            MirRoot::Tests { cases, .. } => {
                for case in cases {
                    collect_body(case.body(), &mut descriptors);
                }
            }
        }
        Self { descriptors }
    }

    pub(crate) fn id(&self, descriptor: &TargetServiceDescriptor) -> Option<MachineImportId> {
        self.descriptors
            .iter()
            .position(|candidate| candidate == descriptor)
            .map(MachineImportId::new)
    }

    pub(crate) fn finish(self) -> MachineImportTable {
        MachineImportTable {
            descriptors: MachineTable::from_values(self.descriptors),
        }
    }
}

fn collect_body(body: &MirBody, descriptors: &mut Vec<TargetServiceDescriptor>) {
    for (_, operation) in body.operations().iter() {
        let MirOperationKind::Call(call) = operation.kind() else {
            continue;
        };
        let MirCallTarget::TargetService { descriptor, .. } = call.target() else {
            continue;
        };
        if !descriptors.contains(descriptor) {
            descriptors.push(descriptor.clone());
        }
    }
}

/// Final source-independent target-service import authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MachineImportTable {
    descriptors: MachineTable<MachineImportId, TargetServiceDescriptor>,
}

impl MachineImportTable {
    pub(crate) fn get(&self, id: MachineImportId) -> Option<&TargetServiceDescriptor> {
        self.descriptors.get(id)
    }

    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineImportId, &TargetServiceDescriptor)> {
        self.descriptors.iter()
    }
}

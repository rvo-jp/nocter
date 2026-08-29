use std::collections::BTreeSet;

use nocter_model::{ConstructionId, DropId, InstanceId, InterfaceImplementationId};

/// Exact declaration containers admitted by declaration validation for editor-only analysis.
///
/// This value contains decisions; it does not evaluate declaration rules. The validation pass is
/// its only constructor, so recovery and checking cannot independently reinterpret a rejected
/// declaration graph.
#[derive(Clone, Debug)]
pub struct DeclarationAnalysisAdmission {
    constructions: BTreeSet<ConstructionId>,
    instances: BTreeSet<InstanceId>,
    inherent_instances: BTreeSet<InstanceId>,
    interface_implementations: BTreeSet<InterfaceImplementationId>,
    drops: BTreeSet<DropId>,
}

impl DeclarationAnalysisAdmission {
    pub(crate) fn new(
        constructions: BTreeSet<ConstructionId>,
        instances: BTreeSet<InstanceId>,
        inherent_instances: BTreeSet<InstanceId>,
        interface_implementations: BTreeSet<InterfaceImplementationId>,
        drops: BTreeSet<DropId>,
    ) -> Self {
        Self {
            constructions,
            instances,
            inherent_instances,
            interface_implementations,
            drops,
        }
    }

    #[must_use]
    pub fn admits_construction(&self, declaration: ConstructionId) -> bool {
        self.constructions.contains(&declaration)
    }

    #[must_use]
    pub fn admits_instance(&self, declaration: InstanceId) -> bool {
        self.instances.contains(&declaration)
    }

    /// Returns whether this instance may contribute ordinary inherent operations.
    ///
    /// An interface-owned instance remains available to interface implementation checking, but
    /// its methods cannot leak into the target type's inherent method namespace.
    #[must_use]
    pub fn admits_inherent_instance(&self, declaration: InstanceId) -> bool {
        self.inherent_instances.contains(&declaration)
    }

    #[must_use]
    pub fn admits_interface_implementation(&self, declaration: InterfaceImplementationId) -> bool {
        self.interface_implementations.contains(&declaration)
    }

    #[must_use]
    pub fn admits_drop(&self, declaration: DropId) -> bool {
        self.drops.contains(&declaration)
    }
}

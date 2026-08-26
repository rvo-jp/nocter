use nocter_declarations::{DeclarationAnalysisAdmission, DeclarationGraph};
use nocter_model::{ConstructionId, DropId, InstanceId, InterfaceImplementationId};

/// Frozen declaration identities allowed to participate in program-wide operation authorities.
///
/// Accepted and editor-analysis programs are normalized into this same contract once. Individual
/// table builders never interpret an optional recovery mode and cannot accidentally process a
/// quarantined declaration.
pub(crate) struct AdmittedOperations {
    constructions: Box<[ConstructionId]>,
    instances: Box<[InstanceId]>,
    interface_implementations: Box<[InterfaceImplementationId]>,
    drops: Box<[DropId]>,
}

impl AdmittedOperations {
    pub(crate) fn new(graph: &DeclarationGraph, admission: &DeclarationAnalysisAdmission) -> Self {
        let declarations = graph.declarations();
        Self {
            constructions: declarations
                .constructions()
                .iter()
                .map(|(id, _)| id)
                .filter(|id| admission.admits_construction(*id))
                .collect(),
            instances: declarations
                .instances()
                .iter()
                .map(|(id, _)| id)
                .filter(|id| admission.admits_inherent_instance(*id))
                .collect(),
            interface_implementations: declarations
                .interface_implementations()
                .iter()
                .map(|(id, _)| id)
                .filter(|id| admission.admits_interface_implementation(*id))
                .collect(),
            drops: declarations
                .drops()
                .iter()
                .map(|(id, _)| id)
                .filter(|id| admission.admits_drop(*id))
                .collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn all(graph: &DeclarationGraph) -> Self {
        let declarations = graph.declarations();
        Self {
            constructions: declarations
                .constructions()
                .iter()
                .map(|(id, _)| id)
                .collect(),
            instances: declarations.instances().iter().map(|(id, _)| id).collect(),
            interface_implementations: declarations
                .interface_implementations()
                .iter()
                .map(|(id, _)| id)
                .collect(),
            drops: declarations.drops().iter().map(|(id, _)| id).collect(),
        }
    }

    pub(crate) const fn constructions(&self) -> &[ConstructionId] {
        &self.constructions
    }

    pub(crate) const fn instances(&self) -> &[InstanceId] {
        &self.instances
    }

    pub(crate) const fn interface_implementations(&self) -> &[InterfaceImplementationId] {
        &self.interface_implementations
    }

    pub(crate) const fn drops(&self) -> &[DropId] {
        &self.drops
    }
}

use nocter_declarations::{DeclarationAnalysisAdmission, DeclarationGraph};
use nocter_model::{ConformanceId, ConstructionId, DropId, InstanceId};

/// Frozen declaration identities allowed to participate in program-wide operation authorities.
///
/// Accepted and editor-analysis programs are normalized into this same contract once. Individual
/// table builders never interpret an optional recovery mode and cannot accidentally process a
/// quarantined declaration.
pub(crate) struct AdmittedOperations {
    constructions: Box<[ConstructionId]>,
    instances: Box<[InstanceId]>,
    conformances: Box<[ConformanceId]>,
    drops: Box<[DropId]>,
}

impl AdmittedOperations {
    pub(crate) fn new(
        graph: &DeclarationGraph,
        admission: Option<&DeclarationAnalysisAdmission>,
    ) -> Self {
        let declarations = graph.declarations();
        Self {
            constructions: declarations
                .constructions()
                .iter()
                .map(|(id, _)| id)
                .filter(|id| admission.is_none_or(|admission| admission.admits_construction(*id)))
                .collect(),
            instances: declarations
                .instances()
                .iter()
                .map(|(id, _)| id)
                .filter(|id| admission.is_none_or(|admission| admission.admits_instance(*id)))
                .collect(),
            conformances: declarations
                .conformances()
                .iter()
                .map(|(id, _)| id)
                .filter(|id| admission.is_none_or(|admission| admission.admits_conformance(*id)))
                .collect(),
            drops: declarations
                .drops()
                .iter()
                .map(|(id, _)| id)
                .filter(|id| admission.is_none_or(|admission| admission.admits_drop(*id)))
                .collect(),
        }
    }

    pub(crate) const fn constructions(&self) -> &[ConstructionId] {
        &self.constructions
    }

    pub(crate) const fn instances(&self) -> &[InstanceId] {
        &self.instances
    }

    pub(crate) const fn conformances(&self) -> &[ConformanceId] {
        &self.conformances
    }

    pub(crate) const fn drops(&self) -> &[DropId] {
        &self.drops
    }
}

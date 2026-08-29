use crate::body_check::{BodyAssumptionTable, CapabilityEvidenceTable};
use crate::{
    ConstructionSurfaceTable, DropTable, InstanceOperationTable, InterfaceImplementationTable,
    StandardSemanticTable,
};
use nocter_declarations::DeclarationGraph;
use std::sync::Arc;

/// Immutable, source-neutral program facts whose identities stay valid across every descendant
/// type branch.
///
/// Prepared, recovery, body-checking, and checked products move this value intact. Adding another
/// program-wide authority therefore changes one owner instead of parallel phase-specific structs.
#[derive(Clone, Debug)]
pub(crate) struct ProgramEnvironment {
    graph: Arc<DeclarationGraph>,
    interface_implementations: Arc<InterfaceImplementationTable>,
    construction_surfaces: Arc<ConstructionSurfaceTable>,
    instance_operations: Arc<InstanceOperationTable>,
    body_assumptions: Arc<BodyAssumptionTable>,
    capability_evidence: Arc<CapabilityEvidenceTable>,
    drops: Arc<DropTable>,
    standard_semantics: Arc<StandardSemanticTable>,
}

impl ProgramEnvironment {
    // This is the single aggregate-construction boundary for independent program-wide facts.
    // Hiding the fields behind an artificial `Parts` value would only move the same invariant and
    // introduce a second representation of `ProgramEnvironment`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        graph: DeclarationGraph,
        interface_implementations: InterfaceImplementationTable,
        construction_surfaces: ConstructionSurfaceTable,
        instance_operations: InstanceOperationTable,
        body_assumptions: BodyAssumptionTable,
        capability_evidence: CapabilityEvidenceTable,
        drops: DropTable,
        standard_semantics: StandardSemanticTable,
    ) -> Self {
        Self {
            graph: Arc::new(graph),
            interface_implementations: Arc::new(interface_implementations),
            construction_surfaces: Arc::new(construction_surfaces),
            instance_operations: Arc::new(instance_operations),
            body_assumptions: Arc::new(body_assumptions),
            capability_evidence: Arc::new(capability_evidence),
            drops: Arc::new(drops),
            standard_semantics: Arc::new(standard_semantics),
        }
    }

    /// Opens a current body branch without cloning or rebuilding program-wide authorities.
    pub(crate) fn with_checking_symbols<S>(&self, spellings: impl IntoIterator<Item = S>) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            graph: Arc::new(self.graph.with_checking_symbols(spellings)),
            interface_implementations: Arc::clone(&self.interface_implementations),
            construction_surfaces: Arc::clone(&self.construction_surfaces),
            instance_operations: Arc::clone(&self.instance_operations),
            body_assumptions: Arc::clone(&self.body_assumptions),
            capability_evidence: Arc::clone(&self.capability_evidence),
            drops: Arc::clone(&self.drops),
            standard_semantics: Arc::clone(&self.standard_semantics),
        }
    }

    pub(crate) fn graph(&self) -> &DeclarationGraph {
        &self.graph
    }
    pub(crate) fn interface_implementations(&self) -> &InterfaceImplementationTable {
        &self.interface_implementations
    }
    pub(crate) fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        &self.construction_surfaces
    }
    pub(crate) fn instance_operations(&self) -> &InstanceOperationTable {
        &self.instance_operations
    }
    pub(crate) fn body_assumptions(&self) -> &BodyAssumptionTable {
        &self.body_assumptions
    }
    pub(crate) fn capability_evidence(&self) -> &CapabilityEvidenceTable {
        &self.capability_evidence
    }
    pub(crate) fn drops(&self) -> &DropTable {
        &self.drops
    }
    pub(crate) fn standard_semantics(&self) -> &StandardSemanticTable {
        &self.standard_semantics
    }
}

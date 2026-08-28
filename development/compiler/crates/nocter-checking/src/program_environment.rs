use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::SourceAccessTable;

use crate::body_check::{BodyAssumptionTable, CapabilityEvidenceTable};
use crate::{
    ConstructionSurfaceTable, DropTable, InstanceOperationTable, InterfaceImplementationTable,
    StandardSemanticTable,
};

/// Immutable program-wide facts whose identities stay valid across every descendant type branch.
///
/// Prepared, recovery, body-checking, and checked products move this value intact. Adding another
/// program-wide authority therefore changes one owner instead of parallel phase-specific structs.
#[derive(Debug)]
pub(crate) struct ProgramEnvironment {
    graph: DeclarationGraph,
    interface_implementations: InterfaceImplementationTable,
    construction_surfaces: ConstructionSurfaceTable,
    instance_operations: InstanceOperationTable,
    body_assumptions: BodyAssumptionTable,
    capability_evidence: CapabilityEvidenceTable,
    drops: DropTable,
    standard_semantics: StandardSemanticTable,
    source_access: SourceAccessTable,
}

impl ProgramEnvironment {
    // This is the single aggregate-construction boundary for independent program-wide facts.
    // Hiding the fields behind an artificial `Parts` value would only move the same invariant and
    // introduce a second representation of `ProgramEnvironment`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        graph: DeclarationGraph,
        interface_implementations: InterfaceImplementationTable,
        construction_surfaces: ConstructionSurfaceTable,
        instance_operations: InstanceOperationTable,
        body_assumptions: BodyAssumptionTable,
        capability_evidence: CapabilityEvidenceTable,
        drops: DropTable,
        standard_semantics: StandardSemanticTable,
        source_access: SourceAccessTable,
    ) -> Self {
        Self {
            graph,
            interface_implementations,
            construction_surfaces,
            instance_operations,
            body_assumptions,
            capability_evidence,
            drops,
            standard_semantics,
            source_access,
        }
    }

    pub(crate) const fn graph(&self) -> &DeclarationGraph {
        &self.graph
    }
    pub(crate) const fn interface_implementations(&self) -> &InterfaceImplementationTable {
        &self.interface_implementations
    }
    pub(crate) const fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        &self.construction_surfaces
    }
    pub(crate) const fn instance_operations(&self) -> &InstanceOperationTable {
        &self.instance_operations
    }
    pub(crate) const fn body_assumptions(&self) -> &BodyAssumptionTable {
        &self.body_assumptions
    }
    pub(crate) const fn capability_evidence(&self) -> &CapabilityEvidenceTable {
        &self.capability_evidence
    }
    pub(crate) const fn drops(&self) -> &DropTable {
        &self.drops
    }
    pub(crate) const fn standard_semantics(&self) -> &StandardSemanticTable {
        &self.standard_semantics
    }
    pub(crate) const fn source_access(&self) -> &SourceAccessTable {
        &self.source_access
    }
}

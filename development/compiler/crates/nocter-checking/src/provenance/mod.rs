mod analysis;
mod contract;
mod state;

pub(crate) use contract::{invocation_place_can_reach_result, type_can_carry_loan};

use nocter_declarations::DeclarationGraph;
use nocter_model::TypeStore;

use crate::{
    BodyRelationError, ClosureTable, InterfaceImplementationTable, ProvenanceTable,
    body_relations::BodyRelationCatalog,
};

pub(crate) fn analyze_program_provenance(
    graph: &DeclarationGraph,
    types: &TypeStore,
    capability_evidence: &crate::body_check::CapabilityEvidenceTable,
    interface_implementations: &InterfaceImplementationTable,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_, '_>,
) -> Result<ProvenanceTable, BodyRelationError> {
    analysis::analyze_program(
        graph,
        types,
        capability_evidence,
        interface_implementations,
        closures,
        inputs,
    )
}

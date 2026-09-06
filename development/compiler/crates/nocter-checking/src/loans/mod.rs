mod analysis;
mod liveness;
mod state;
mod value;

use nocter_declarations::DeclarationGraph;
use nocter_model::TypeStore;

use crate::{
    BodyCheckError, ClosureTable, DropTable, LoanTable, ProvenanceTable,
    body_relations::BodyRelationCatalog,
};

pub(crate) fn analyze_program_loans(
    graph: &DeclarationGraph,
    types: &TypeStore,
    capability_evidence: &crate::body_check::CapabilityEvidenceTable,
    drops: &DropTable,
    provenance: &ProvenanceTable,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_, '_>,
) -> Result<LoanTable, BodyCheckError> {
    analysis::analyze_program(
        graph,
        types,
        capability_evidence,
        drops,
        provenance,
        closures,
        inputs,
    )
}

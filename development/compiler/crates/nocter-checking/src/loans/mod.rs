mod analysis;
mod liveness;
mod state;
mod value;

use std::collections::HashMap;

use nocter_declarations::DeclarationGraph;
use nocter_model::{BodyNodeId, TypeStore};
use nocter_source_index::SourceOrigin;

use crate::{
    BodyCheckError, BodySource, CheckedBody, ClosureTable, DropTable, LoanTable, ProvenanceTable,
};

pub(crate) struct LoanBodyInput<'program, 'syntax> {
    source: BodySource<'syntax>,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, SourceOrigin>,
}

impl<'program, 'syntax> LoanBodyInput<'program, 'syntax> {
    pub(crate) const fn new(
        source: BodySource<'syntax>,
        body: &'program CheckedBody,
        origins: &'program HashMap<BodyNodeId, SourceOrigin>,
    ) -> Self {
        Self {
            source,
            body,
            origins,
        }
    }
}

pub(crate) fn analyze_program_loans(
    graph: &DeclarationGraph,
    types: &TypeStore,
    capability_evidence: &crate::body_check::CapabilityEvidenceTable,
    drops: &DropTable,
    provenance: &ProvenanceTable,
    closures: &ClosureTable,
    inputs: &[LoanBodyInput<'_, '_>],
) -> Result<LoanTable, BodyCheckError> {
    let inputs = inputs
        .iter()
        .map(|input| analysis::LoanBodyInput::new(input.source, input.body, input.origins))
        .collect::<Vec<_>>();
    analysis::analyze_program(
        graph,
        types,
        capability_evidence,
        drops,
        provenance,
        closures,
        &inputs,
    )
}

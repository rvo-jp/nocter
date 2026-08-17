mod analysis;
mod state;

use std::collections::HashMap;

use nocter_declarations::DeclarationGraph;
use nocter_model::{BodyId, BodyNodeId, TypeStore};
use nocter_source_index::SourceOrigin;

use crate::{BodyCheckError, BodySource, CheckedBody, ConformanceTable, ProvenanceTable};

pub(crate) struct ProvenanceBodyInput<'program, 'syntax> {
    source: BodySource<'syntax>,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, SourceOrigin>,
}

impl<'program, 'syntax> ProvenanceBodyInput<'program, 'syntax> {
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

    const fn source(&self) -> BodySource<'syntax> {
        self.source
    }

    const fn body(&self) -> &'program CheckedBody {
        self.body
    }

    const fn origins(&self) -> &'program HashMap<BodyNodeId, SourceOrigin> {
        self.origins
    }
}

pub(crate) fn analyze_program_provenance(
    graph: &DeclarationGraph,
    types: &TypeStore,
    conformances: &ConformanceTable,
    inputs: &[ProvenanceBodyInput<'_, '_>],
) -> Result<ProvenanceTable, BodyCheckError> {
    analysis::analyze_program(graph, types, conformances, inputs)
}

fn input_for_body<'a, 'syntax>(
    inputs: &'a [ProvenanceBodyInput<'a, 'syntax>],
    body: BodyId,
) -> Option<&'a ProvenanceBodyInput<'a, 'syntax>> {
    inputs.iter().find(|input| input.source().body() == body)
}

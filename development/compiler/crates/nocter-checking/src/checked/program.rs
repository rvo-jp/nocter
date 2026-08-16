use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source_index::SourceIndex;

use crate::ConformanceTable;

use super::CheckedBody;

/// Complete syntax-independent Phase 3 program.
#[derive(Debug)]
pub struct CheckedProgram {
    graph: DeclarationGraph,
    types: TypeStore,
    conformances: ConformanceTable,
    bodies: Arena<BodyId, CheckedBody>,
}

impl CheckedProgram {
    #[must_use]
    pub const fn graph(&self) -> &DeclarationGraph {
        &self.graph
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn conformances(&self) -> &ConformanceTable {
        &self.conformances
    }

    #[must_use]
    pub const fn bodies(&self) -> &Arena<BodyId, CheckedBody> {
        &self.bodies
    }
}

/// Checked semantics and its independent source projection.
#[derive(Debug)]
pub struct CheckedProgramOutput {
    program: CheckedProgram,
    source_index: SourceIndex,
}

impl CheckedProgramOutput {
    #[must_use]
    pub const fn program(&self) -> &CheckedProgram {
        &self.program
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (CheckedProgram, SourceIndex) {
        (self.program, self.source_index)
    }
}

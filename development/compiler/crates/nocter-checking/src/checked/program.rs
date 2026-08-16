use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source_index::SourceIndex;

use crate::{ConformanceTable, CopyabilityTable, DropTable, InstanceOperationTable};

use super::CheckedBody;

/// Complete syntax-independent Phase 3 program.
#[derive(Debug)]
pub struct CheckedProgram {
    graph: DeclarationGraph,
    types: TypeStore,
    conformances: ConformanceTable,
    instance_operations: InstanceOperationTable,
    copyabilities: CopyabilityTable,
    drops: DropTable,
    bodies: Arena<BodyId, CheckedBody>,
}

impl CheckedProgram {
    pub(crate) const fn new(
        graph: DeclarationGraph,
        types: TypeStore,
        conformances: ConformanceTable,
        instance_operations: InstanceOperationTable,
        copyabilities: CopyabilityTable,
        drops: DropTable,
        bodies: Arena<BodyId, CheckedBody>,
    ) -> Self {
        Self {
            graph,
            types,
            conformances,
            instance_operations,
            copyabilities,
            drops,
            bodies,
        }
    }

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
    pub const fn instance_operations(&self) -> &InstanceOperationTable {
        &self.instance_operations
    }

    #[must_use]
    pub const fn copyabilities(&self) -> &CopyabilityTable {
        &self.copyabilities
    }

    #[must_use]
    pub const fn drops(&self) -> &DropTable {
        &self.drops
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
    pub(crate) const fn new(program: CheckedProgram, source_index: SourceIndex) -> Self {
        Self {
            program,
            source_index,
        }
    }

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

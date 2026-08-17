use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source_index::SourceIndex;

use crate::{
    ConformanceTable, ConstructionSurfaceTable, CopyabilityTable, DropTable,
    InstanceOperationTable, LoanTable, ProvenanceTable,
};

use super::CheckedBody;

/// Complete syntax-independent Phase 3 program.
#[derive(Debug)]
pub struct CheckedProgram {
    graph: DeclarationGraph,
    types: TypeStore,
    conformances: ConformanceTable,
    construction_surfaces: ConstructionSurfaceTable,
    instance_operations: InstanceOperationTable,
    copyabilities: CopyabilityTable,
    drops: DropTable,
    provenance: ProvenanceTable,
    loans: LoanTable,
    bodies: Arena<BodyId, CheckedBody>,
}

pub(crate) struct CheckedProgramAuthorities {
    pub(crate) conformances: ConformanceTable,
    pub(crate) construction_surfaces: ConstructionSurfaceTable,
    pub(crate) instance_operations: InstanceOperationTable,
    pub(crate) copyabilities: CopyabilityTable,
    pub(crate) drops: DropTable,
    pub(crate) provenance: ProvenanceTable,
    pub(crate) loans: LoanTable,
}

impl CheckedProgram {
    pub(crate) fn new(
        graph: DeclarationGraph,
        types: TypeStore,
        authorities: CheckedProgramAuthorities,
        bodies: Arena<BodyId, CheckedBody>,
    ) -> Self {
        Self {
            graph,
            types,
            conformances: authorities.conformances,
            construction_surfaces: authorities.construction_surfaces,
            instance_operations: authorities.instance_operations,
            copyabilities: authorities.copyabilities,
            drops: authorities.drops,
            provenance: authorities.provenance,
            loans: authorities.loans,
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
    pub const fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        &self.construction_surfaces
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
    pub const fn provenance(&self) -> &ProvenanceTable {
        &self.provenance
    }

    #[must_use]
    pub const fn loans(&self) -> &LoanTable {
        &self.loans
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

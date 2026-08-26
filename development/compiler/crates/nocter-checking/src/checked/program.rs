use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::SourceAccessTable;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source::SourceId;
use nocter_source_index::SourceIndex;

use crate::declaration_patterns::DeclarationPatternTable;
use crate::{
    AssociatedTypeCompletionContext, ClosureTable, ConstructionSurfaceTable, CopyabilityTable,
    DropTable, InstanceOperationTable, InterfaceImplementationTable, LoanTable, ProvenanceTable,
    StandardSemanticTable,
};

use super::{CheckedBody, OpaqueWitnessTable};

/// Complete syntax-independent Phase 3 program.
#[derive(Debug)]
pub struct CheckedProgram {
    graph: DeclarationGraph,
    types: TypeStore,
    interface_implementations: InterfaceImplementationTable,
    construction_surfaces: ConstructionSurfaceTable,
    instance_operations: InstanceOperationTable,
    declaration_patterns: DeclarationPatternTable,
    copyabilities: CopyabilityTable,
    drops: DropTable,
    standard_semantics: StandardSemanticTable,
    provenance: ProvenanceTable,
    loans: LoanTable,
    closures: ClosureTable,
    opaque_witnesses: OpaqueWitnessTable,
    bodies: Arena<BodyId, CheckedBody>,
    associated_type_completion_contexts: Box<[AssociatedTypeCompletionContext]>,
    source_access: SourceAccessTable,
}

pub(crate) struct CheckedProgramAuthorities {
    pub(crate) interface_implementations: InterfaceImplementationTable,
    pub(crate) construction_surfaces: ConstructionSurfaceTable,
    pub(crate) instance_operations: InstanceOperationTable,
    pub(crate) declaration_patterns: DeclarationPatternTable,
    pub(crate) copyabilities: CopyabilityTable,
    pub(crate) drops: DropTable,
    pub(crate) standard_semantics: StandardSemanticTable,
    pub(crate) provenance: ProvenanceTable,
    pub(crate) loans: LoanTable,
    pub(crate) closures: ClosureTable,
    pub(crate) opaque_witnesses: OpaqueWitnessTable,
    pub(crate) associated_type_completion_contexts: Box<[AssociatedTypeCompletionContext]>,
    pub(crate) source_access: SourceAccessTable,
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
            interface_implementations: authorities.interface_implementations,
            construction_surfaces: authorities.construction_surfaces,
            instance_operations: authorities.instance_operations,
            declaration_patterns: authorities.declaration_patterns,
            copyabilities: authorities.copyabilities,
            drops: authorities.drops,
            standard_semantics: authorities.standard_semantics,
            provenance: authorities.provenance,
            loans: authorities.loans,
            closures: authorities.closures,
            opaque_witnesses: authorities.opaque_witnesses,
            bodies,
            associated_type_completion_contexts: authorities.associated_type_completion_contexts,
            source_access: authorities.source_access,
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
    pub const fn interface_implementations(&self) -> &InterfaceImplementationTable {
        &self.interface_implementations
    }

    #[must_use]
    pub(crate) const fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        &self.construction_surfaces
    }

    #[must_use]
    pub const fn instance_operations(&self) -> &InstanceOperationTable {
        &self.instance_operations
    }

    pub(crate) const fn declaration_patterns(&self) -> &DeclarationPatternTable {
        &self.declaration_patterns
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
    pub const fn standard_semantics(&self) -> &StandardSemanticTable {
        &self.standard_semantics
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
    pub const fn closures(&self) -> &ClosureTable {
        &self.closures
    }

    #[must_use]
    pub(crate) const fn source_access(&self) -> &SourceAccessTable {
        &self.source_access
    }

    /// Creates the semantic visibility contract for one exact source in this checked program.
    ///
    /// # Errors
    ///
    /// Returns an error when declaration lowering did not publish the source's semantic module.
    pub fn source_access_context(
        &self,
        source: SourceId,
    ) -> Result<crate::SourceAccessContext<'_>, crate::SourceVisibilityError> {
        crate::SourceAccessContext::for_source(&self.source_access, source)
            .map_err(crate::SourceVisibilityError::Access)
    }

    #[must_use]
    pub const fn opaque_witnesses(&self) -> &OpaqueWitnessTable {
        &self.opaque_witnesses
    }

    #[must_use]
    pub const fn bodies(&self) -> &Arena<BodyId, CheckedBody> {
        &self.bodies
    }

    #[must_use]
    pub const fn associated_type_completion_contexts(&self) -> &[AssociatedTypeCompletionContext] {
        &self.associated_type_completion_contexts
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
    pub const fn new(program: CheckedProgram, source_index: SourceIndex) -> Self {
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

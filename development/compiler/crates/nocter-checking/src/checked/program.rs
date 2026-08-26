use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::SourceAccessTable;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source::SourceId;
use nocter_source_index::SourceIndex;

use crate::body_check::BodyAssumptionTable;
use crate::{
    AssociatedTypeCompletionContext, ClosureTable, ConstructionSurfaceTable, CopyabilityTable,
    DropTable, InstanceOperationTable, InterfaceImplementationTable, LoanTable, ProvenanceTable,
    StandardSemanticTable,
};

use super::{CheckedBody, OpaqueWitnessTable};

/// Complete syntax-independent Phase 3 program.
#[derive(Debug)]
pub struct CheckedProgram {
    environment: crate::program_environment::ProgramEnvironment,
    semantics: crate::semantic_authority::CheckedSemanticAuthority,
    provenance: ProvenanceTable,
    loans: LoanTable,
    opaque_witnesses: OpaqueWitnessTable,
    bodies: Arena<BodyId, CheckedBody>,
    associated_type_completion_contexts: Box<[AssociatedTypeCompletionContext]>,
}

pub(crate) struct CheckedProgramAuthorities {
    pub(crate) provenance: ProvenanceTable,
    pub(crate) loans: LoanTable,
    pub(crate) opaque_witnesses: OpaqueWitnessTable,
    pub(crate) associated_type_completion_contexts: Box<[AssociatedTypeCompletionContext]>,
}

impl CheckedProgram {
    pub(crate) fn new(
        environment: crate::program_environment::ProgramEnvironment,
        semantics: crate::semantic_authority::CheckedSemanticAuthority,
        authorities: CheckedProgramAuthorities,
        bodies: Arena<BodyId, CheckedBody>,
    ) -> Self {
        Self {
            environment,
            semantics,
            provenance: authorities.provenance,
            loans: authorities.loans,
            opaque_witnesses: authorities.opaque_witnesses,
            bodies,
            associated_type_completion_contexts: authorities.associated_type_completion_contexts,
        }
    }

    #[must_use]
    pub const fn graph(&self) -> &DeclarationGraph {
        self.environment.graph()
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        self.semantics.semantics().types()
    }

    #[must_use]
    pub const fn interface_implementations(&self) -> &InterfaceImplementationTable {
        self.environment.interface_implementations()
    }

    #[must_use]
    pub(crate) const fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        self.environment.construction_surfaces()
    }

    #[must_use]
    pub const fn instance_operations(&self) -> &InstanceOperationTable {
        self.environment.instance_operations()
    }

    pub(crate) const fn body_assumptions(&self) -> &BodyAssumptionTable {
        self.environment.body_assumptions()
    }

    #[must_use]
    pub const fn copyabilities(&self) -> &CopyabilityTable {
        self.semantics.semantics().copyabilities()
    }

    pub(crate) const fn semantic_authority(&self) -> &crate::semantic_authority::SemanticAuthority {
        self.semantics.semantics()
    }

    #[must_use]
    pub const fn drops(&self) -> &DropTable {
        self.environment.drops()
    }

    #[must_use]
    pub const fn standard_semantics(&self) -> &StandardSemanticTable {
        self.environment.standard_semantics()
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
        self.semantics.closures()
    }

    #[must_use]
    pub(crate) const fn source_access(&self) -> &SourceAccessTable {
        self.environment.source_access()
    }

    #[must_use]
    pub const fn source_ownership(&self) -> &nocter_frontend_bindings::SourceOwnershipTable {
        self.environment.source_access().ownership()
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
        crate::SourceAccessContext::for_source(self.environment.source_access(), source)
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

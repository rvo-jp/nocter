use std::sync::Arc;

use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::SourceAccessTable;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source::SourceId;
use nocter_source_index::SourceIndex;

use crate::{
    AssociatedTypeCompletionContext, ClosureTable, ConstructionSurfaceTable, CopyabilityTable,
    DropTable, InstanceOperationTable, InterfaceImplementationTable, LoanTable, ProvenanceTable,
    StandardSemanticTable,
};

use super::{CheckedBody, OpaqueWitnessTable};

/// Complete syntax-independent Phase 3 program.
#[derive(Clone, Debug)]
pub struct CheckedProgram {
    environment: Arc<crate::program_environment::ProgramEnvironment>,
    source_access: Arc<SourceAccessTable>,
    semantics: Arc<crate::body_check::CheckedSemanticAuthority>,
    provenance: Arc<ProvenanceTable>,
    execution_facts: Arc<crate::ExecutionFactTable>,
    loans: Arc<LoanTable>,
    opaque_witnesses: Arc<OpaqueWitnessTable>,
    compile_time: Arc<crate::CompileTimeProgram>,
    bodies: Arc<Arena<BodyId, CheckedBody>>,
    associated_type_completion_contexts: Arc<[AssociatedTypeCompletionContext]>,
}

pub(crate) struct CheckedProgramAuthorities {
    pub(crate) provenance: Arc<ProvenanceTable>,
    pub(crate) execution_facts: Arc<crate::ExecutionFactTable>,
    pub(crate) loans: Arc<LoanTable>,
    pub(crate) opaque_witnesses: Arc<OpaqueWitnessTable>,
    pub(crate) compile_time: Arc<crate::CompileTimeProgram>,
    pub(crate) associated_type_completion_contexts: Arc<[AssociatedTypeCompletionContext]>,
}

impl CheckedProgram {
    pub(crate) fn environment(&self) -> &crate::program_environment::ProgramEnvironment {
        &self.environment
    }

    pub(crate) fn new(
        environment: Arc<crate::program_environment::ProgramEnvironment>,
        semantics: Arc<crate::body_check::CheckedSemanticAuthority>,
        authorities: CheckedProgramAuthorities,
        bodies: Arc<Arena<BodyId, CheckedBody>>,
        source_access: Arc<SourceAccessTable>,
    ) -> Self {
        Self {
            environment,
            source_access,
            semantics,
            provenance: authorities.provenance,
            execution_facts: authorities.execution_facts,
            loans: authorities.loans,
            opaque_witnesses: authorities.opaque_witnesses,
            compile_time: authorities.compile_time,
            bodies,
            associated_type_completion_contexts: authorities.associated_type_completion_contexts,
        }
    }

    #[must_use]
    pub fn graph(&self) -> &DeclarationGraph {
        self.environment.graph()
    }

    /// Returns the single evaluated value owned by one declared constant identity.
    #[must_use]
    pub fn constant_value(
        &self,
        id: nocter_model::ConstantId,
    ) -> Option<&nocter_model::ConstantValue> {
        self.compile_time.values().constants().get(id)
    }

    /// Returns the single evaluated value owned by one declared static identity.
    #[must_use]
    pub fn static_value(&self, id: nocter_model::StaticId) -> Option<&nocter_model::FrozenValue> {
        self.compile_time.values().statics().get(id)
    }

    #[must_use]
    pub fn declaration_values(&self) -> &nocter_declarations::DeclarationValueTable {
        self.compile_time.values()
    }

    #[must_use]
    pub fn types(&self) -> &TypeStore {
        self.semantics.semantics().types()
    }

    #[must_use]
    pub fn interface_implementations(&self) -> &InterfaceImplementationTable {
        self.environment.interface_implementations()
    }

    #[must_use]
    pub fn capability_evidence(
        &self,
        evidence: nocter_model::CapabilityEvidenceId,
    ) -> Option<&crate::CapabilityEvidence> {
        self.environment.capability_evidence().get(evidence)
    }

    #[must_use]
    pub(crate) fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        self.environment.construction_surfaces()
    }

    #[must_use]
    pub fn instance_operations(&self) -> &InstanceOperationTable {
        self.environment.instance_operations()
    }

    #[must_use]
    pub fn copyabilities(&self) -> &CopyabilityTable {
        self.semantics.semantics().copyabilities()
    }

    pub(crate) fn semantic_authority(&self) -> &crate::semantic_authority::SemanticAuthority {
        self.semantics.semantics()
    }

    #[must_use]
    pub fn drops(&self) -> &DropTable {
        self.environment.drops()
    }

    #[must_use]
    pub fn standard_semantics(&self) -> &StandardSemanticTable {
        self.environment.standard_semantics()
    }

    #[must_use]
    pub fn provenance(&self) -> &ProvenanceTable {
        &self.provenance
    }

    #[must_use]
    pub fn execution_facts(&self) -> &crate::ExecutionFactTable {
        &self.execution_facts
    }

    #[must_use]
    pub fn loans(&self) -> &LoanTable {
        &self.loans
    }

    #[must_use]
    pub fn closures(&self) -> &ClosureTable {
        self.semantics.closures()
    }

    #[must_use]
    pub(crate) fn source_access(&self) -> &SourceAccessTable {
        &self.source_access
    }

    #[must_use]
    pub fn source_ownership(&self) -> &nocter_frontend_bindings::SourceOwnershipTable {
        self.source_access.ownership()
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
    pub fn opaque_witnesses(&self) -> &OpaqueWitnessTable {
        &self.opaque_witnesses
    }

    /// Returns the canonical compile-time plans accepted during this program's finalization.
    #[must_use]
    pub fn compile_time_plans(&self) -> &crate::CompileTimePlanTable {
        self.compile_time.plans()
    }

    /// Returns the complete compile-time authority for this accepted program.
    #[must_use]
    pub fn compile_time_program(&self) -> &crate::CompileTimeProgram {
        &self.compile_time
    }

    #[must_use]
    pub fn bodies(&self) -> &Arena<BodyId, CheckedBody> {
        &self.bodies
    }

    #[must_use]
    pub fn associated_type_completion_contexts(&self) -> &[AssociatedTypeCompletionContext] {
        &self.associated_type_completion_contexts
    }
}

/// Checked semantics and its independent source projection.
#[derive(Clone, Debug)]
pub struct CheckedProgramOutput {
    program: CheckedProgram,
    source_index: Arc<SourceIndex>,
}

/// A rejected semantic-component transform with the exact checked output restored.
#[derive(Clone, Debug)]
pub struct CheckedProgramMapFailure<E> {
    error: E,
    checked: CheckedProgramOutput,
}

impl<E> CheckedProgramMapFailure<E> {
    #[must_use]
    pub fn into_parts(self) -> (E, CheckedProgramOutput) {
        (self.error, self.checked)
    }
}

impl CheckedProgramOutput {
    #[must_use]
    pub(crate) const fn new(program: CheckedProgram, source_index: Arc<SourceIndex>) -> Self {
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
    pub fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (CheckedProgram, SourceIndex) {
        (self.program, Arc::unwrap_or_clone(self.source_index))
    }

    /// Transforms only the semantic component while preserving its exact source projection.
    ///
    /// A rejecting transform must return the consumed semantic program with its error. This keeps
    /// the pair recoverable without granting the transform access to presentation state.
    ///
    /// # Errors
    ///
    /// Returns the transform error with the exact restored checked output.
    pub fn try_map_program<T, E>(
        self,
        transform: impl FnOnce(CheckedProgram) -> Result<T, Box<(E, CheckedProgram)>>,
    ) -> Result<(T, SourceIndex), Box<CheckedProgramMapFailure<E>>> {
        let Self {
            program,
            source_index,
        } = self;
        match transform(program) {
            Ok(transformed) => Ok((transformed, Arc::unwrap_or_clone(source_index))),
            Err(failure) => {
                let (error, program) = *failure;
                Err(Box::new(CheckedProgramMapFailure {
                    error,
                    checked: Self {
                        program,
                        source_index,
                    },
                }))
            }
        }
    }

    /// Opens an owned consumer branch from one immutable exact-current query result.
    #[must_use]
    pub fn current_branch(&self) -> Self {
        self.clone()
    }
}

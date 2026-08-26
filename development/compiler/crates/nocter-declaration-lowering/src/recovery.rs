use nocter_declarations::{
    BodyAnalysisDeclarationProgram, DeclarationAnalysisProgram, DeclarationGraph,
    RejectedDeclarationAnalysis,
};
use nocter_frontend_bindings::{FrontendBindings, SourceOwnershipTable};
use nocter_model::TypeStore;
use nocter_source_index::SourceIndex;

/// Source-projected declaration facts retained after an authored declaration rule rejects source.
///
/// The snapshot contains no accepted declaration program, builder, or production transition. It
/// retains source bindings only so the editor-only admission boundary can analyze independent
/// bodies without rerunning declaration lowering. Consumers cannot pass it to production checking.
#[derive(Debug)]
pub struct DeclarationLoweringRecovery {
    program: DeclarationRecoveryProgram,
    frontend_bindings: FrontendBindings,
    source_index: SourceIndex,
}

#[derive(Debug)]
enum DeclarationRecoveryProgram {
    Declarations(DeclarationAnalysisProgram),
    Bodies(BodyAnalysisDeclarationProgram),
}

/// Consuming transition from declaration recovery into its exact editor-analysis capability.
#[derive(Debug)]
pub enum DeclarationCheckingTransition {
    Declarations(Box<DeclarationLoweringRecovery>),
    Bodies(Box<DeclarationBodyAnalysisInput>),
}

/// Complete input admitted to editor-only body analysis after declaration rejection.
#[derive(Debug)]
pub struct DeclarationBodyAnalysisInput {
    program: BodyAnalysisDeclarationProgram,
    frontend_bindings: FrontendBindings,
    source_index: SourceIndex,
}

impl DeclarationBodyAnalysisInput {
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        BodyAnalysisDeclarationProgram,
        FrontendBindings,
        SourceIndex,
    ) {
        (self.program, self.frontend_bindings, self.source_index)
    }
}

impl DeclarationLoweringRecovery {
    pub(crate) fn new(
        program: RejectedDeclarationAnalysis,
        frontend_bindings: FrontendBindings,
        source_index: SourceIndex,
    ) -> Self {
        let program = match program {
            RejectedDeclarationAnalysis::Declarations(program) => {
                DeclarationRecoveryProgram::Declarations(program)
            }
            RejectedDeclarationAnalysis::Bodies(program) => {
                DeclarationRecoveryProgram::Bodies(program)
            }
        };
        Self {
            program,
            frontend_bindings,
            source_index,
        }
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_declaration_parts(
        self,
    ) -> (
        DeclarationGraph,
        TypeStore,
        SourceOwnershipTable,
        SourceIndex,
    ) {
        let ownership = self.frontend_bindings.source_ownership().clone();
        let (graph, types) = match self.program {
            DeclarationRecoveryProgram::Declarations(program) => program.into_parts(),
            DeclarationRecoveryProgram::Bodies(program) => {
                let (graph, types, _) = program.into_parts();
                (graph, types)
            }
        };
        (graph, types, ownership, self.source_index)
    }

    /// Opens the editor-only declaration-to-body analysis boundary. The returned program cannot
    /// enter the production checking API.
    #[must_use]
    pub fn into_checking_transition(self) -> DeclarationCheckingTransition {
        match self {
            Self {
                program: DeclarationRecoveryProgram::Bodies(program),
                frontend_bindings,
                source_index,
            } => DeclarationCheckingTransition::Bodies(Box::new(DeclarationBodyAnalysisInput {
                program,
                frontend_bindings,
                source_index,
            })),
            recovery => DeclarationCheckingTransition::Declarations(Box::new(recovery)),
        }
    }
}

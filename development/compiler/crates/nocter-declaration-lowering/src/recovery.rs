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
#[derive(Clone, Debug)]
pub struct DeclarationLoweringRecovery {
    program: DeclarationRecoveryProgram,
    frontend_bindings: FrontendBindings,
    source_index: SourceIndex,
}

#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
        current_symbols: &crate::current_symbols::CurrentCheckingSymbols,
    ) -> Self {
        let program = match program {
            RejectedDeclarationAnalysis::Declarations(program) => {
                DeclarationRecoveryProgram::Declarations(program)
            }
            RejectedDeclarationAnalysis::Bodies(program) => {
                DeclarationRecoveryProgram::Bodies(current_symbols.extend_body_analysis(program))
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
    pub const fn graph(&self) -> &DeclarationGraph {
        match &self.program {
            DeclarationRecoveryProgram::Declarations(program) => program.graph(),
            DeclarationRecoveryProgram::Bodies(program) => program.graph(),
        }
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        match &self.program {
            DeclarationRecoveryProgram::Declarations(program) => program.types(),
            DeclarationRecoveryProgram::Bodies(program) => program.types(),
        }
    }

    #[must_use]
    pub const fn source_ownership(&self) -> &SourceOwnershipTable {
        self.frontend_bindings.source_ownership()
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

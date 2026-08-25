use nocter_declarations::{AnalysisDeclarationProgram, DeclarationGraph};
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::TypeStore;
use nocter_source_index::SourceIndex;

use crate::CompileUnitInput;

/// Source-projected declaration facts retained after an authored declaration rule rejects source.
///
/// The snapshot contains no accepted declaration program, builder, or production transition. It
/// retains source bindings only so the editor-only admission boundary can analyze independent
/// bodies without rerunning declaration lowering. Consumers cannot pass it to production checking.
#[derive(Debug)]
pub struct DeclarationLoweringRecovery {
    program: AnalysisDeclarationProgram,
    frontend_bindings: FrontendBindings,
    source_index: SourceIndex,
}

impl DeclarationLoweringRecovery {
    pub(crate) const fn new(
        program: AnalysisDeclarationProgram,
        frontend_bindings: FrontendBindings,
        source_index: SourceIndex,
    ) -> Self {
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
    pub fn into_declaration_parts(self) -> (DeclarationGraph, TypeStore, SourceIndex) {
        let (graph, types, _) = self.program.into_parts();
        (graph, types, self.source_index)
    }

    /// Opens the editor-only declaration-to-body analysis boundary. Block imports are projected
    /// exactly as they are for accepted declarations; the returned program cannot enter the
    /// production checking API.
    #[must_use]
    pub fn into_checking_parts(
        self,
        input: &CompileUnitInput<'_>,
    ) -> (AnalysisDeclarationProgram, FrontendBindings, SourceIndex) {
        let bindings = crate::frontend_projection::add_block_imports(input, self.frontend_bindings);
        (self.program, bindings, self.source_index)
    }
}

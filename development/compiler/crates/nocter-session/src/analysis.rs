use nocter_checking::CheckedProgramOutput;
use nocter_discovery::DiscoveredUnit;
use nocter_target_program::{TargetProgram, ToolchainSnapshot};

use crate::{CompileSessionError, CompileSessionFailure, CompiledTarget, SemanticEvidenceBundle};

/// A failed target analysis and the exact current-generation semantic evidence that remains valid.
#[derive(Debug)]
pub(crate) struct CompileTargetFailure {
    failure: CompileSessionFailure,
    semantic: Option<Box<SemanticEvidenceBundle>>,
    diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
}

/// Best-effort source analysis performed beneath an authoritative syntax failure.
///
/// The syntax outcome remains failed. This value independently retains a semantic failure and the
/// exact evidence contract reached by that attempt.
#[derive(Debug)]
pub(crate) struct IncompleteSyntaxAnalysis {
    failure: Option<CompileSessionFailure>,
    semantic: Option<Box<SemanticEvidenceBundle>>,
    diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
}

impl IncompleteSyntaxAnalysis {
    pub(crate) fn empty() -> Self {
        Self {
            failure: None,
            semantic: None,
            diagnostics: Box::new([]),
        }
    }

    fn failed(failure: CompileSessionFailure, semantic: Option<SemanticEvidenceBundle>) -> Self {
        let diagnostics = analysis_diagnostics(Some(&failure), semantic.as_ref());
        Self {
            failure: Some(failure),
            semantic: semantic.map(Box::new),
            diagnostics,
        }
    }

    #[must_use]
    pub fn into_analysis_parts(
        self,
    ) -> (
        Option<CompileSessionFailure>,
        Option<SemanticEvidenceBundle>,
        Box<[nocter_diagnostics::SourceDiagnostic]>,
    ) {
        (
            self.failure,
            self.semantic.map(|semantic| *semantic),
            self.diagnostics,
        )
    }
}

impl CompileTargetFailure {
    fn new(error: CompileSessionError, semantic: Option<SemanticEvidenceBundle>) -> Self {
        Self::from_failure(CompileSessionFailure::single(error), semantic)
    }

    fn from_failure(
        failure: CompileSessionFailure,
        semantic: Option<SemanticEvidenceBundle>,
    ) -> Self {
        let diagnostics = analysis_diagnostics(Some(&failure), semantic.as_ref());
        Self {
            failure,
            semantic: semantic.map(Box::new),
            diagnostics,
        }
    }

    #[must_use]
    pub fn into_analysis_parts(
        self,
    ) -> (
        CompileSessionFailure,
        Option<SemanticEvidenceBundle>,
        Box<[nocter_diagnostics::SourceDiagnostic]>,
    ) {
        (
            self.failure,
            self.semantic.map(|semantic| *semantic),
            self.diagnostics,
        )
    }
}

fn analysis_diagnostics(
    failure: Option<&CompileSessionFailure>,
    semantic: Option<&SemanticEvidenceBundle>,
) -> Box<[nocter_diagnostics::SourceDiagnostic]> {
    let mut diagnostics = failure
        .into_iter()
        .flat_map(CompileSessionFailure::source_diagnostics)
        .cloned()
        .collect::<Vec<_>>();
    if let Some(semantic) = semantic {
        semantic.extend_rejection_diagnostics(&mut diagnostics);
    }
    diagnostics.into_boxed_slice()
}

pub(crate) fn target_from_finalized_program(
    unit: &DiscoveredUnit,
    finalized: &nocter_semantic_product::FinalizedProgram,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let primitive_bindings = finalized.declarations().primitive_bindings().to_vec();
    let checked = finalized.current_branch();
    finish_semantic_product(unit, primitive_bindings, checked)
}

pub(crate) fn failure_from_finalization(
    failure: &nocter_checking::BodyCheckFailure,
) -> CompileTargetFailure {
    let (error, recovery) = failure.current_branch().into_parts();
    CompileTargetFailure::new(
        error.into(),
        recovery.map(SemanticEvidenceBundle::from_bodies),
    )
}

pub(crate) fn failure_from_name_resolution(
    failure: &nocter_checking::QueriedNameResolutionFailure,
) -> CompileTargetFailure {
    let (error, evidence) = failure.current_branch().into_parts();
    CompileTargetFailure::new(
        error.into(),
        evidence.map(SemanticEvidenceBundle::from_preparation_failure),
    )
}

pub(crate) fn failure_from_preparation(
    rejection: &nocter_checking::QueriedProgramPreparationRejection,
) -> CompileTargetFailure {
    let (error, evidence) = rejection.current_branch().into_parts();
    CompileTargetFailure::new(
        error.into(),
        evidence.map(SemanticEvidenceBundle::from_preparation_failure),
    )
}

pub(crate) fn incomplete_syntax_analysis(
    analysis: &nocter_semantic_product::IncompleteSemanticAnalysis,
) -> IncompleteSyntaxAnalysis {
    let Some(failure) = analysis.failure() else {
        return IncompleteSyntaxAnalysis::empty();
    };
    let (failure, evidence) = semantic_failure_parts(failure);
    IncompleteSyntaxAnalysis::failed(failure, evidence)
}

pub(crate) fn failure_from_incomplete_semantics(
    failure: &nocter_semantic_product::IncompleteSemanticFailure,
) -> CompileTargetFailure {
    let (failure, evidence) = semantic_failure_parts(failure);
    CompileTargetFailure::from_failure(failure, evidence)
}

fn semantic_failure_parts(
    failure: &nocter_semantic_product::IncompleteSemanticFailure,
) -> (CompileSessionFailure, Option<SemanticEvidenceBundle>) {
    let (primary, continuation, evidence) = failure.current_branch().into_parts();
    (
        CompileSessionFailure::new(
            primary.into(),
            continuation
                .into_iter()
                .map(CompileSessionError::from)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        evidence.map(SemanticEvidenceBundle::from_incomplete),
    )
}

fn finish_semantic_product(
    unit: &DiscoveredUnit,
    primitive_bindings: Vec<nocter_runtime_contract::PrimitiveBinding>,
    checked: CheckedProgramOutput,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let primitives = match nocter_runtime_contract::PrimitiveRegistry::new(primitive_bindings) {
        Ok(primitives) => primitives,
        Err(error) => {
            return Err(Box::new(failure_with_checked(error.into(), checked)));
        }
    };
    finish_checked_target(unit.target(), primitives, checked)
}

fn finish_checked_target(
    target: nocter_model::CompilationTarget,
    primitives: nocter_runtime_contract::PrimitiveRegistry,
    checked: CheckedProgramOutput,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let Some(standard_package) = checked.program().graph().standard_package() else {
        return Err(Box::new(failure_with_checked(
            CompileSessionError::MissingStandardPackage,
            checked,
        )));
    };
    let snapshot = match ToolchainSnapshot::select(target, standard_package, primitives) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(Box::new(failure_with_checked(error.into(), checked)));
        }
    };
    let (program, source_index) = match checked.try_map_program(|program| {
        TargetProgram::build_retaining_checked(program, snapshot)
            .map_err(|failure| Box::new((*failure).into_parts()))
    }) {
        Ok(output) => output,
        Err(failure) => {
            let (error, checked) = (*failure).into_parts();
            return Err(Box::new(CompileTargetFailure::new(
                error.into(),
                Some(SemanticEvidenceBundle::from_checked(checked)),
            )));
        }
    };
    Ok(CompiledTarget::new(program, source_index))
}

fn failure_with_checked(
    error: CompileSessionError,
    checked: CheckedProgramOutput,
) -> CompileTargetFailure {
    CompileTargetFailure::new(error, Some(SemanticEvidenceBundle::from_checked(checked)))
}

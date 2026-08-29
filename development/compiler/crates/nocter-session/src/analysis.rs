use nocter_checking::CheckedProgramOutput;
use nocter_discovery::DiscoveredUnit;
use nocter_target_program::{TargetProgram, ToolchainSnapshot};

use crate::{CompileSessionError, CompiledTarget, SemanticEvidenceBundle};

/// A failed target analysis and the exact current-generation semantic evidence that remains valid.
#[derive(Debug)]
pub(crate) struct CompileTargetFailure {
    error: CompileSessionError,
    semantic: Option<Box<SemanticEvidenceBundle>>,
    diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
}

/// Best-effort source analysis performed beneath an authoritative syntax failure.
///
/// The syntax outcome remains failed. This value independently retains a semantic failure and the
/// exact evidence contract reached by that attempt.
#[derive(Debug)]
pub(crate) struct IncompleteSyntaxAnalysis {
    semantic: Option<Box<SemanticEvidenceBundle>>,
    diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
}

impl IncompleteSyntaxAnalysis {
    pub(crate) fn empty() -> Self {
        Self {
            semantic: None,
            diagnostics: Box::new([]),
        }
    }

    fn failed(error: &CompileSessionError, semantic: Option<SemanticEvidenceBundle>) -> Self {
        let diagnostics = analysis_diagnostics(Some(error), semantic.as_ref());
        Self {
            semantic: semantic.map(Box::new),
            diagnostics,
        }
    }

    #[must_use]
    pub fn into_analysis_parts(
        self,
    ) -> (
        Option<SemanticEvidenceBundle>,
        Box<[nocter_diagnostics::SourceDiagnostic]>,
    ) {
        (self.semantic.map(|semantic| *semantic), self.diagnostics)
    }
}

impl CompileTargetFailure {
    fn new(error: CompileSessionError, semantic: Option<SemanticEvidenceBundle>) -> Self {
        let diagnostics = analysis_diagnostics(Some(&error), semantic.as_ref());
        Self {
            error,
            semantic: semantic.map(Box::new),
            diagnostics,
        }
    }

    #[must_use]
    pub fn into_analysis_parts(
        self,
    ) -> (
        CompileSessionError,
        Option<SemanticEvidenceBundle>,
        Box<[nocter_diagnostics::SourceDiagnostic]>,
    ) {
        (
            self.error,
            self.semantic.map(|semantic| *semantic),
            self.diagnostics,
        )
    }
}

fn analysis_diagnostics(
    error: Option<&CompileSessionError>,
    semantic: Option<&SemanticEvidenceBundle>,
) -> Box<[nocter_diagnostics::SourceDiagnostic]> {
    let mut diagnostics = error
        .into_iter()
        .flat_map(CompileSessionError::source_diagnostics)
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
) -> Box<CompileTargetFailure> {
    let (error, recovery) = failure.current_branch().into_parts();
    Box::new(CompileTargetFailure::new(
        error.into(),
        recovery.map(SemanticEvidenceBundle::from_bodies),
    ))
}

pub(crate) fn failure_from_name_resolution(
    failure: &nocter_checking::QueriedNameResolutionFailure,
) -> Box<CompileTargetFailure> {
    let error = nocter_checking::PreparationError::NameResolution(
        nocter_checking::NameResolutionError::Rule(failure.diagnostic().clone()),
    );
    let evidence =
        nocter_checking::PreparationFailureEvidence::Names(Box::new(failure.current_recovery()));
    Box::new(CompileTargetFailure::new(
        error.into(),
        Some(SemanticEvidenceBundle::from_preparation_failure(evidence)),
    ))
}

pub(crate) fn failure_from_preparation(
    rejection: &nocter_checking::QueriedProgramPreparationRejection,
) -> Box<CompileTargetFailure> {
    let (error, evidence) = rejection.current_branch().into_parts();
    Box::new(CompileTargetFailure::new(
        error.into(),
        evidence.map(SemanticEvidenceBundle::from_preparation_failure),
    ))
}

pub(crate) fn incomplete_syntax_analysis(
    analysis: &nocter_semantic_product::IncompleteSemanticAnalysis,
) -> IncompleteSyntaxAnalysis {
    let Some(failure) = analysis.failure() else {
        return IncompleteSyntaxAnalysis::empty();
    };
    let (error, evidence) = semantic_failure_parts(failure);
    IncompleteSyntaxAnalysis::failed(&error, evidence)
}

pub(crate) fn failure_from_incomplete_semantics(
    failure: &nocter_semantic_product::IncompleteSemanticFailure,
) -> Box<CompileTargetFailure> {
    let (error, evidence) = semantic_failure_parts(failure);
    Box::new(CompileTargetFailure::new(error, evidence))
}

fn semantic_failure_parts(
    failure: &nocter_semantic_product::IncompleteSemanticFailure,
) -> (CompileSessionError, Option<SemanticEvidenceBundle>) {
    let (error, evidence) = failure.current_branch().into_parts();
    (
        error.into(),
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
    let (program, source_index) = checked.into_parts();
    let program = match TargetProgram::build_retaining_checked(program, snapshot) {
        Ok(program) => program,
        Err(failure) => {
            let (error, program) = (*failure).into_parts();
            let checked = CheckedProgramOutput::new(program, source_index);
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

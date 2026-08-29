use nocter_checking::CheckedProgramOutput;
use nocter_discovery::DiscoveredUnit;
use nocter_target_program::{TargetProgram, ToolchainSnapshot};

use crate::semantic_pipeline::{
    SemanticPipelineFailure, SemanticPipelineOutput, SyntaxAdmission, run_semantic_pipeline,
    run_semantic_pipeline_from_declaration_failure, run_semantic_pipeline_from_declarations,
    run_semantic_pipeline_from_prepared_declarations,
};
use crate::{CompileSessionError, CompiledTarget, SemanticEvidenceBundle};

/// A failed target analysis and the exact current-generation semantic evidence that remains valid.
#[derive(Debug)]
pub struct CompileTargetFailure {
    error: CompileSessionError,
    semantic: Option<Box<SemanticEvidenceBundle>>,
    diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
}

/// Best-effort source analysis performed beneath an authoritative syntax failure.
///
/// The syntax outcome remains failed. This value independently retains a semantic failure and the
/// exact evidence contract reached by that attempt.
#[derive(Debug)]
pub struct IncompleteSyntaxAnalysis {
    failure: Option<CompileSessionError>,
    semantic: Option<Box<SemanticEvidenceBundle>>,
    diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
}

impl IncompleteSyntaxAnalysis {
    fn empty() -> Self {
        Self {
            failure: None,
            semantic: None,
            diagnostics: Box::new([]),
        }
    }

    fn failed(error: CompileSessionError, semantic: Option<SemanticEvidenceBundle>) -> Self {
        let diagnostics = analysis_diagnostics(Some(&error), semantic.as_ref());
        Self {
            failure: Some(error),
            semantic: semantic.map(Box::new),
            diagnostics,
        }
    }

    #[must_use]
    pub const fn failure(&self) -> Option<&CompileSessionError> {
        self.failure.as_ref()
    }

    #[must_use]
    pub fn semantic_evidence(&self) -> Option<crate::SemanticEvidenceView<'_>> {
        self.semantic.as_deref().map(SemanticEvidenceBundle::view)
    }

    #[must_use]
    pub const fn source_diagnostics(&self) -> &[nocter_diagnostics::SourceDiagnostic] {
        &self.diagnostics
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
    pub const fn error(&self) -> &CompileSessionError {
        &self.error
    }

    #[must_use]
    pub fn semantic_evidence(&self) -> Option<crate::SemanticEvidenceView<'_>> {
        self.semantic.as_deref().map(SemanticEvidenceBundle::view)
    }

    /// Returns every source diagnostic that explains this analysis result, including rejected
    /// body evidence retained beneath an earlier production failure.
    #[must_use]
    pub const fn source_diagnostics(&self) -> &[nocter_diagnostics::SourceDiagnostic] {
        &self.diagnostics
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

    #[must_use]
    pub fn into_error(self) -> CompileSessionError {
        self.error
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

/// Runs one immutable discovery snapshot while retaining the deepest valid current-generation
/// analysis recovery when declaration preparation, name resolution, or typed-body source fails.
///
/// # Errors
///
/// Returns the exact production-session failure. No earlier successful generation participates.
pub fn analyze_target(unit: &DiscoveredUnit) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    analyze_target_internal(unit, true)
}

pub(crate) fn analyze_target_from_declarations(
    unit: &DiscoveredUnit,
    declarations: &nocter_declaration_lowering::ReusableDeclarations,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let output = run_semantic_pipeline_from_declarations(unit, declarations).map_err(
        |SemanticPipelineFailure { error, evidence }| {
            Box::new(CompileTargetFailure::new(*error, evidence))
        },
    )?;
    finish_semantic_pipeline(unit, output, true)
}

pub(crate) fn analyze_target_from_prepared_declarations(
    unit: &DiscoveredUnit,
    declarations: &nocter_declaration_lowering::ReusableDeclarations,
    prepared: &nocter_checking::ReusablePreparedProgram,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let output = run_semantic_pipeline_from_prepared_declarations(unit, declarations, prepared)
        .map_err(|SemanticPipelineFailure { error, evidence }| {
            Box::new(CompileTargetFailure::new(*error, evidence))
        })?;
    finish_semantic_pipeline(unit, output, true)
}

pub(crate) fn analyze_target_from_declaration_failure(
    unit: &DiscoveredUnit,
    failure: &nocter_declaration_lowering::DeclarationLoweringFailure,
) -> Box<CompileTargetFailure> {
    let SemanticPipelineFailure { error, evidence } =
        run_semantic_pipeline_from_declaration_failure(unit, failure);
    Box::new(CompileTargetFailure::new(*error, evidence))
}

/// Attempts editor-only semantic analysis beneath an authoritative syntax failure.
///
/// This path can never return a target program or claim compilation success. It preserves the
/// deepest declaration, name, or body stage reached before the explicit missing/error syntax node
/// or an independent authored rule stopped analysis.
#[must_use]
pub fn analyze_incomplete_syntax(unit: &DiscoveredUnit) -> Option<IncompleteSyntaxAnalysis> {
    if !unit.has_syntax_errors() {
        return None;
    }
    match run_semantic_pipeline(unit, SyntaxAdmission::IncompleteBodies) {
        Err(SemanticPipelineFailure { error, evidence }) => {
            Some(IncompleteSyntaxAnalysis::failed(*error, evidence))
        }
        Ok(_) => Some(IncompleteSyntaxAnalysis::empty()),
    }
}

pub(crate) fn compile_target_without_recovery(
    unit: &DiscoveredUnit,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    analyze_target_internal(unit, false)
}

fn analyze_target_internal(
    unit: &DiscoveredUnit,
    retain_semantic: bool,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let output = run_semantic_pipeline(unit, SyntaxAdmission::Complete).map_err(
        |SemanticPipelineFailure { error, evidence }| {
            Box::new(CompileTargetFailure::new(
                *error,
                retain_semantic.then_some(evidence).flatten(),
            ))
        },
    )?;
    finish_semantic_pipeline(unit, output, retain_semantic)
}

fn finish_semantic_pipeline(
    unit: &DiscoveredUnit,
    output: SemanticPipelineOutput,
    retain_semantic: bool,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let primitive_bindings = output.primitive_bindings;
    let checked = output.checked;
    let primitives = match nocter_runtime_contract::PrimitiveRegistry::new(primitive_bindings) {
        Ok(primitives) => primitives,
        Err(error) => {
            return Err(Box::new(failure_with_checked(
                error.into(),
                checked,
                retain_semantic,
            )));
        }
    };
    finish_checked_target(unit.target(), primitives, checked, retain_semantic)
}

fn finish_checked_target(
    target: nocter_model::CompilationTarget,
    primitives: nocter_runtime_contract::PrimitiveRegistry,
    checked: CheckedProgramOutput,
    retain_semantic: bool,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let Some(standard_package) = checked.program().graph().standard_package() else {
        return Err(Box::new(failure_with_checked(
            CompileSessionError::MissingStandardPackage,
            checked,
            retain_semantic,
        )));
    };
    let snapshot = match ToolchainSnapshot::select(target, standard_package, primitives) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(Box::new(failure_with_checked(
                error.into(),
                checked,
                retain_semantic,
            )));
        }
    };
    let (program, source_index) = checked.into_parts();
    let program = if retain_semantic {
        match TargetProgram::build_retaining_checked(program, snapshot) {
            Ok(program) => program,
            Err(failure) => {
                let (error, program) = (*failure).into_parts();
                let checked = CheckedProgramOutput::new(program, source_index);
                return Err(Box::new(CompileTargetFailure::new(
                    error.into(),
                    Some(SemanticEvidenceBundle::from_checked(checked)),
                )));
            }
        }
    } else {
        TargetProgram::build(program, snapshot)
            .map_err(CompileSessionError::from)
            .map_err(without_prepared)
            .map_err(Box::new)?
    };
    Ok(CompiledTarget::new(program, source_index))
}

fn failure_with_checked(
    error: CompileSessionError,
    checked: CheckedProgramOutput,
    retain: bool,
) -> CompileTargetFailure {
    CompileTargetFailure::new(
        error,
        retain.then(|| SemanticEvidenceBundle::from_checked(checked)),
    )
}

fn without_prepared(error: CompileSessionError) -> CompileTargetFailure {
    CompileTargetFailure::new(error, None)
}

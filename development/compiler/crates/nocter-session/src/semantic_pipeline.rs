#![allow(clippy::disallowed_methods)]

use nocter_checking::{
    CheckedProgramOutput, check_prepared_program_recovering, prepare_program_checking_recovering,
};
use nocter_declaration_lowering::{
    DeclarationLoweringRecovery, lower_compile_unit_declarations_recovering,
};
use nocter_discovery::DiscoveredUnit;
use nocter_runtime_contract::PrimitiveBinding;

use crate::{CompileSessionError, SemanticEvidenceBundle};

pub(crate) struct SemanticPipelineOutput {
    pub(crate) primitive_bindings: Vec<PrimitiveBinding>,
    pub(crate) checked: CheckedProgramOutput,
}

pub(crate) struct SemanticPipelineFailure {
    pub(crate) error: Box<CompileSessionError>,
    pub(crate) evidence: Option<SemanticEvidenceBundle>,
}

/// Runs complete-syntax declaration lowering, checking preparation, and body checking exactly once.
///
/// Declaration recovery delegates to the same compiler-domain continuation used by the
/// incomplete-syntax query, so direct compilation and editor analysis cannot diverge there.
pub(crate) fn run_semantic_pipeline(
    unit: &DiscoveredUnit,
) -> Result<SemanticPipelineOutput, SemanticPipelineFailure> {
    let input = unit
        .compile_input()
        .map_err(CompileSessionError::from)
        .map_err(|error| SemanticPipelineFailure {
            error: Box::new(error),
            evidence: None,
        })?;

    let lowered = match lower_compile_unit_declarations_recovering(&input) {
        Ok(lowered) => lowered,
        Err(failure) => {
            return Err(continue_declaration_failure(&input, failure));
        }
    };

    let primitive_bindings = lowered.primitive_bindings().to_vec();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    check_declaration_bodies(
        &input,
        primitive_bindings,
        program,
        &frontend_bindings,
        source_index,
    )
}

fn continue_declaration_failure(
    input: &nocter_compile_input::CompileUnitInput<'_>,
    failure: nocter_declaration_lowering::DeclarationLoweringFailure,
) -> SemanticPipelineFailure {
    let (error, recovery) = failure.into_parts();
    let evidence = recovery.and_then(|recovery| continue_rejected_declarations(input, recovery));
    SemanticPipelineFailure {
        error: Box::new(error.into()),
        evidence,
    }
}

fn check_declaration_bodies(
    input: &nocter_compile_input::CompileUnitInput<'_>,
    primitive_bindings: Vec<PrimitiveBinding>,
    program: nocter_declarations::AcceptedDeclarationProgram,
    frontend_bindings: &nocter_frontend_bindings::FrontendBindings,
    source_index: nocter_source_index::SourceIndex,
) -> Result<SemanticPipelineOutput, SemanticPipelineFailure> {
    let prepared =
        prepare_program_checking_recovering(input, program, frontend_bindings, source_index)
            .map_err(|failure| {
                let (error, evidence) = failure.into_parts();
                let evidence = evidence.map(SemanticEvidenceBundle::from_preparation_failure);
                SemanticPipelineFailure {
                    error: Box::new(error.into()),
                    evidence,
                }
            })?;
    check_prepared_bodies(primitive_bindings, input, prepared)
}

fn check_prepared_bodies(
    primitive_bindings: Vec<PrimitiveBinding>,
    input: &nocter_compile_input::CompileUnitInput<'_>,
    prepared: nocter_checking::PreparedChecking<'_>,
) -> Result<SemanticPipelineOutput, SemanticPipelineFailure> {
    let checked = check_prepared_program_recovering(input, prepared).map_err(|failure| {
        let (error, recovery) = failure.into_parts();
        SemanticPipelineFailure {
            error: Box::new(error.into()),
            evidence: recovery.map(SemanticEvidenceBundle::from_bodies),
        }
    })?;
    Ok(SemanticPipelineOutput {
        primitive_bindings,
        checked,
    })
}

fn continue_rejected_declarations(
    input: &nocter_compile_input::CompileUnitInput<'_>,
    recovery: DeclarationLoweringRecovery,
) -> Option<SemanticEvidenceBundle> {
    nocter_semantic_computation::continue_declaration_recovery(input, recovery)
        .map(SemanticEvidenceBundle::from_incomplete)
}

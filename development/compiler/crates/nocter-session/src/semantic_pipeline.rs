use nocter_checking::{
    CheckedProgramOutput, analyze_prepared_program_bodies, check_prepared_program_recovering,
    prepare_analysis_program_checking_recovering, prepare_program_checking_recovering,
};
use nocter_declaration_lowering::{
    DeclarationCheckingTransition, DeclarationLoweringRecovery,
    lower_compile_unit_declarations_recovering, lower_incomplete_body_declarations_recovering,
};
use nocter_discovery::DiscoveredUnit;
use nocter_runtime_contract::PrimitiveBinding;

use crate::{CompileSessionError, SemanticEvidenceBundle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SyntaxAdmission {
    Complete,
    IncompleteBodies,
}

pub(crate) struct SemanticPipelineOutput {
    pub(crate) primitive_bindings: Vec<PrimitiveBinding>,
    pub(crate) checked: CheckedProgramOutput,
}

pub(crate) struct SemanticPipelineFailure {
    pub(crate) error: Box<CompileSessionError>,
    pub(crate) evidence: Option<SemanticEvidenceBundle>,
}

/// Runs declaration lowering, checking preparation, and body checking exactly once.
///
/// Syntax admission selects which source input may enter the pipeline. Evidence production does
/// not select another stage graph: every failure carries the exact recovery authority produced by
/// this one traversal, and callers may discard that evidence only after the traversal ends.
pub(crate) fn run_semantic_pipeline(
    unit: &DiscoveredUnit,
    admission: SyntaxAdmission,
) -> Result<SemanticPipelineOutput, SemanticPipelineFailure> {
    let input = match admission {
        SyntaxAdmission::Complete => unit.compile_input(),
        SyntaxAdmission::IncompleteBodies => unit.analysis_input(),
    }
    .map_err(CompileSessionError::from)
    .map_err(|error| SemanticPipelineFailure {
        error: Box::new(error),
        evidence: None,
    })?;

    let lowered = {
        let result = match admission {
            SyntaxAdmission::Complete => lower_compile_unit_declarations_recovering(&input),
            SyntaxAdmission::IncompleteBodies => {
                lower_incomplete_body_declarations_recovering(&input)
            }
        };
        match result {
            Ok(lowered) => lowered,
            Err(failure) => {
                let (error, recovery) = failure.into_parts();
                let evidence =
                    recovery.and_then(|recovery| continue_rejected_declarations(&input, recovery));
                return Err(SemanticPipelineFailure {
                    error: Box::new(error.into()),
                    evidence,
                });
            }
        }
    };

    let primitive_bindings = lowered.primitive_bindings().to_vec();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking_recovering(&input, program, &frontend_bindings, source_index)
            .map_err(|failure| {
                let (error, recovery) = failure.into_parts();
                SemanticPipelineFailure {
                    error: Box::new(error.into()),
                    evidence: recovery.map(SemanticEvidenceBundle::from_preparation),
                }
            })?;
    let checked = check_prepared_program_recovering(&input, prepared).map_err(|failure| {
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
    let (program, frontend_bindings, source_index) = match recovery.into_checking_transition() {
        DeclarationCheckingTransition::Bodies(input) => input.into_parts(),
        DeclarationCheckingTransition::Declarations(recovery) => {
            return Some(SemanticEvidenceBundle::from_declaration_lowering(*recovery));
        }
    };
    let prepared = match prepare_analysis_program_checking_recovering(
        input,
        program,
        &frontend_bindings,
        source_index,
    ) {
        Ok(prepared) => prepared,
        Err(failure) => {
            let (_, recovery) = failure.into_parts();
            return recovery.map(SemanticEvidenceBundle::from_preparation);
        }
    };
    match analyze_prepared_program_bodies(input, prepared) {
        Ok(analysis) => Some(SemanticEvidenceBundle::from_bodies(analysis)),
        Err(failure) => {
            let (_, recovery) = failure.into_parts();
            recovery.map(SemanticEvidenceBundle::from_bodies)
        }
    }
}

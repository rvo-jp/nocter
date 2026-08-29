#![allow(clippy::disallowed_methods)]

use nocter_checking::{
    CheckedProgramOutput, analyze_prepared_program_bodies, check_prepared_program_recovering,
    prepare_analysis_program_checking_recovering,
    prepare_program_checking_from_reusable_recovering, prepare_program_checking_recovering,
};
use nocter_declaration_lowering::{
    DeclarationCheckingTransition, DeclarationLoweringRecovery, ReusableDeclarations,
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
                return Err(continue_declaration_failure(&input, failure));
            }
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

/// Continues editor recovery from the exact declaration rejection computed by a query.
pub(crate) fn run_semantic_pipeline_from_declaration_failure(
    unit: &DiscoveredUnit,
    failure: &nocter_declaration_lowering::DeclarationLoweringFailure,
) -> SemanticPipelineFailure {
    let input = match unit.compile_input() {
        Ok(input) => input,
        Err(error) => {
            return SemanticPipelineFailure {
                error: Box::new(error.into()),
                evidence: None,
            };
        }
    };
    continue_declaration_failure(&input, failure.current_branch())
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

/// Continues checking from one source-neutral declaration query result.
pub(crate) fn run_semantic_pipeline_from_declarations(
    unit: &DiscoveredUnit,
    declarations: &ReusableDeclarations,
) -> Result<SemanticPipelineOutput, SemanticPipelineFailure> {
    let input = unit
        .compile_input()
        .map_err(CompileSessionError::from)
        .map_err(|error| SemanticPipelineFailure {
            error: Box::new(error),
            evidence: None,
        })?;
    let projection = declarations
        .materialize_projection(&input)
        .map_err(CompileSessionError::from)
        .map_err(|error| SemanticPipelineFailure {
            error: Box::new(error),
            evidence: None,
        })?;
    let primitive_bindings = declarations.primitive_bindings().to_vec();
    let (frontend_bindings, source_index, checking_symbols) = projection.into_parts();
    let program = declarations
        .checking_branch()
        .with_checking_symbols(checking_symbols.spellings());
    check_declaration_bodies(
        &input,
        primitive_bindings,
        program,
        &frontend_bindings,
        source_index,
    )
}

/// Continues checking from query-owned declaration and program-wide semantic authorities.
pub(crate) fn run_semantic_pipeline_from_prepared_declarations(
    unit: &DiscoveredUnit,
    declarations: &ReusableDeclarations,
    prepared: &nocter_checking::ReusablePreparedProgram,
) -> Result<SemanticPipelineOutput, SemanticPipelineFailure> {
    let input = unit
        .compile_input()
        .map_err(CompileSessionError::from)
        .map_err(|error| SemanticPipelineFailure {
            error: Box::new(error),
            evidence: None,
        })?;
    let projection = declarations
        .materialize_projection(&input)
        .map_err(CompileSessionError::from)
        .map_err(|error| SemanticPipelineFailure {
            error: Box::new(error),
            evidence: None,
        })?;
    let primitive_bindings = declarations.primitive_bindings().to_vec();
    let (frontend_bindings, source_index, checking_symbols) = projection.into_parts();
    let prepared = prepare_program_checking_from_reusable_recovering(
        &input,
        prepared,
        checking_symbols.spellings(),
        &frontend_bindings,
        source_index,
    )
    .map_err(|failure| {
        let (error, evidence) = failure.into_parts();
        let evidence = evidence.map(SemanticEvidenceBundle::from_preparation_failure);
        SemanticPipelineFailure {
            error: Box::new(error.into()),
            evidence,
        }
    })?;
    check_prepared_bodies(primitive_bindings, &input, prepared)
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
            let (_, evidence) = failure.into_parts();
            return evidence.map(SemanticEvidenceBundle::from_preparation_failure);
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

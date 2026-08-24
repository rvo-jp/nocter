use nocter_checking::{
    BodyAnalysisRecovery, DeclarationAnalysisRecovery, PreparedSemanticProgram,
    SemanticAnalysisRecovery, check_prepared_program, check_prepared_program_recovering,
    prepare_program_checking, prepare_program_checking_recovering,
};
use nocter_declaration_lowering::{
    lower_compile_unit_declarations, lower_compile_unit_declarations_recovering,
    lower_incomplete_body_declarations, resolve_primitive_bindings,
};
use nocter_discovery::DiscoveredUnit;
use nocter_target_program::{TargetProgram, ToolchainSnapshot};

use crate::{CompileSessionError, CompiledTarget};

/// A failed target analysis and the deepest current-generation semantic stage that remains valid.
#[derive(Debug)]
pub struct CompileTargetFailure {
    error: CompileSessionError,
    recovery: Option<Box<SemanticAnalysisRecovery>>,
}

impl CompileTargetFailure {
    fn new(error: CompileSessionError, recovery: Option<SemanticAnalysisRecovery>) -> Self {
        Self {
            error,
            recovery: recovery.map(Box::new),
        }
    }

    #[must_use]
    pub const fn error(&self) -> &CompileSessionError {
        &self.error
    }

    #[must_use]
    pub fn prepared(&self) -> Option<&PreparedSemanticProgram> {
        self.recovery()
            .and_then(SemanticAnalysisRecovery::bodies)
            .map(BodyAnalysisRecovery::prepared)
    }

    #[must_use]
    pub fn recovery(&self) -> Option<&SemanticAnalysisRecovery> {
        self.recovery.as_deref()
    }

    #[must_use]
    pub fn into_parts(self) -> (CompileSessionError, Option<SemanticAnalysisRecovery>) {
        (self.error, self.recovery.map(|recovery| *recovery))
    }

    #[must_use]
    pub fn into_error(self) -> CompileSessionError {
        self.error
    }
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

/// Attempts editor-only semantic recovery beneath an authoritative syntax failure.
///
/// This path can never return a checked or target program. It preserves only a preparation stage
/// and an optional typed interruption reached before an explicit missing/error syntax node stopped
/// the production phases.
#[must_use]
pub fn analyze_incomplete_syntax(unit: &DiscoveredUnit) -> Option<BodyAnalysisRecovery> {
    if !unit.has_syntax_errors() {
        return None;
    }
    let input = unit.analysis_input().ok()?;
    let lowered = lower_incomplete_body_declarations(&input).ok()?;
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).ok()?;
    match check_prepared_program_recovering(&input, prepared) {
        Err(failure) => failure.into_parts().1,
        Ok(_) => None,
    }
}

pub(crate) fn compile_target_without_recovery(
    unit: &DiscoveredUnit,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    analyze_target_internal(unit, false)
}

fn analyze_target_internal(
    unit: &DiscoveredUnit,
    retain_prepared: bool,
) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    let input = unit
        .compile_input()
        .map_err(CompileSessionError::from)
        .map_err(without_prepared)
        .map_err(Box::new)?;
    let primitive_roles = input
        .toolchain()
        .ok_or(CompileSessionError::MissingToolchainProfile)
        .map_err(without_prepared)
        .map_err(Box::new)?
        .primitive_roles()
        .to_vec();
    let lowered = if retain_prepared {
        lower_compile_unit_declarations_recovering(&input).map_err(|failure| {
            let (error, recovery) = failure.into_parts();
            let recovery = recovery.map(|lowered| {
                let (program, source_index) = lowered.into_parts();
                SemanticAnalysisRecovery::Declarations(Box::new(
                    DeclarationAnalysisRecovery::from_program(program, source_index),
                ))
            });
            Box::new(CompileTargetFailure::new(error.into(), recovery))
        })?
    } else {
        lower_compile_unit_declarations(&input)
            .map_err(CompileSessionError::from)
            .map_err(without_prepared)
            .map_err(Box::new)?
    };
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared = if retain_prepared {
        prepare_program_checking_recovering(&input, program, &frontend_bindings, source_index)
            .map_err(|failure| {
                let (error, recovery) = failure.into_parts();
                Box::new(CompileTargetFailure::new(error.into(), recovery))
            })?
    } else {
        prepare_program_checking(&input, program, &frontend_bindings, source_index)
            .map_err(CompileSessionError::from)
            .map_err(without_prepared)
            .map_err(Box::new)?
    };
    let checked = if retain_prepared {
        check_prepared_program_recovering(&input, prepared).map_err(|failure| {
            let (error, recovery) = failure.into_parts();
            Box::new(CompileTargetFailure::new(
                error.into(),
                recovery.map(|recovery| SemanticAnalysisRecovery::Bodies(Box::new(recovery))),
            ))
        })?
    } else {
        check_prepared_program(&input, prepared)
            .map_err(CompileSessionError::from)
            .map_err(without_prepared)
            .map_err(Box::new)?
    };
    let (program, source_index) = checked.into_parts();
    let standard_package = program
        .graph()
        .standard_package()
        .ok_or(CompileSessionError::MissingStandardPackage)
        .map_err(without_prepared)
        .map_err(Box::new)?;
    let primitives = resolve_primitive_bindings(&primitive_roles, &source_index)
        .map_err(CompileSessionError::from)
        .map_err(without_prepared)
        .map_err(Box::new)?;
    let snapshot = ToolchainSnapshot::select(input.target(), standard_package, primitives)
        .map_err(CompileSessionError::from)
        .map_err(without_prepared)
        .map_err(Box::new)?;
    let program = TargetProgram::build(program, snapshot)
        .map_err(CompileSessionError::from)
        .map_err(without_prepared)
        .map_err(Box::new)?;
    Ok(CompiledTarget::new(program, source_index))
}

fn without_prepared(error: CompileSessionError) -> CompileTargetFailure {
    CompileTargetFailure::new(error, None)
}

use nocter_checking::{
    CheckedProgramOutput, check_prepared_program, check_prepared_program_recovering,
    prepare_program_checking, prepare_program_checking_recovering,
};
use nocter_declaration_lowering::{
    lower_compile_unit_declarations, lower_compile_unit_declarations_recovering,
    lower_incomplete_body_declarations_recovering, resolve_primitive_bindings,
};
use nocter_discovery::DiscoveredUnit;
use nocter_target_program::{TargetProgram, ToolchainSnapshot};

use crate::{CompileSessionError, CompiledTarget, SemanticAnalysis};

/// A failed target analysis and the deepest current-generation semantic stage that remains valid.
#[derive(Debug)]
pub struct CompileTargetFailure {
    error: CompileSessionError,
    semantic: Option<Box<SemanticAnalysis>>,
}

/// Best-effort source analysis performed beneath an authoritative syntax failure.
///
/// The syntax outcome remains failed. This value independently retains a semantic-stage failure
/// and the deepest completed semantic authority reached by that attempt.
#[derive(Debug)]
pub struct IncompleteSyntaxAnalysis {
    failure: Option<CompileSessionError>,
    semantic: Option<Box<SemanticAnalysis>>,
}

impl IncompleteSyntaxAnalysis {
    fn empty() -> Self {
        Self {
            failure: None,
            semantic: None,
        }
    }

    fn failed(error: CompileSessionError, semantic: Option<SemanticAnalysis>) -> Self {
        Self {
            failure: Some(error),
            semantic: semantic.map(Box::new),
        }
    }

    #[must_use]
    pub const fn failure(&self) -> Option<&CompileSessionError> {
        self.failure.as_ref()
    }

    #[must_use]
    pub fn semantic(&self) -> Option<&SemanticAnalysis> {
        self.semantic.as_deref()
    }

    #[must_use]
    pub fn into_parts(self) -> (Option<CompileSessionError>, Option<SemanticAnalysis>) {
        (self.failure, self.semantic.map(|semantic| *semantic))
    }
}

impl CompileTargetFailure {
    fn new(error: CompileSessionError, semantic: Option<SemanticAnalysis>) -> Self {
        Self {
            error,
            semantic: semantic.map(Box::new),
        }
    }

    #[must_use]
    pub const fn error(&self) -> &CompileSessionError {
        &self.error
    }

    #[must_use]
    pub fn semantic(&self) -> Option<&SemanticAnalysis> {
        self.semantic.as_deref()
    }

    #[must_use]
    pub fn into_parts(self) -> (CompileSessionError, Option<SemanticAnalysis>) {
        (self.error, self.semantic.map(|semantic| *semantic))
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
    let input = match unit.analysis_input() {
        Ok(input) => input,
        Err(error) => {
            return Some(IncompleteSyntaxAnalysis::failed(error.into(), None));
        }
    };
    let lowered = match lower_incomplete_body_declarations_recovering(&input) {
        Ok(lowered) => lowered,
        Err(failure) => {
            let (error, recovery) = failure.into_parts();
            return Some(IncompleteSyntaxAnalysis::failed(
                error.into(),
                recovery.map(SemanticAnalysis::from_declaration_lowering),
            ));
        }
    };
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared = match prepare_program_checking_recovering(
        &input,
        program,
        &frontend_bindings,
        source_index,
    ) {
        Ok(prepared) => prepared,
        Err(failure) => {
            let (error, recovery) = failure.into_parts();
            return Some(IncompleteSyntaxAnalysis::failed(
                error.into(),
                recovery.map(SemanticAnalysis::from_preparation),
            ));
        }
    };
    match check_prepared_program_recovering(&input, prepared) {
        Err(failure) => {
            let (error, recovery) = failure.into_parts();
            Some(IncompleteSyntaxAnalysis::failed(
                error.into(),
                recovery.map(SemanticAnalysis::from_bodies),
            ))
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
    let lowered = if retain_semantic {
        lower_compile_unit_declarations_recovering(&input).map_err(|failure| {
            let (error, recovery) = failure.into_parts();
            let semantic = recovery.map(SemanticAnalysis::from_declaration_lowering);
            Box::new(CompileTargetFailure::new(error.into(), semantic))
        })?
    } else {
        lower_compile_unit_declarations(&input)
            .map_err(CompileSessionError::from)
            .map_err(without_prepared)
            .map_err(Box::new)?
    };
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared = if retain_semantic {
        prepare_program_checking_recovering(&input, program, &frontend_bindings, source_index)
            .map_err(|failure| {
                let (error, recovery) = failure.into_parts();
                Box::new(CompileTargetFailure::new(
                    error.into(),
                    recovery.map(SemanticAnalysis::from_preparation),
                ))
            })?
    } else {
        prepare_program_checking(&input, program, &frontend_bindings, source_index)
            .map_err(CompileSessionError::from)
            .map_err(without_prepared)
            .map_err(Box::new)?
    };
    let checked = if retain_semantic {
        check_prepared_program_recovering(&input, prepared).map_err(|failure| {
            let (error, recovery) = failure.into_parts();
            Box::new(CompileTargetFailure::new(
                error.into(),
                recovery.map(SemanticAnalysis::from_bodies),
            ))
        })?
    } else {
        check_prepared_program(&input, prepared)
            .map_err(CompileSessionError::from)
            .map_err(without_prepared)
            .map_err(Box::new)?
    };
    finish_checked_target(&input, &primitive_roles, checked, retain_semantic)
}

fn finish_checked_target(
    input: &nocter_compile_input::CompileUnitInput<'_>,
    primitive_roles: &[nocter_compile_input::PrimitiveRoleInput],
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
    let primitives = match resolve_primitive_bindings(primitive_roles, checked.source_index()) {
        Ok(primitives) => primitives,
        Err(error) => {
            return Err(Box::new(failure_with_checked(
                error.into(),
                checked,
                retain_semantic,
            )));
        }
    };
    let snapshot = match ToolchainSnapshot::select(input.target(), standard_package, primitives) {
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
                    Some(SemanticAnalysis::from_checked(checked)),
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
        retain.then(|| SemanticAnalysis::from_checked(checked)),
    )
}

fn without_prepared(error: CompileSessionError) -> CompileTargetFailure {
    CompileTargetFailure::new(error, None)
}

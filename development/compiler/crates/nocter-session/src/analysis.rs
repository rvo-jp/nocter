use nocter_checking::{
    PreparedSemanticProgram, check_prepared_program, check_prepared_program_recovering,
    prepare_program_checking,
};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_discovery::DiscoveredUnit;
use nocter_target_program::{PrimitiveRegistry, TargetProgram, ToolchainSnapshot};

use crate::{CompileSessionError, CompiledTarget};

/// A failed target analysis and the deepest current-generation semantic stage that remains valid.
#[derive(Debug)]
pub struct CompileTargetFailure {
    error: CompileSessionError,
    prepared: Option<Box<PreparedSemanticProgram>>,
}

impl CompileTargetFailure {
    fn new(error: CompileSessionError, prepared: Option<PreparedSemanticProgram>) -> Self {
        Self {
            error,
            prepared: prepared.map(Box::new),
        }
    }

    #[must_use]
    pub const fn error(&self) -> &CompileSessionError {
        &self.error
    }

    #[must_use]
    pub fn prepared(&self) -> Option<&PreparedSemanticProgram> {
        self.prepared.as_deref()
    }

    #[must_use]
    pub fn into_parts(self) -> (CompileSessionError, Option<PreparedSemanticProgram>) {
        (self.error, self.prepared.map(|prepared| *prepared))
    }

    #[must_use]
    pub fn into_error(self) -> CompileSessionError {
        self.error
    }
}

/// Runs one immutable discovery snapshot while retaining a completed preparation stage when
/// authored typed-body source fails.
///
/// # Errors
///
/// Returns the exact production-session failure. No earlier successful generation participates.
pub fn analyze_target(unit: &DiscoveredUnit) -> Result<CompiledTarget, Box<CompileTargetFailure>> {
    analyze_target_internal(unit, true)
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
    let lowered = lower_compile_unit_declarations(&input)
        .map_err(CompileSessionError::from)
        .map_err(without_prepared)
        .map_err(Box::new)?;
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index)
        .map_err(CompileSessionError::from)
        .map_err(without_prepared)
        .map_err(Box::new)?;
    let checked = if retain_prepared {
        check_prepared_program_recovering(&input, prepared).map_err(|failure| {
            let (error, prepared) = failure.into_parts();
            Box::new(CompileTargetFailure::new(error.into(), prepared))
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
    let primitives = PrimitiveRegistry::resolve(&primitive_roles, &source_index)
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

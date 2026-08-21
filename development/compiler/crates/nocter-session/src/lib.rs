//! Closed orchestration for one Nocter compilation session.
//!
//! This crate is the production ownership boundary from a syntax-clean discovery snapshot to a
//! target-validated program. It does not repeat discovery, lowering, checking, registry, or target
//! decisions owned by lower layers.

mod error;
mod executable;
mod output;
mod profile;

pub use error::CompileSessionError;
pub use executable::{
    ExecutableCompileRequest, ExecutableSelectionError, ExecutableSelector, ExecutableSessionError,
    compile_executable,
};
pub use output::{CompiledExecutable, CompiledTarget};
pub use profile::bundled_standard_toolchain;

use nocter_checking::{check_prepared_program, prepare_program_checking};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_discovery::DiscoveredUnit;
use nocter_target_program::{PrimitiveRegistry, TargetProgram, ToolchainSnapshot};

/// Consumes one immutable discovery snapshot through the complete selected-target boundary.
///
/// # Errors
///
/// Returns the exact failed stage. Syntax diagnostics remain available on `unit`; semantic stage
/// errors retain their native source diagnostics and rule identities.
pub fn compile_target(unit: &DiscoveredUnit) -> Result<CompiledTarget, CompileSessionError> {
    let input = unit.compile_input()?;
    let primitive_roles = input
        .toolchain()
        .ok_or(CompileSessionError::MissingToolchainProfile)?
        .primitive_roles()
        .to_vec();
    let lowered = lower_compile_unit_declarations(&input)?;
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index)?;
    let checked = check_prepared_program(&input, prepared)?;
    let (program, source_index) = checked.into_parts();
    let standard_package = program
        .graph()
        .standard_package()
        .ok_or(CompileSessionError::MissingStandardPackage)?;
    let primitives = PrimitiveRegistry::resolve(&primitive_roles, &source_index)?;
    let snapshot = ToolchainSnapshot::select(input.target(), standard_package, primitives)?;
    let program = TargetProgram::build(program, snapshot)?;
    Ok(CompiledTarget::new(program, source_index))
}

#[cfg(test)]
mod tests;

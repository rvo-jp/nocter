//! Closed orchestration for one Nocter compilation session.
//!
//! This crate is the production ownership boundary from a syntax-clean discovery snapshot to a
//! target-validated program. It does not repeat discovery, lowering, checking, registry, or target
//! decisions owned by lower layers.

mod analysis;
mod error;
mod executable;
mod native;
mod output;
mod profile;
mod test;

pub use analysis::{CompileTargetFailure, analyze_target};
pub use error::CompileSessionError;
pub use executable::{
    ExecutableCompileRequest, ExecutableIdentity, ExecutableSelectionError, ExecutableSelector,
    ExecutableSessionError, compile_executable,
};
pub use native::{
    NativeImageError, NativeImageSetCompileRequest, NativeImageSetError, NativeSessionError,
    compile_native_image, compile_native_images,
};
pub use output::{
    CompiledExecutable, CompiledNativeImage, CompiledNativeImageSet, CompiledTarget,
    NativeImageEntry,
};
pub use profile::bundled_standard_toolchain;
pub use test::{
    CompiledNativeTestSet, NativeTestCompileRequest, NativeTestImage, NativeTestSessionError,
    NativeTestTargetCompilation, NativeTestTargetOutcome, TestCaseIdentity, TestTargetIdentity,
    TestTargetSelectionError, TestTargetSelector, compile_native_tests,
};

use nocter_discovery::DiscoveredUnit;

/// Consumes one immutable discovery snapshot through the complete selected-target boundary.
///
/// # Errors
///
/// Returns the exact failed stage. Syntax diagnostics remain available on `unit`; semantic stage
/// errors retain their native source diagnostics and rule identities.
pub fn compile_target(unit: &DiscoveredUnit) -> Result<CompiledTarget, CompileSessionError> {
    analysis::compile_target_without_recovery(unit).map_err(|failure| (*failure).into_error())
}

#[cfg(test)]
mod tests;

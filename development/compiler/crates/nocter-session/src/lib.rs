//! Closed orchestration for one Nocter compilation session.
//!
//! This crate is the production ownership boundary from a syntax-clean discovery snapshot to a
//! target-validated program. It does not repeat discovery, lowering, checking, registry, or target
//! decisions owned by lower layers.

mod analysis;
mod analyzed_unit;
mod error;
mod executable;
mod output;
mod profile;
mod semantic_analysis;
mod semantic_capabilities;
mod semantic_pipeline;
mod test_selection;

pub use analysis::{
    CompileTargetFailure, IncompleteSyntaxAnalysis, analyze_incomplete_syntax, analyze_target,
};
pub use analyzed_unit::{
    AnalyzedUnit, AnalyzedUnitStatus, SemanticAnalysisDomainError, analyze_unit,
    analyze_unit_from_query,
};
pub use error::CompileSessionError;
pub use executable::{
    ExecutableCompileRequest, ExecutableIdentity, ExecutableSelectionError, ExecutableSelector,
    ExecutableSessionError, close_executable, compile_executable, root_executables,
};
pub use output::{CompiledExecutable, CompiledTarget};
pub use profile::bundled_standard_toolchain;
pub use semantic_analysis::{SemanticEvidenceBundle, SemanticEvidenceView};
pub use semantic_capabilities::{
    CompleteSemanticEvidenceView, InterfaceImplementationRepairView, SemanticBodyNamesView,
    SemanticInterruptionView, SemanticTypedBodyView,
};
pub use test_selection::TestTargetSelector;

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

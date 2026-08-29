//! Closed orchestration for one Nocter compilation session.
//!
//! This crate is the production ownership boundary from one closed unit-analysis product to a
//! target-validated program or capability-oriented recovery evidence. It does not repeat
//! discovery, lowering, checking, or semantic query decisions owned by lower layers.

mod analysis;
mod analyzed_unit;
mod error;
mod executable;
mod output;
mod profile;
mod semantic_analysis;
mod semantic_capabilities;
mod test_selection;

pub use analyzed_unit::{
    AnalyzedCompilationFailure, AnalyzedUnit, AnalyzedUnitStatus, SemanticAnalysisDomainError,
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

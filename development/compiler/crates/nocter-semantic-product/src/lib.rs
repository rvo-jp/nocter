//! Immutable semantic result contract shared with post-computation consumers.

pub use nocter_compiler_computation::{
    FinalizedProgram, IncompleteSemanticAnalysis, IncompleteSemanticError,
    IncompleteSemanticEvidence, IncompleteSemanticFailure, ProgramAnalysisOutcome,
    ProgramAnalysisProduct, ProgramAnalysisUnavailable, UnitAnalysisOutcome, UnitAnalysisProduct,
    UnitAnalysisUnavailable,
};

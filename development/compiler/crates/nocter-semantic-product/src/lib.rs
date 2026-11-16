//! Immutable semantic result contract shared with post-computation consumers.

pub use nocter_compiler_computation::{
    AcceptedValueAccess, FinalizedProgram, IncompleteSemanticAnalysis, IncompleteSemanticError,
    IncompleteSemanticEvidence, IncompleteSemanticFailure, ProgramAnalysisOutcome,
    ProgramAnalysisProduct, SemanticQueryFailure, UnitAnalysisOutcome, UnitAnalysisProduct,
    ValueAccessAnalysis,
};

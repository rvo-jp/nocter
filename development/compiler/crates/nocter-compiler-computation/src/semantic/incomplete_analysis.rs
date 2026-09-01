//! Exact-current incomplete-syntax analysis.

#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};
use nocter_declaration_lowering::{
    DeclarationCheckingTransition, DeclarationLoweringRecovery,
    lower_incomplete_body_declarations_recovering,
};

use super::{CurrentSourceScopeInput, SemanticScopeKey};

struct IncompleteAnalysisQuery;

/// Compiler-domain failure selected by one editor-only incomplete-syntax traversal.
#[derive(Clone, Debug)]
pub enum IncompleteSemanticError {
    CompileInput(nocter_discovery::CompileInputError),
    Declaration(nocter_declaration_lowering::DeclarationLoweringError),
    Preparation(nocter_checking::PreparationError),
    Checking(nocter_checking::BodyCheckError),
}

/// Deepest semantic authority retained by an incomplete-syntax traversal.
#[derive(Clone, Debug)]
pub enum IncompleteSemanticEvidence {
    Declarations(Box<nocter_declaration_lowering::DeclarationLoweringRecovery>),
    Preparation(Box<nocter_checking::PreparationFailureEvidence>),
    Bodies(Box<nocter_checking::BodyAnalysisRecovery>),
}

/// One failure and the independently valid current-source evidence reached beneath it.
#[derive(Clone, Debug)]
pub struct IncompleteSemanticFailure {
    error: IncompleteSemanticError,
    evidence: Option<IncompleteSemanticEvidence>,
}

impl IncompleteSemanticFailure {
    #[must_use]
    pub fn current_branch(&self) -> Self {
        self.clone()
    }

    #[must_use]
    pub fn into_parts(self) -> (IncompleteSemanticError, Option<IncompleteSemanticEvidence>) {
        (self.error, self.evidence)
    }
}

/// Result of the sole editor-only semantic traversal admitted beneath invalid syntax.
#[derive(Clone, Debug)]
pub struct IncompleteSemanticAnalysis {
    failure: Option<IncompleteSemanticFailure>,
}

impl IncompleteSemanticAnalysis {
    #[must_use]
    pub const fn failure(&self) -> Option<&IncompleteSemanticFailure> {
        self.failure.as_ref()
    }
}

/// Exact-current query product paired with the discovery snapshot that justified it.
#[derive(Debug)]
pub struct IncompleteAnalysisProduct {
    outcome: IncompleteAnalysisOutcome,
    fingerprint: Fingerprint,
}

#[derive(Debug)]
pub enum IncompleteAnalysisOutcome {
    Analyzed(IncompleteSemanticAnalysis),
    Failed(Arc<super::SemanticQueryFailure>),
}

impl IncompleteAnalysisProduct {
    #[must_use]
    pub const fn outcome(&self) -> &IncompleteAnalysisOutcome {
        &self.outcome
    }
}

impl QueryValue for IncompleteAnalysisProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for IncompleteAnalysisQuery {
    type Key = SemanticScopeKey;
    type Value = IncompleteAnalysisProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let outcome = if current.unit.has_syntax_errors() {
            IncompleteAnalysisOutcome::Analyzed(analyze_incomplete_semantics(&current.unit))
        } else {
            IncompleteAnalysisOutcome::Failed(Arc::new(
                super::SemanticQueryFailure::InvalidStageTransition(
                    "incomplete-syntax query demanded for complete syntax",
                ),
            ))
        };
        Ok(IncompleteAnalysisProduct {
            outcome,
            fingerprint: current.fingerprint,
        })
    }
}

/// Demands editor-only semantic recovery for one exact current source scope.
///
/// # Errors
///
/// Returns computation-kernel failures. Compiler-domain failure is retained inside the product.
pub(super) fn incomplete_analysis(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<IncompleteAnalysisProduct>, ComputationError> {
    database.query::<IncompleteAnalysisQuery>(key)
}

/// Runs the one compiler-domain incomplete-syntax traversal demanded by the incremental query
/// graph.
#[must_use]
fn analyze_incomplete_semantics(
    unit: &nocter_discovery::DiscoveredUnit,
) -> IncompleteSemanticAnalysis {
    let input = match unit.analysis_input() {
        Ok(input) => input,
        Err(error) => {
            return IncompleteSemanticAnalysis {
                failure: Some(IncompleteSemanticFailure {
                    error: IncompleteSemanticError::CompileInput(error),
                    evidence: None,
                }),
            };
        }
    };
    let lowered = match lower_incomplete_body_declarations_recovering(&input) {
        Ok(lowered) => lowered,
        Err(failure) => {
            return IncompleteSemanticAnalysis {
                failure: Some(continue_declaration_failure(&input, failure)),
            };
        }
    };
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared = match nocter_checking::prepare_program_checking_recovering(
        &input,
        program,
        &frontend_bindings,
        source_index,
    ) {
        Ok(prepared) => prepared,
        Err(failure) => {
            let (error, evidence) = failure.into_parts();
            return IncompleteSemanticAnalysis {
                failure: Some(IncompleteSemanticFailure {
                    error: IncompleteSemanticError::Preparation(error),
                    evidence: evidence
                        .map(Box::new)
                        .map(IncompleteSemanticEvidence::Preparation),
                }),
            };
        }
    };
    let failure = nocter_checking::check_prepared_program_recovering(&input, prepared)
        .err()
        .map(|failure| {
            let (error, recovery) = failure.into_parts();
            IncompleteSemanticFailure {
                error: IncompleteSemanticError::Checking(error),
                evidence: recovery
                    .map(Box::new)
                    .map(IncompleteSemanticEvidence::Bodies),
            }
        });
    IncompleteSemanticAnalysis { failure }
}

fn continue_declaration_failure(
    input: &nocter_compile_input::CompileUnitInput<'_>,
    failure: nocter_declaration_lowering::DeclarationLoweringFailure,
) -> IncompleteSemanticFailure {
    let (error, recovery) = failure.into_parts();
    let evidence = recovery.and_then(|recovery| continue_declaration_recovery(input, recovery));
    IncompleteSemanticFailure {
        error: IncompleteSemanticError::Declaration(error),
        evidence,
    }
}

/// Materializes the deepest valid editor recovery beneath one exact-current declaration failure.
#[must_use]
pub(super) fn analyze_declaration_failure(
    input: &nocter_compile_input::CompileUnitInput<'_>,
    failure: &nocter_declaration_lowering::DeclarationLoweringFailure,
) -> IncompleteSemanticFailure {
    continue_declaration_failure(input, failure.current_branch())
}

/// Continues one declaration recovery through its sole editor-analysis transition.
#[must_use]
pub(super) fn continue_declaration_recovery(
    input: &nocter_compile_input::CompileUnitInput<'_>,
    recovery: DeclarationLoweringRecovery,
) -> Option<IncompleteSemanticEvidence> {
    let (program, frontend_bindings, source_index) = match recovery.into_checking_transition() {
        DeclarationCheckingTransition::Bodies(input) => input.into_parts(),
        DeclarationCheckingTransition::Declarations(recovery) => {
            return Some(IncompleteSemanticEvidence::Declarations(recovery));
        }
    };
    let prepared = match nocter_checking::prepare_analysis_program_checking_recovering(
        input,
        program,
        &frontend_bindings,
        source_index,
    ) {
        Ok(prepared) => prepared,
        Err(failure) => {
            let (_, evidence) = failure.into_parts();
            return evidence
                .map(Box::new)
                .map(IncompleteSemanticEvidence::Preparation);
        }
    };
    match nocter_checking::analyze_prepared_program_bodies(input, prepared) {
        Ok(analysis) => Some(IncompleteSemanticEvidence::Bodies(Box::new(analysis))),
        Err(failure) => {
            let (_, recovery) = failure.into_parts();
            recovery
                .map(Box::new)
                .map(IncompleteSemanticEvidence::Bodies)
        }
    }
}

#[must_use]
pub(super) fn incomplete_analysis_execution_count(database: &Database) -> u64 {
    database.execution_count::<IncompleteAnalysisQuery>()
}

#[must_use]
pub(super) fn incomplete_analysis_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<IncompleteAnalysisQuery>()
}

//! Closed complete-program analysis query.

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use super::{
    CurrentSourceScopeInput, DeclarationQueryOutcome, ProgramFinalizationOutcome,
    ProgramPreparationOutcome, SemanticScopeKey,
};

struct ProgramAnalysisQuery;

/// Exact-current declaration rejection after its editor recovery continuation has completed.
#[derive(Debug)]
pub struct FailedDeclarationAnalysis {
    failure: Arc<super::IncompleteSemanticFailure>,
}

impl FailedDeclarationAnalysis {
    #[must_use]
    pub fn failure(&self) -> &super::IncompleteSemanticFailure {
        &self.failure
    }
}

/// Closed source-complete semantic outcome consumed by session analysis.
#[derive(Debug)]
pub enum ProgramAnalysisOutcome {
    Checked(Arc<super::FinalizedProgram>),
    NamesRejected(super::FailedProgramNameResolution),
    BodiesRejected(super::FailedProgramFinalization),
    PreparationRejected(super::RejectedProgramPreparation),
    DeclarationsRejected(FailedDeclarationAnalysis),
    Failed(Arc<super::SemanticQueryFailure>),
}

/// One source-complete semantic outcome inseparably paired with its exact discovery snapshot.
#[derive(Debug)]
pub struct ProgramAnalysisProduct {
    outcome: ProgramAnalysisOutcome,
    fingerprint: Fingerprint,
}

impl ProgramAnalysisProduct {
    #[must_use]
    pub const fn outcome(&self) -> &ProgramAnalysisOutcome {
        &self.outcome
    }
}

impl QueryValue for ProgramAnalysisProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for ProgramAnalysisQuery {
    type Key = SemanticScopeKey;
    type Value = ProgramAnalysisProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        if current.unit.has_syntax_errors() {
            return Ok(ProgramAnalysisProduct {
                outcome: ProgramAnalysisOutcome::Failed(Arc::new(
                    super::SemanticQueryFailure::InvalidStageTransition(
                        "complete-program query demanded for incomplete syntax",
                    ),
                )),
                fingerprint: current.fingerprint,
            });
        }
        let declarations = super::declarations(database, key.clone())?;
        let outcome = match declarations.outcome() {
            DeclarationQueryOutcome::Rejected(rejection) => match current.unit.compile_input() {
                Ok(input) => {
                    ProgramAnalysisOutcome::DeclarationsRejected(FailedDeclarationAnalysis {
                        failure: Arc::new(super::analyze_declaration_failure(
                            &input,
                            rejection.failure(),
                        )),
                    })
                }
                Err(error) => ProgramAnalysisOutcome::Failed(Arc::new(error.into())),
            },
            DeclarationQueryOutcome::Failed(failure) => {
                ProgramAnalysisOutcome::Failed(Arc::clone(failure))
            }
            DeclarationQueryOutcome::Accepted(_) => {
                let preparation = super::prepared_program(database, key.clone())?;
                match preparation.outcome() {
                    ProgramPreparationOutcome::Rejected(rejection) => {
                        ProgramAnalysisOutcome::PreparationRejected(rejection.clone())
                    }
                    ProgramPreparationOutcome::Failed(failure) => {
                        ProgramAnalysisOutcome::Failed(Arc::clone(failure))
                    }
                    ProgramPreparationOutcome::Prepared(_) => {
                        let finalization = super::finalized_program(database, key.clone())?;
                        match finalization.outcome() {
                            ProgramFinalizationOutcome::Checked(program) => {
                                ProgramAnalysisOutcome::Checked(Arc::clone(program))
                            }
                            ProgramFinalizationOutcome::NamesRejected(rejection) => {
                                ProgramAnalysisOutcome::NamesRejected(rejection.clone())
                            }
                            ProgramFinalizationOutcome::Failed(rejection) => {
                                ProgramAnalysisOutcome::BodiesRejected(rejection.clone())
                            }
                            ProgramFinalizationOutcome::QueryFailed(failure) => {
                                ProgramAnalysisOutcome::Failed(Arc::clone(failure))
                            }
                        }
                    }
                }
            }
        };
        Ok(ProgramAnalysisProduct {
            outcome,
            fingerprint: current.fingerprint,
        })
    }
}

/// Demands the sole closed semantic outcome for one source-complete scope.
///
/// # Errors
///
/// Returns computation-kernel failures. Authored rejection and missing compiler authority are
/// retained as explicit domain outcomes.
pub(super) fn analyzed_program(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<ProgramAnalysisProduct>, ComputationError> {
    database.query::<ProgramAnalysisQuery>(key)
}

#[must_use]
pub(super) fn program_analysis_execution_count(database: &Database) -> u64 {
    database.execution_count::<ProgramAnalysisQuery>()
}

#[must_use]
pub(super) fn program_analysis_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ProgramAnalysisQuery>()
}

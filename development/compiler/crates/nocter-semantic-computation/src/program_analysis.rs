use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use crate::{
    CurrentSourceScopeInput, DeclarationQueryOutcome, ProgramFinalizationOutcome,
    ProgramPreparationOutcome, SemanticScopeKey,
};

struct ProgramAnalysisQuery;

/// Required semantic authority that was absent from an otherwise valid computation revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramAnalysisUnavailable {
    Declarations,
    DeclarationRecovery,
    Preparation,
    Finalization,
}

impl std::fmt::Display for ProgramAnalysisUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Declarations => "declaration",
            Self::DeclarationRecovery => "declaration-recovery",
            Self::Preparation => "program-preparation",
            Self::Finalization => "whole-program finalization",
        })
    }
}

/// Exact-current declaration rejection after its editor recovery continuation has completed.
#[derive(Debug)]
pub struct FailedDeclarationAnalysis {
    failure: Arc<crate::IncompleteSemanticFailure>,
}

impl FailedDeclarationAnalysis {
    #[must_use]
    pub fn failure(&self) -> &crate::IncompleteSemanticFailure {
        &self.failure
    }
}

/// Closed source-complete semantic outcome consumed by session analysis.
#[derive(Debug)]
pub enum ProgramAnalysisOutcome {
    Checked(Arc<crate::FinalizedProgram>),
    NamesRejected(crate::FailedProgramNameResolution),
    BodiesRejected(crate::FailedProgramFinalization),
    PreparationRejected(crate::RejectedProgramPreparation),
    DeclarationsRejected(FailedDeclarationAnalysis),
    Unavailable(ProgramAnalysisUnavailable),
}

/// One source-complete semantic outcome inseparably paired with its exact discovery snapshot.
#[derive(Debug)]
pub struct ProgramAnalysisProduct {
    unit: Arc<nocter_discovery::DiscoveredUnit>,
    outcome: ProgramAnalysisOutcome,
    fingerprint: Fingerprint,
}

impl ProgramAnalysisProduct {
    #[must_use]
    pub fn unit(&self) -> &Arc<nocter_discovery::DiscoveredUnit> {
        &self.unit
    }

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
        let declarations = crate::declarations(database, key.clone())?;
        let outcome = match declarations.outcome() {
            DeclarationQueryOutcome::Rejected(rejection) => {
                let failure =
                    current.unit.compile_input().ok().map(|input| {
                        crate::analyze_declaration_failure(&input, rejection.failure())
                    });
                failure.map_or(
                    ProgramAnalysisOutcome::Unavailable(
                        ProgramAnalysisUnavailable::DeclarationRecovery,
                    ),
                    |failure| {
                        ProgramAnalysisOutcome::DeclarationsRejected(FailedDeclarationAnalysis {
                            failure: Arc::new(failure),
                        })
                    },
                )
            }
            DeclarationQueryOutcome::Unavailable => {
                ProgramAnalysisOutcome::Unavailable(ProgramAnalysisUnavailable::Declarations)
            }
            DeclarationQueryOutcome::Accepted(_) => {
                let preparation = crate::prepared_program(database, key.clone())?;
                match preparation.outcome() {
                    ProgramPreparationOutcome::Rejected(rejection) => {
                        ProgramAnalysisOutcome::PreparationRejected(rejection.clone())
                    }
                    ProgramPreparationOutcome::Unavailable => {
                        ProgramAnalysisOutcome::Unavailable(ProgramAnalysisUnavailable::Preparation)
                    }
                    ProgramPreparationOutcome::Prepared(_) => {
                        let finalization = crate::finalized_program(database, key.clone())?;
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
                            ProgramFinalizationOutcome::Unavailable => {
                                ProgramAnalysisOutcome::Unavailable(
                                    ProgramAnalysisUnavailable::Finalization,
                                )
                            }
                        }
                    }
                }
            }
        };
        Ok(ProgramAnalysisProduct {
            unit: Arc::clone(&current.unit),
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
pub fn analyzed_program(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<ProgramAnalysisProduct>, ComputationError> {
    database.query::<ProgramAnalysisQuery>(key)
}

#[must_use]
pub fn program_analysis_execution_count(database: &Database) -> u64 {
    database.execution_count::<ProgramAnalysisQuery>()
}

#[must_use]
pub fn program_analysis_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ProgramAnalysisQuery>()
}

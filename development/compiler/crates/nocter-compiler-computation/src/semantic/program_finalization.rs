//! Canonical whole-program finalization query.

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use super::{
    CurrentSourceScopeInput, DeclarationQuery, DeclarationQueryOutcome, ProgramPreparationOutcome,
    SemanticScopeKey,
};

struct ProgramFinalizationQuery;

/// Exact-current whole-program semantic result after canonical body replay.
#[derive(Debug)]
pub struct FinalizedProgram {
    declarations: Arc<nocter_declaration_lowering::ReusableDeclarations>,
    checked: nocter_checking::CheckedProgramOutput,
}

impl FinalizedProgram {
    #[must_use]
    pub fn declarations(&self) -> &nocter_declaration_lowering::ReusableDeclarations {
        &self.declarations
    }

    #[must_use]
    pub fn current_branch(&self) -> nocter_checking::CheckedProgramOutput {
        self.checked.current_branch()
    }
}

/// Exact-current whole-program checking failure after canonical body replay.
#[derive(Clone, Debug)]
pub struct FailedProgramFinalization {
    failure: Arc<nocter_checking::BodyCheckFailure>,
}

/// Exact-current lexical rejection materialized from the complete body-name query set.
#[derive(Clone, Debug)]
pub struct FailedProgramNameResolution {
    failure: Arc<nocter_checking::QueriedNameResolutionFailure>,
}

impl FailedProgramNameResolution {
    #[must_use]
    pub fn failure(&self) -> &nocter_checking::QueriedNameResolutionFailure {
        &self.failure
    }
}

impl FailedProgramFinalization {
    #[must_use]
    pub fn failure(&self) -> &nocter_checking::BodyCheckFailure {
        &self.failure
    }
}

#[derive(Debug)]
pub enum ProgramFinalizationOutcome {
    Checked(Arc<FinalizedProgram>),
    NamesRejected(FailedProgramNameResolution),
    Failed(FailedProgramFinalization),
    QueryFailed(Arc<super::SemanticQueryFailure>),
}

#[derive(Debug)]
pub struct ProgramFinalizationProduct {
    outcome: ProgramFinalizationOutcome,
    fingerprint: Fingerprint,
}

impl ProgramFinalizationProduct {
    #[must_use]
    pub const fn outcome(&self) -> &ProgramFinalizationOutcome {
        &self.outcome
    }
}

impl QueryValue for ProgramFinalizationProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for ProgramFinalizationQuery {
    type Key = SemanticScopeKey;
    type Value = ProgramFinalizationProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let declarations = database.query::<DeclarationQuery>(key.clone())?;
        let declarations = match declarations.outcome() {
            DeclarationQueryOutcome::Accepted(declarations) => declarations,
            DeclarationQueryOutcome::Failed(failure) => {
                return query_failed(database, key, Arc::clone(failure));
            }
            DeclarationQueryOutcome::Rejected(_) => {
                return invalid_transition(
                    database,
                    key,
                    "program finalization demanded after declaration rejection",
                );
            }
        };
        let preparation = super::prepared_program(database, key.clone())?;
        match preparation.outcome() {
            ProgramPreparationOutcome::Prepared(_) => {}
            ProgramPreparationOutcome::Failed(failure) => {
                return query_failed(database, key, Arc::clone(failure));
            }
            ProgramPreparationOutcome::Rejected(_) => {
                return invalid_transition(
                    database,
                    key,
                    "program finalization demanded after preparation rejection",
                );
            }
        }
        let body_names = match super::resolved_body_names(database, key)? {
            Ok(body_names) => body_names,
            Err(failure) => return query_failed(database, key, failure),
        };
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let context =
            database.query::<super::body_context::BodySemanticContextQuery>(key.clone())?;
        if !body_names.rejections().is_empty() {
            let failure = match context.materialize_name_rejection(&body_names) {
                Ok(failure) => failure,
                Err(failure) => return query_failed(database, key, failure),
            };
            return Ok(ProgramFinalizationProduct {
                outcome: ProgramFinalizationOutcome::NamesRejected(FailedProgramNameResolution {
                    failure: Arc::new(failure),
                }),
                fingerprint: current.fingerprint,
            });
        }
        let typed_bodies = match super::typed_bodies(database, key)? {
            Ok(typed_bodies) => typed_bodies,
            Err(failure) => return query_failed(database, key, failure),
        };
        let checked = match context.finalize(&body_names, &typed_bodies) {
            Ok(checked) => checked,
            Err(failure) => return query_failed(database, key, failure),
        };
        let outcome = match checked {
            nocter_checking::QueriedProgramFinalizationOutcome::Checked(checked) => {
                ProgramFinalizationOutcome::Checked(Arc::new(FinalizedProgram {
                    declarations: Arc::clone(declarations),
                    checked: *checked,
                }))
            }
            nocter_checking::QueriedProgramFinalizationOutcome::Failed(failure) => {
                ProgramFinalizationOutcome::Failed(FailedProgramFinalization {
                    failure: Arc::from(failure),
                })
            }
        };
        Ok(ProgramFinalizationProduct {
            outcome,
            fingerprint: current.fingerprint,
        })
    }
}

fn invalid_transition(
    database: &Database,
    key: &SemanticScopeKey,
    message: &'static str,
) -> Result<ProgramFinalizationProduct, ComputationError> {
    query_failed(
        database,
        key,
        Arc::new(super::SemanticQueryFailure::InvalidStageTransition(message)),
    )
}

fn query_failed(
    database: &Database,
    key: &SemanticScopeKey,
    failure: Arc<super::SemanticQueryFailure>,
) -> Result<ProgramFinalizationProduct, ComputationError> {
    let current = database.input::<CurrentSourceScopeInput>(key)?;
    Ok(ProgramFinalizationProduct {
        outcome: ProgramFinalizationOutcome::QueryFailed(failure),
        fingerprint: current.fingerprint,
    })
}

/// Demands canonical body replay and whole-program semantic finalization.
///
/// # Errors
///
/// Returns only computation-kernel failures. Compiler-domain finalization failure is an ordinary
/// exact-current outcome.
pub(super) fn finalized_program(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<ProgramFinalizationProduct>, ComputationError> {
    database.query::<ProgramFinalizationQuery>(key)
}

#[must_use]
pub(super) fn finalization_execution_count(database: &Database) -> u64 {
    database.execution_count::<ProgramFinalizationQuery>()
}

#[must_use]
pub(super) fn finalization_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ProgramFinalizationQuery>()
}

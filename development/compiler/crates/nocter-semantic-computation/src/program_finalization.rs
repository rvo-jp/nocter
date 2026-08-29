use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use crate::{
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
    Unavailable,
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
        let DeclarationQueryOutcome::Accepted(declarations) = declarations.outcome() else {
            return unavailable(database, key);
        };
        let preparation = crate::prepared_program(database, key.clone())?;
        let ProgramPreparationOutcome::Prepared(_) = preparation.outcome() else {
            return unavailable(database, key);
        };
        let Some(body_names) = crate::resolved_body_names(database, key)? else {
            return unavailable(database, key);
        };
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let context =
            database.query::<crate::body_context::BodySemanticContextQuery>(key.clone())?;
        if !body_names.rejections().is_empty() {
            let Some(failure) = context.materialize_name_rejection(&body_names) else {
                return unavailable(database, key);
            };
            return Ok(ProgramFinalizationProduct {
                outcome: ProgramFinalizationOutcome::NamesRejected(FailedProgramNameResolution {
                    failure: Arc::new(failure),
                }),
                fingerprint: current.fingerprint,
            });
        }
        let Some(typed_bodies) = crate::typed_bodies(database, key)? else {
            return unavailable(database, key);
        };
        let Some(checked) = context.finalize(&body_names, &typed_bodies) else {
            return unavailable(database, key);
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

fn unavailable(
    database: &Database,
    key: &SemanticScopeKey,
) -> Result<ProgramFinalizationProduct, ComputationError> {
    let current = database.input::<CurrentSourceScopeInput>(key)?;
    Ok(ProgramFinalizationProduct {
        outcome: ProgramFinalizationOutcome::Unavailable,
        fingerprint: current.fingerprint,
    })
}

/// Demands canonical body replay and whole-program semantic finalization.
///
/// # Errors
///
/// Returns only computation-kernel failures. Compiler-domain finalization failure is an ordinary
/// exact-current outcome.
pub fn finalized_program(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<ProgramFinalizationProduct>, ComputationError> {
    database.query::<ProgramFinalizationQuery>(key)
}

#[must_use]
pub fn finalization_execution_count(database: &Database) -> u64 {
    database.execution_count::<ProgramFinalizationQuery>()
}

#[must_use]
pub fn finalization_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ProgramFinalizationQuery>()
}

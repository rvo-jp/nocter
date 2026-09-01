//! Source-neutral program preparation query.

#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use super::{
    CurrentSourceScopeInput, DeclarationQuery, DeclarationQueryOutcome, DeclarationScopeInput,
    SemanticScopeKey,
};

struct ProgramPreparationQuery;

/// Reusable program-wide checking preparation or an explicitly uncached current failure.
#[derive(Debug)]
pub enum ProgramPreparationOutcome {
    Prepared(Arc<nocter_checking::ReusablePreparedProgram>),
    Rejected(RejectedProgramPreparation),
    Failed(Arc<super::SemanticQueryFailure>),
}

/// One program-preparation rejection stored only inside an exact-current query product.
#[derive(Clone, Debug)]
pub struct RejectedProgramPreparation {
    rejection: Arc<nocter_checking::QueriedProgramPreparationRejection>,
}

impl RejectedProgramPreparation {
    #[must_use]
    pub fn rejection(&self) -> &nocter_checking::QueriedProgramPreparationRejection {
        &self.rejection
    }
}

#[derive(Debug)]
pub struct ProgramPreparationProduct {
    outcome: ProgramPreparationOutcome,
    fingerprint: Fingerprint,
}

impl ProgramPreparationProduct {
    #[must_use]
    pub const fn outcome(&self) -> &ProgramPreparationOutcome {
        &self.outcome
    }
}

impl QueryValue for ProgramPreparationProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for ProgramPreparationQuery {
    type Key = SemanticScopeKey;
    type Value = ProgramPreparationProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let declarations = database.query::<DeclarationQuery>(key.clone())?;
        let declaration_fingerprint = declarations.fingerprint();
        let DeclarationQueryOutcome::Accepted(declarations) = declarations.outcome() else {
            let current = database.input::<CurrentSourceScopeInput>(key)?;
            let failure = match declarations.outcome() {
                DeclarationQueryOutcome::Failed(failure) => Arc::clone(failure),
                DeclarationQueryOutcome::Rejected(_) => {
                    Arc::new(super::SemanticQueryFailure::InvalidStageTransition(
                        "program preparation demanded after declaration rejection",
                    ))
                }
                DeclarationQueryOutcome::Accepted(_) => {
                    Arc::new(super::SemanticQueryFailure::InvalidStageTransition(
                        "accepted declaration branch was not selected",
                    ))
                }
            };
            return Ok(ProgramPreparationProduct {
                outcome: ProgramPreparationOutcome::Failed(failure),
                fingerprint: current.fingerprint,
            });
        };
        let semantic = database.input::<DeclarationScopeInput>(key)?;
        let input = match semantic.unit.compile_input() {
            Ok(input) => input,
            Err(error) => return failed(database, key, error.into()),
        };
        let projection = match declarations.materialize_authority_projection(&input) {
            Ok(projection) => projection,
            Err(error) => return failed(database, key, error.into()),
        };
        let (bindings, source_index) = projection.into_parts();
        let outcome = match nocter_checking::prepare_reusable_program_for_query(
            &input,
            declarations.checking_branch(),
            &bindings,
            source_index,
        ) {
            Ok(outcome) => outcome,
            Err(error) => return failed(database, key, error.into()),
        };
        match outcome {
            nocter_checking::ReusableProgramPreparationQueryOutcome::Prepared(prepared) => {
                Ok(ProgramPreparationProduct {
                    outcome: ProgramPreparationOutcome::Prepared(Arc::from(prepared)),
                    fingerprint: declaration_fingerprint,
                })
            }
            nocter_checking::ReusableProgramPreparationQueryOutcome::Rejected(rejection) => {
                let current = database.input::<CurrentSourceScopeInput>(key)?;
                Ok(ProgramPreparationProduct {
                    outcome: ProgramPreparationOutcome::Rejected(RejectedProgramPreparation {
                        rejection: Arc::from(rejection),
                    }),
                    fingerprint: current.fingerprint,
                })
            }
        }
    }
}

fn failed(
    database: &Database,
    key: &SemanticScopeKey,
    failure: super::SemanticQueryFailure,
) -> Result<ProgramPreparationProduct, ComputationError> {
    let current = database.input::<CurrentSourceScopeInput>(key)?;
    Ok(ProgramPreparationProduct {
        outcome: ProgramPreparationOutcome::Failed(Arc::new(failure)),
        fingerprint: current.fingerprint,
    })
}

/// Demands source-neutral program-wide checking authorities for one semantic scope.
///
/// # Errors
///
/// Returns only computation-kernel failures. Authored preparation rejection is a first-class
/// exact-current outcome; compiler-domain integrity failures remain typed in the product.
pub(super) fn prepared_program(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<ProgramPreparationProduct>, ComputationError> {
    database.query::<ProgramPreparationQuery>(key)
}

#[must_use]
pub(super) fn preparation_execution_count(database: &Database) -> u64 {
    database.execution_count::<ProgramPreparationQuery>()
}

#[must_use]
pub(super) fn preparation_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ProgramPreparationQuery>()
}

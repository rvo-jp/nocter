use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use crate::{
    CurrentSourceScopeInput, DeclarationQuery, DeclarationQueryOutcome, DeclarationScopeInput,
    SemanticScopeKey,
};

struct ProgramPreparationQuery;

/// Reusable program-wide checking preparation or an explicitly uncached current failure.
#[derive(Debug)]
pub enum ProgramPreparationOutcome {
    Prepared(Arc<nocter_checking::ReusablePreparedProgram>),
    Rejected(RejectedProgramPreparation),
    Unavailable,
}

/// One program-preparation rejection paired with the exact source domain that produced it.
#[derive(Clone, Debug)]
pub struct RejectedProgramPreparation {
    unit: Arc<nocter_discovery::DiscoveredUnit>,
    rejection: Arc<nocter_checking::QueriedProgramPreparationRejection>,
}

impl RejectedProgramPreparation {
    #[must_use]
    pub fn unit(&self) -> &Arc<nocter_discovery::DiscoveredUnit> {
        &self.unit
    }

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
            return Ok(ProgramPreparationProduct {
                outcome: ProgramPreparationOutcome::Unavailable,
                fingerprint: current.fingerprint,
            });
        };
        let semantic = database.input::<DeclarationScopeInput>(key)?;
        let outcome = semantic.unit.compile_input().ok().and_then(|input| {
            let projection = declarations.materialize_authority_projection(&input).ok()?;
            let (bindings, source_index) = projection.into_parts();
            nocter_checking::prepare_reusable_program_for_query(
                &input,
                declarations.checking_branch(),
                &bindings,
                source_index,
            )
            .ok()
        });
        match outcome {
            Some(nocter_checking::ReusableProgramPreparationQueryOutcome::Prepared(prepared)) => {
                Ok(ProgramPreparationProduct {
                    outcome: ProgramPreparationOutcome::Prepared(Arc::from(prepared)),
                    fingerprint: declaration_fingerprint,
                })
            }
            Some(nocter_checking::ReusableProgramPreparationQueryOutcome::Rejected(rejection)) => {
                let current = database.input::<CurrentSourceScopeInput>(key)?;
                Ok(ProgramPreparationProduct {
                    outcome: ProgramPreparationOutcome::Rejected(RejectedProgramPreparation {
                        unit: Arc::clone(&current.unit),
                        rejection: Arc::from(rejection),
                    }),
                    fingerprint: current.fingerprint,
                })
            }
            None => {
                let current = database.input::<CurrentSourceScopeInput>(key)?;
                Ok(ProgramPreparationProduct {
                    outcome: ProgramPreparationOutcome::Unavailable,
                    fingerprint: current.fingerprint,
                })
            }
        }
    }
}

/// Demands source-neutral program-wide checking authorities for one semantic scope.
///
/// # Errors
///
/// Returns only computation-kernel failures. Authored preparation rejection is a first-class
/// exact-current outcome; unavailable is reserved for missing input or internal failure.
pub fn prepared_program(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<ProgramPreparationProduct>, ComputationError> {
    database.query::<ProgramPreparationQuery>(key)
}

#[must_use]
pub fn preparation_execution_count(database: &Database) -> u64 {
    database.execution_count::<ProgramPreparationQuery>()
}

#[must_use]
pub fn preparation_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ProgramPreparationQuery>()
}

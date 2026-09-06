//! Reusable source-neutral whole-program relation analysis.

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use super::{SemanticScopeKey, program_materialization};

struct ProgramRelationInputQuery;
struct ProgramRelationQuery;

struct ProgramRelationInput {
    materialization: Arc<program_materialization::ProgramMaterializationProduct>,
    fingerprint: Fingerprint,
}

#[derive(Debug)]
pub(super) enum ProgramRelationOutcome {
    Analyzed(Arc<nocter_checking::ReusableProgramRelationOutcome>),
    Unavailable,
}

pub(super) struct ProgramRelationProduct {
    outcome: ProgramRelationOutcome,
    fingerprint: Fingerprint,
}

impl ProgramRelationProduct {
    #[must_use]
    pub(super) const fn outcome(&self) -> &ProgramRelationOutcome {
        &self.outcome
    }
}

impl QueryValue for ProgramRelationInput {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl QueryValue for ProgramRelationProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for ProgramRelationInputQuery {
    type Key = SemanticScopeKey;
    type Value = ProgramRelationInput;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let materialization = program_materialization::materialized_program(database, key.clone())?;
        Ok(ProgramRelationInput {
            fingerprint: materialization.relation_fingerprint(),
            materialization,
        })
    }
}

impl Query for ProgramRelationQuery {
    type Key = SemanticScopeKey;
    type Value = ProgramRelationProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let input = database.query::<ProgramRelationInputQuery>(key.clone())?;
        let outcome = match input.materialization.outcome() {
            program_materialization::ProgramMaterializationOutcome::Materialized(materialized) => {
                ProgramRelationOutcome::Analyzed(Arc::new(
                    nocter_checking::analyze_queried_program_relations(materialized),
                ))
            }
            program_materialization::ProgramMaterializationOutcome::NamesRejected(_)
            | program_materialization::ProgramMaterializationOutcome::Failed(_)
            | program_materialization::ProgramMaterializationOutcome::QueryFailed(_) => {
                ProgramRelationOutcome::Unavailable
            }
        };
        Ok(ProgramRelationProduct {
            outcome,
            fingerprint: input.fingerprint,
        })
    }
}

pub(super) fn relations_for_program(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<ProgramRelationProduct>, ComputationError> {
    database.query::<ProgramRelationQuery>(key)
}

#[must_use]
pub(super) fn execution_count(database: &Database) -> u64 {
    database.execution_count::<ProgramRelationQuery>()
}

#[must_use]
pub(super) fn reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ProgramRelationQuery>()
}

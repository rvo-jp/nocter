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
    Unavailable,
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
        let prepared = semantic.unit.compile_input().ok().and_then(|input| {
            let projection = declarations.materialize_authority_projection(&input).ok()?;
            let (bindings, source_index) = projection.into_parts();
            nocter_checking::prepare_reusable_program(
                &input,
                declarations.checking_branch(),
                &bindings,
                source_index.diagnostic_origins(),
            )
            .ok()
        });
        if let Some(prepared) = prepared {
            Ok(ProgramPreparationProduct {
                outcome: ProgramPreparationOutcome::Prepared(Arc::new(prepared)),
                fingerprint: declaration_fingerprint,
            })
        } else {
            let current = database.input::<CurrentSourceScopeInput>(key)?;
            Ok(ProgramPreparationProduct {
                outcome: ProgramPreparationOutcome::Unavailable,
                fingerprint: current.fingerprint,
            })
        }
    }
}

/// Demands source-neutral program-wide checking authorities for one semantic scope.
///
/// # Errors
///
/// Returns only computation-kernel failures. Compiler rejection remains an ordinary unavailable
/// outcome until query-owned recovery migration is complete.
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

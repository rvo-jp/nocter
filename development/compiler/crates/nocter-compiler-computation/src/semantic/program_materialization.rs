//! Exact-current canonical body replay before reusable program-relation analysis.

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use super::{
    CurrentSourceScopeInput, DeclarationQuery, DeclarationQueryOutcome, ProgramPreparationOutcome,
    SemanticBodyKey, SemanticScopeKey,
};

pub(super) struct ProgramMaterializationQuery;

#[derive(Debug)]
pub(super) enum ProgramMaterializationOutcome {
    Materialized(Arc<nocter_checking::QueriedProgramMaterialization>),
    NamesRejected(Arc<nocter_checking::QueriedNameResolutionFailure>),
    Failed(Arc<nocter_checking::BodyCheckFailure>),
    QueryFailed(Arc<super::SemanticQueryFailure>),
}

#[derive(Debug)]
pub(super) struct ProgramMaterializationProduct {
    outcome: ProgramMaterializationOutcome,
    fingerprint: Fingerprint,
    relation_fingerprint: Fingerprint,
}

impl ProgramMaterializationProduct {
    #[must_use]
    pub(super) const fn outcome(&self) -> &ProgramMaterializationOutcome {
        &self.outcome
    }

    #[must_use]
    pub(super) const fn relation_fingerprint(&self) -> Fingerprint {
        self.relation_fingerprint
    }
}

impl QueryValue for ProgramMaterializationProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for ProgramMaterializationQuery {
    type Key = SemanticScopeKey;
    type Value = ProgramMaterializationProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let declaration_product = database.query::<DeclarationQuery>(key.clone())?;
        let declaration_fingerprint = declaration_product.fingerprint();
        let declarations = match declaration_product.outcome() {
            DeclarationQueryOutcome::Accepted(declarations) => declarations,
            DeclarationQueryOutcome::Failed(failure) => {
                return Ok(query_failed(current.fingerprint, Arc::clone(failure)));
            }
            DeclarationQueryOutcome::Rejected(_) => {
                return Ok(invalid_transition(
                    current.fingerprint,
                    "program materialization demanded after declaration rejection",
                ));
            }
        };
        let preparation = super::prepared_program(database, key.clone())?;
        match preparation.outcome() {
            ProgramPreparationOutcome::Prepared(_) => {}
            ProgramPreparationOutcome::Failed(failure) => {
                return Ok(query_failed(current.fingerprint, Arc::clone(failure)));
            }
            ProgramPreparationOutcome::Rejected(_) => {
                return Ok(invalid_transition(
                    current.fingerprint,
                    "program materialization demanded after preparation rejection",
                ));
            }
        }
        let body_names = match super::resolved_body_names(database, key)? {
            Ok(body_names) => body_names,
            Err(failure) => return Ok(query_failed(current.fingerprint, failure)),
        };
        let context =
            database.query::<super::body_context::BodySemanticContextQuery>(key.clone())?;
        if !body_names.rejections().is_empty() {
            let failure = match context.materialize_name_rejection(&body_names) {
                Ok(failure) => failure,
                Err(failure) => return Ok(query_failed(current.fingerprint, failure)),
            };
            return Ok(ProgramMaterializationProduct {
                outcome: ProgramMaterializationOutcome::NamesRejected(Arc::new(failure)),
                fingerprint: current.fingerprint,
                relation_fingerprint: current.fingerprint,
            });
        }
        let typed_bodies = match super::typed_bodies(database, key)? {
            Ok(typed_bodies) => typed_bodies,
            Err(failure) => return Ok(query_failed(current.fingerprint, failure)),
        };
        let materialized = match context.materialize(&body_names, &typed_bodies) {
            Ok(materialized) => materialized,
            Err(failure) => return Ok(query_failed(current.fingerprint, failure)),
        };
        let outcome = match materialized {
            nocter_checking::QueriedProgramMaterializationOutcome::Materialized(materialized) => {
                ProgramMaterializationOutcome::Materialized(Arc::from(materialized))
            }
            nocter_checking::QueriedProgramMaterializationOutcome::Failed(failure) => {
                ProgramMaterializationOutcome::Failed(Arc::from(failure))
            }
        };
        let relation_fingerprint =
            if matches!(&outcome, ProgramMaterializationOutcome::Materialized(_)) {
                structural_program_fingerprint(
                    database,
                    key,
                    declarations,
                    declaration_fingerprint,
                )?
            } else {
                current.fingerprint
            };
        Ok(ProgramMaterializationProduct {
            outcome,
            fingerprint: current.fingerprint,
            relation_fingerprint,
        })
    }
}

fn structural_program_fingerprint(
    database: &Database,
    key: &SemanticScopeKey,
    declarations: &nocter_declaration_lowering::ReusableDeclarations,
    declaration_fingerprint: Fingerprint,
) -> Result<Fingerprint, ComputationError> {
    let mut bytes = declaration_fingerprint.digest().to_vec();
    for identity in declarations.body_identities() {
        let body_key = SemanticBodyKey::for_identity(key.clone(), identity);
        let body = database.input::<super::BodySourceInput>(body_key.source())?;
        bytes.extend_from_slice(&body.structural_fingerprint().digest());
    }
    Ok(Fingerprint::from_bytes(&bytes))
}

fn invalid_transition(
    fingerprint: Fingerprint,
    message: &'static str,
) -> ProgramMaterializationProduct {
    query_failed(
        fingerprint,
        Arc::new(super::SemanticQueryFailure::InvalidStageTransition(message)),
    )
}

fn query_failed(
    fingerprint: Fingerprint,
    failure: Arc<super::SemanticQueryFailure>,
) -> ProgramMaterializationProduct {
    ProgramMaterializationProduct {
        outcome: ProgramMaterializationOutcome::QueryFailed(failure),
        fingerprint,
        relation_fingerprint: fingerprint,
    }
}

pub(super) fn materialized_program(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<ProgramMaterializationProduct>, ComputationError> {
    database.query::<ProgramMaterializationQuery>(key)
}

#[must_use]
pub(super) fn execution_count(database: &Database) -> u64 {
    database.execution_count::<ProgramMaterializationQuery>()
}

#[must_use]
pub(super) fn reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ProgramMaterializationQuery>()
}

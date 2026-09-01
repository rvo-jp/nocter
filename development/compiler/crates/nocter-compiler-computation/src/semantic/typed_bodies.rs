//! Source-neutral typed-body queries.

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use super::{
    CurrentSourceScopeInput, DeclarationQuery, DeclarationQueryOutcome, SemanticBodyKey,
    SemanticScopeKey,
};

struct TypedBodyQuery;

#[derive(Debug)]
pub enum TypedBodyQueryOutcome {
    Checked(Arc<nocter_checking::ReusableCheckedBody>),
    Rejected(Arc<nocter_checking::QueriedBodyRejection>),
    Failed(Arc<super::SemanticQueryFailure>),
}

#[derive(Debug)]
pub struct TypedBodyQueryProduct {
    outcome: TypedBodyQueryOutcome,
    fingerprint: Fingerprint,
}

/// Complete canonical set of independently checked source-neutral bodies for one program.
#[derive(Debug)]
pub struct TypedBodySet {
    entries: Box<[Arc<nocter_checking::ReusableCheckedBody>]>,
    rejections: Box<[Arc<nocter_checking::QueriedBodyRejection>]>,
}

impl TypedBodySet {
    #[must_use]
    pub fn entries(&self) -> &[Arc<nocter_checking::ReusableCheckedBody>] {
        &self.entries
    }

    #[must_use]
    pub fn rejections(&self) -> &[Arc<nocter_checking::QueriedBodyRejection>] {
        &self.rejections
    }
}

impl TypedBodyQueryProduct {
    #[must_use]
    pub const fn outcome(&self) -> &TypedBodyQueryOutcome {
        &self.outcome
    }
}

impl QueryValue for TypedBodyQueryProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for TypedBodyQuery {
    type Key = SemanticBodyKey;
    type Value = TypedBodyQueryProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let name_product = super::resolve_body_name(database, key.clone())?;
        let body = database.input::<super::BodySourceInput>(key.source())?;
        let exact_body = match name_product.bind_exact_body(&body) {
            Ok(super::body_names::ExactBodyNamesBinding::Ready(exact_body)) => exact_body,
            Ok(super::body_names::ExactBodyNamesBinding::Rejected) => {
                return failed(
                    database,
                    key,
                    Arc::new(super::SemanticQueryFailure::InvalidStageTransition(
                        "typed-body query demanded after body-name rejection",
                    )),
                );
            }
            Ok(super::body_names::ExactBodyNamesBinding::Failed(failure)) => {
                return failed(database, key, Arc::clone(failure));
            }
            Err(failure) => return failed(database, key, Arc::new(failure)),
        };
        let context =
            database.query::<super::body_context::BodySemanticContextQuery>(key.scope().clone())?;
        let outcome = match context.check_body(exact_body) {
            Ok(outcome) => outcome,
            Err(failure) => return failed(database, key, failure),
        };
        let (outcome, fingerprint) = match outcome {
            nocter_checking::ReusableBodyQueryOutcome::Checked(checked) => {
                let mut fingerprint = name_product.fingerprint().digest().to_vec();
                fingerprint.extend_from_slice(&exact_body.fingerprint().digest());
                (
                    TypedBodyQueryOutcome::Checked(Arc::new(checked)),
                    Fingerprint::from_bytes(&fingerprint),
                )
            }
            nocter_checking::ReusableBodyQueryOutcome::Rejected(rejection) => (
                TypedBodyQueryOutcome::Rejected(Arc::new(rejection)),
                database
                    .input::<CurrentSourceScopeInput>(key.scope())?
                    .fingerprint,
            ),
        };
        Ok(TypedBodyQueryProduct {
            outcome,
            fingerprint,
        })
    }
}

fn failed(
    database: &Database,
    key: &SemanticBodyKey,
    failure: Arc<super::SemanticQueryFailure>,
) -> Result<TypedBodyQueryProduct, ComputationError> {
    let current = database.input::<CurrentSourceScopeInput>(key.scope())?;
    Ok(TypedBodyQueryProduct {
        outcome: TypedBodyQueryOutcome::Failed(failure),
        fingerprint: current.fingerprint,
    })
}

/// Demands the source-neutral typed result for one stable body identity.
///
/// # Errors
///
/// Returns only computation-kernel failures. Authored rejection is a first-class exact-current
/// query outcome; an earlier rejection and an integrity failure remain distinct products.
pub(super) fn typed_body(
    database: &Database,
    key: SemanticBodyKey,
) -> Result<Arc<TypedBodyQueryProduct>, ComputationError> {
    database.query::<TypedBodyQuery>(key)
}

/// Demands every body declared by one program and returns them in canonical `BodyId` order.
///
/// # Errors
///
/// Returns computation-kernel failures from an individual body demand.
pub(super) fn typed_bodies(
    database: &Database,
    scope: &SemanticScopeKey,
) -> Result<super::SemanticStage<TypedBodySet>, ComputationError> {
    let declarations = database.query::<DeclarationQuery>(scope.clone())?;
    let DeclarationQueryOutcome::Accepted(declarations) = declarations.outcome() else {
        let failure = match declarations.outcome() {
            DeclarationQueryOutcome::Failed(failure) => Arc::clone(failure),
            DeclarationQueryOutcome::Rejected(_) => {
                Arc::new(super::SemanticQueryFailure::InvalidStageTransition(
                    "typed-body set demanded after declaration rejection",
                ))
            }
            DeclarationQueryOutcome::Accepted(_) => {
                Arc::new(super::SemanticQueryFailure::InvalidStageTransition(
                    "accepted declaration branch was not selected",
                ))
            }
        };
        return Ok(Err(failure));
    };
    let mut entries = Vec::with_capacity(declarations.body_identities().len());
    let mut rejections = Vec::new();
    for identity in declarations.body_identities() {
        let product = typed_body(
            database,
            SemanticBodyKey::for_identity(scope.clone(), identity),
        )?;
        match product.outcome() {
            TypedBodyQueryOutcome::Checked(checked) => entries.push(Arc::clone(checked)),
            TypedBodyQueryOutcome::Rejected(rejection) => {
                rejections.push(Arc::clone(rejection));
            }
            TypedBodyQueryOutcome::Failed(failure) => {
                return Ok(Err(Arc::clone(failure)));
            }
        }
    }
    entries.sort_unstable_by_key(|checked| checked.body());
    rejections.sort_unstable_by_key(|rejection| rejection.body());
    Ok(Ok(TypedBodySet {
        entries: entries.into_boxed_slice(),
        rejections: rejections.into_boxed_slice(),
    }))
}

#[must_use]
pub(super) fn typed_body_execution_count(database: &Database) -> u64 {
    database.execution_count::<TypedBodyQuery>()
}

#[must_use]
pub(super) fn typed_body_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<TypedBodyQuery>()
}

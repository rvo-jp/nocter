use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use crate::{
    BodyNameQueryOutcome, CurrentSourceScopeInput, DeclarationQuery, DeclarationQueryOutcome,
    SemanticBodyKey, SemanticScopeKey,
};

struct TypedBodyQuery;

#[derive(Debug)]
pub enum TypedBodyQueryOutcome {
    Checked(Arc<nocter_checking::ReusableCheckedBody>),
    Unavailable,
}

#[derive(Debug)]
pub struct TypedBodyQueryProduct {
    outcome: TypedBodyQueryOutcome,
    fingerprint: Fingerprint,
}

/// Complete canonical set of independently checked source-neutral bodies for one program.
#[derive(Debug)]
pub struct CheckedBodySet {
    entries: Box<[Arc<nocter_checking::ReusableCheckedBody>]>,
}

impl CheckedBodySet {
    #[must_use]
    pub fn entries(&self) -> &[Arc<nocter_checking::ReusableCheckedBody>] {
        &self.entries
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
        let name_product = crate::resolve_body_name(database, key.clone())?;
        let BodyNameQueryOutcome::Resolved(names) = name_product.outcome() else {
            return unavailable(database, key);
        };
        let body = database.input::<crate::BodySourceInput>(key.source())?;
        let context =
            database.query::<crate::body_context::BodySemanticContextQuery>(key.scope().clone())?;
        let Some(checked) = context.check_body(&body, names) else {
            return unavailable(database, key);
        };
        let mut fingerprint = name_product.fingerprint().digest().to_vec();
        fingerprint.extend_from_slice(&body.fingerprint.digest());
        Ok(TypedBodyQueryProduct {
            outcome: TypedBodyQueryOutcome::Checked(Arc::new(checked)),
            fingerprint: Fingerprint::from_bytes(&fingerprint),
        })
    }
}

fn unavailable(
    database: &Database,
    key: &SemanticBodyKey,
) -> Result<TypedBodyQueryProduct, ComputationError> {
    let current = database.input::<CurrentSourceScopeInput>(key.scope())?;
    Ok(TypedBodyQueryProduct {
        outcome: TypedBodyQueryOutcome::Unavailable,
        fingerprint: current.fingerprint,
    })
}

/// Demands the source-neutral typed result for one stable body identity.
///
/// # Errors
///
/// Returns only computation-kernel failures. Authored body rejection remains unavailable until
/// typed recovery becomes a first-class query outcome.
pub fn typed_body(
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
pub fn checked_bodies(
    database: &Database,
    scope: &SemanticScopeKey,
) -> Result<Option<CheckedBodySet>, ComputationError> {
    let declarations = database.query::<DeclarationQuery>(scope.clone())?;
    let DeclarationQueryOutcome::Accepted(declarations) = declarations.outcome() else {
        return Ok(None);
    };
    let mut entries = Vec::with_capacity(declarations.body_identities().len());
    for identity in declarations.body_identities() {
        let product = typed_body(
            database,
            SemanticBodyKey::for_identity(scope.clone(), identity),
        )?;
        let TypedBodyQueryOutcome::Checked(checked) = product.outcome() else {
            return Ok(None);
        };
        entries.push(Arc::clone(checked));
    }
    entries.sort_unstable_by_key(|checked| checked.body());
    Ok(Some(CheckedBodySet {
        entries: entries.into_boxed_slice(),
    }))
}

#[must_use]
pub fn typed_body_execution_count(database: &Database) -> u64 {
    database.execution_count::<TypedBodyQuery>()
}

#[must_use]
pub fn typed_body_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<TypedBodyQuery>()
}

use std::sync::Arc;

use nocter_computation::{
    ComputationError, ComputationKey, Database, Fingerprint, Query, QueryValue,
};

use crate::{
    BodySourceInput, BodySourceKey, CurrentSourceScopeInput, DeclarationQuery,
    DeclarationQueryOutcome, SemanticScopeKey,
};

/// Stable identity of one body query beneath a semantic scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticBodyKey {
    scope: SemanticScopeKey,
    source: BodySourceKey,
    stable: Box<[u8]>,
}

impl SemanticBodyKey {
    #[must_use]
    pub(crate) fn for_identity(
        scope: SemanticScopeKey,
        identity: &nocter_declaration_lowering::ReusableBodyIdentity,
    ) -> Self {
        let source = BodySourceKey::new(identity.canonical_path(), identity.locator());
        let mut stable = scope.stable_bytes().into_vec();
        stable.extend_from_slice(&source.stable_bytes());
        Self {
            scope,
            source,
            stable: stable.into_boxed_slice(),
        }
    }

    pub(crate) const fn scope(&self) -> &SemanticScopeKey {
        &self.scope
    }

    pub(crate) const fn source(&self) -> &BodySourceKey {
        &self.source
    }
}

impl ComputationKey for SemanticBodyKey {
    fn stable_bytes(&self) -> Box<[u8]> {
        self.stable.clone()
    }
}

struct BodyNameQuery;

#[derive(Debug)]
pub enum BodyNameQueryOutcome {
    Resolved(Arc<nocter_checking::ReusableBodyNames>),
    Rejected(Arc<nocter_checking::QueriedBodyNameRejection>),
    Unavailable,
}

#[derive(Debug)]
pub struct BodyNameQueryProduct {
    outcome: BodyNameQueryOutcome,
    fingerprint: Fingerprint,
}

/// Complete, canonical set of source-neutral lexical results for one program.
///
/// Workspace orchestration transports this authority as one opaque semantic product. Only the
/// semantic session consumes its checking-level entries.
#[derive(Debug)]
pub struct BodyNameSet {
    entries: Box<[Arc<nocter_checking::ReusableBodyNames>]>,
    rejections: Box<[Arc<nocter_checking::QueriedBodyNameRejection>]>,
}

impl BodyNameSet {
    #[must_use]
    pub fn entries(&self) -> &[Arc<nocter_checking::ReusableBodyNames>] {
        &self.entries
    }

    #[must_use]
    pub fn rejections(&self) -> &[Arc<nocter_checking::QueriedBodyNameRejection>] {
        &self.rejections
    }
}

impl BodyNameQueryProduct {
    #[must_use]
    pub const fn outcome(&self) -> &BodyNameQueryOutcome {
        &self.outcome
    }
}

impl QueryValue for BodyNameQueryProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for BodyNameQuery {
    type Key = SemanticBodyKey;
    type Value = BodyNameQueryProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let declarations = database.query::<DeclarationQuery>(key.scope.clone())?;
        let declaration_fingerprint = declarations.fingerprint();
        let DeclarationQueryOutcome::Accepted(declarations) = declarations.outcome() else {
            return unavailable(database, key);
        };
        let Some(identity) = declarations.body_identity(&key.source.path, key.source.locator)
        else {
            return unavailable(database, key);
        };
        let body = database.input::<BodySourceInput>(&key.source)?;
        let context =
            database.query::<crate::body_context::BodySemanticContextQuery>(key.scope.clone())?;
        let outcome = context.resolve_names(&body, identity);
        let Some(outcome) = outcome else {
            return unavailable(database, key);
        };
        let outcome = match outcome {
            nocter_checking::ReusableBodyNameQueryOutcome::Resolved(resolved) => {
                BodyNameQueryOutcome::Resolved(Arc::new(resolved))
            }
            nocter_checking::ReusableBodyNameQueryOutcome::Rejected(rejection) => {
                BodyNameQueryOutcome::Rejected(Arc::new(rejection))
            }
        };
        let fingerprint = match &outcome {
            BodyNameQueryOutcome::Resolved(_) => {
                let mut fingerprint = declaration_fingerprint.digest().to_vec();
                fingerprint.extend_from_slice(&body.fingerprint.digest());
                Fingerprint::from_bytes(&fingerprint)
            }
            BodyNameQueryOutcome::Rejected(_) => {
                database
                    .input::<CurrentSourceScopeInput>(&key.scope)?
                    .fingerprint
            }
            BodyNameQueryOutcome::Unavailable => unreachable!("constructed outcome"),
        };
        Ok(BodyNameQueryProduct {
            outcome,
            fingerprint,
        })
    }
}

fn unavailable(
    database: &Database,
    key: &SemanticBodyKey,
) -> Result<BodyNameQueryProduct, ComputationError> {
    let current = database.input::<CurrentSourceScopeInput>(&key.scope)?;
    Ok(BodyNameQueryProduct {
        outcome: BodyNameQueryOutcome::Unavailable,
        fingerprint: current.fingerprint,
    })
}

/// Demands source-neutral lexical resolution for one body.
///
/// # Errors
///
/// Returns only computation-kernel failures. Authored rejection is a first-class exact-current
/// query outcome; unavailable is reserved for an earlier missing authority or internal failure.
pub fn resolve_body_name(
    database: &Database,
    key: SemanticBodyKey,
) -> Result<Arc<BodyNameQueryProduct>, ComputationError> {
    database.query::<BodyNameQuery>(key)
}

/// Demands the canonical body-name authority for every body declared by a program.
///
/// The set is unavailable as a whole when any body cannot publish a reusable lexical result.
/// This keeps fallback selection outside individual body queries while keeping query scheduling
/// out of workspace orchestration.
///
/// # Errors
///
/// Returns computation-kernel failures from an individual body demand.
pub fn resolved_body_names(
    database: &Database,
    scope: &SemanticScopeKey,
) -> Result<Option<BodyNameSet>, ComputationError> {
    let declarations = database.query::<DeclarationQuery>(scope.clone())?;
    let DeclarationQueryOutcome::Accepted(declarations) = declarations.outcome() else {
        return Ok(None);
    };
    let mut entries = Vec::with_capacity(declarations.body_identities().len());
    let mut rejections = Vec::new();
    for identity in declarations.body_identities() {
        let product = resolve_body_name(
            database,
            SemanticBodyKey::for_identity(scope.clone(), identity),
        )?;
        match product.outcome() {
            BodyNameQueryOutcome::Resolved(names) => entries.push(Arc::clone(names)),
            BodyNameQueryOutcome::Rejected(rejection) => {
                rejections.push(Arc::clone(rejection));
            }
            BodyNameQueryOutcome::Unavailable => return Ok(None),
        }
    }
    entries.sort_unstable_by_key(|names| names.body());
    rejections.sort_unstable_by_key(|rejection| rejection.body());
    Ok(Some(BodyNameSet {
        entries: entries.into_boxed_slice(),
        rejections: rejections.into_boxed_slice(),
    }))
}

#[must_use]
pub fn body_name_execution_count(database: &Database) -> u64 {
    database.execution_count::<BodyNameQuery>()
}

#[must_use]
pub fn body_name_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<BodyNameQuery>()
}

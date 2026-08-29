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
    pub fn for_identity(
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
pub struct ResolvedBodyNameSet {
    entries: Box<[Arc<nocter_checking::ReusableBodyNames>]>,
}

impl ResolvedBodyNameSet {
    #[must_use]
    pub fn entries(&self) -> &[Arc<nocter_checking::ReusableBodyNames>] {
        &self.entries
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
        let resolved = context.resolve_names(&body, identity);
        let Some(resolved) = resolved else {
            return unavailable(database, key);
        };
        let mut fingerprint = declaration_fingerprint.digest().to_vec();
        fingerprint.extend_from_slice(&body.fingerprint.digest());
        Ok(BodyNameQueryProduct {
            outcome: BodyNameQueryOutcome::Resolved(Arc::new(resolved)),
            fingerprint: Fingerprint::from_bytes(&fingerprint),
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
/// Returns only computation-kernel failures. Current authored rejection remains an unavailable
/// outcome until query-owned recovery migration is complete.
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
) -> Result<Option<ResolvedBodyNameSet>, ComputationError> {
    let declarations = database.query::<DeclarationQuery>(scope.clone())?;
    let DeclarationQueryOutcome::Accepted(declarations) = declarations.outcome() else {
        return Ok(None);
    };
    let mut entries = Vec::with_capacity(declarations.body_identities().len());
    for identity in declarations.body_identities() {
        let product = resolve_body_name(
            database,
            SemanticBodyKey::for_identity(scope.clone(), identity),
        )?;
        let BodyNameQueryOutcome::Resolved(names) = product.outcome() else {
            return Ok(None);
        };
        entries.push(Arc::clone(names));
    }
    entries.sort_unstable_by_key(|names| names.body());
    Ok(Some(ResolvedBodyNameSet {
        entries: entries.into_boxed_slice(),
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

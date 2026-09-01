//! Source-neutral lexical body queries.

use std::sync::Arc;

use nocter_computation::{
    ComputationError, ComputationKey, Database, Fingerprint, Query, QueryValue,
};

use super::{
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
    pub(super) fn for_identity(
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

    pub(super) const fn scope(&self) -> &SemanticScopeKey {
        &self.scope
    }

    pub(super) const fn source(&self) -> &BodySourceKey {
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
    Failed(Arc<super::SemanticQueryFailure>),
}

#[derive(Debug)]
pub struct BodyNameQueryProduct {
    source: BodySourceKey,
    outcome: BodyNameQueryOutcome,
    fingerprint: Fingerprint,
}

pub(super) enum ExactBodyNamesBinding<'a> {
    Ready(super::ExactBodyNamesInput<'a>),
    Rejected,
    Failed(&'a Arc<super::SemanticQueryFailure>),
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

    pub(super) fn bind_exact_body<'a>(
        &'a self,
        source: &'a super::BodySourceValue,
    ) -> Result<ExactBodyNamesBinding<'a>, super::SemanticQueryFailure> {
        if self.source != source.key {
            return Err(super::SemanticQueryFailure::BodySourceIdentityMismatch {
                demanded_path: source.key.path.clone(),
                demanded_locator: source.key.locator,
                semantic_path: self.source.path.clone(),
                semantic_locator: self.source.locator,
            });
        }
        Ok(match &self.outcome {
            BodyNameQueryOutcome::Resolved(names) => {
                ExactBodyNamesBinding::Ready(super::ExactBodyNamesInput::new(source, names))
            }
            BodyNameQueryOutcome::Rejected(_) => ExactBodyNamesBinding::Rejected,
            BodyNameQueryOutcome::Failed(failure) => ExactBodyNamesBinding::Failed(failure),
        })
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
            return failed(
                database,
                key,
                match declarations.outcome() {
                    DeclarationQueryOutcome::Failed(failure) => Arc::clone(failure),
                    DeclarationQueryOutcome::Rejected(_) => {
                        Arc::new(super::SemanticQueryFailure::InvalidStageTransition(
                            "body-name query demanded after declaration rejection",
                        ))
                    }
                    DeclarationQueryOutcome::Accepted(_) => {
                        Arc::new(super::SemanticQueryFailure::InvalidStageTransition(
                            "accepted declaration branch was not selected",
                        ))
                    }
                },
            );
        };
        let Some(identity) = declarations.body_identity(&key.source.path, key.source.locator)
        else {
            return failed(
                database,
                key,
                Arc::new(super::SemanticQueryFailure::MissingBodyIdentity {
                    path: key.source.path.clone(),
                    locator: key.source.locator,
                }),
            );
        };
        let body = database.input::<BodySourceInput>(&key.source)?;
        let exact_body = match body.bind_identity(identity) {
            Ok(exact_body) => exact_body,
            Err(failure) => return failed(database, key, Arc::new(failure)),
        };
        let context =
            database.query::<super::body_context::BodySemanticContextQuery>(key.scope.clone())?;
        let outcome = match context.resolve_names(exact_body) {
            Ok(outcome) => outcome,
            Err(failure) => return failed(database, key, failure),
        };
        let (outcome, fingerprint) = match outcome {
            nocter_checking::ReusableBodyNameQueryOutcome::Resolved(resolved) => {
                let mut fingerprint = declaration_fingerprint.digest().to_vec();
                fingerprint.extend_from_slice(&exact_body.fingerprint().digest());
                (
                    BodyNameQueryOutcome::Resolved(Arc::new(resolved)),
                    Fingerprint::from_bytes(&fingerprint),
                )
            }
            nocter_checking::ReusableBodyNameQueryOutcome::Rejected(rejection) => (
                BodyNameQueryOutcome::Rejected(Arc::new(rejection)),
                database
                    .input::<CurrentSourceScopeInput>(&key.scope)?
                    .fingerprint,
            ),
        };
        Ok(BodyNameQueryProduct {
            source: key.source.clone(),
            outcome,
            fingerprint,
        })
    }
}

fn failed(
    database: &Database,
    key: &SemanticBodyKey,
    failure: Arc<super::SemanticQueryFailure>,
) -> Result<BodyNameQueryProduct, ComputationError> {
    let current = database.input::<CurrentSourceScopeInput>(&key.scope)?;
    Ok(BodyNameQueryProduct {
        source: key.source.clone(),
        outcome: BodyNameQueryOutcome::Failed(failure),
        fingerprint: current.fingerprint,
    })
}

/// Demands source-neutral lexical resolution for one body.
///
/// # Errors
///
/// Returns only computation-kernel failures. Authored rejection is a first-class exact-current
/// query outcome; an earlier rejection and an integrity failure remain distinct products.
pub(super) fn resolve_body_name(
    database: &Database,
    key: SemanticBodyKey,
) -> Result<Arc<BodyNameQueryProduct>, ComputationError> {
    database.query::<BodyNameQuery>(key)
}

/// Demands the canonical body-name authority for every body declared by a program.
///
/// An authored body rejection remains in the complete set. An integrity failure aborts set
/// assembly with its typed cause. This keeps branch selection outside individual body queries
/// while keeping query scheduling out of workspace orchestration.
///
/// # Errors
///
/// Returns computation-kernel failures from an individual body demand.
pub(super) fn resolved_body_names(
    database: &Database,
    scope: &SemanticScopeKey,
) -> Result<super::SemanticStage<BodyNameSet>, ComputationError> {
    let declarations = database.query::<DeclarationQuery>(scope.clone())?;
    let DeclarationQueryOutcome::Accepted(declarations) = declarations.outcome() else {
        let failure = match declarations.outcome() {
            DeclarationQueryOutcome::Failed(failure) => Arc::clone(failure),
            DeclarationQueryOutcome::Rejected(_) => {
                Arc::new(super::SemanticQueryFailure::InvalidStageTransition(
                    "body-name set demanded after declaration rejection",
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
        let product = resolve_body_name(
            database,
            SemanticBodyKey::for_identity(scope.clone(), identity),
        )?;
        match product.outcome() {
            BodyNameQueryOutcome::Resolved(names) => entries.push(Arc::clone(names)),
            BodyNameQueryOutcome::Rejected(rejection) => {
                rejections.push(Arc::clone(rejection));
            }
            BodyNameQueryOutcome::Failed(failure) => {
                return Ok(Err(Arc::clone(failure)));
            }
        }
    }
    entries.sort_unstable_by_key(|names| names.body());
    rejections.sort_unstable_by_key(|rejection| rejection.body());
    Ok(Ok(BodyNameSet {
        entries: entries.into_boxed_slice(),
        rejections: rejections.into_boxed_slice(),
    }))
}

#[must_use]
pub(super) fn body_name_execution_count(database: &Database) -> u64 {
    database.execution_count::<BodyNameQuery>()
}

#[must_use]
pub(super) fn body_name_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<BodyNameQuery>()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nocter_computation::{Fingerprint, QueryValue};
    use nocter_syntax::DeclarationSyntaxLocator;

    use super::{BodyNameQueryOutcome, BodyNameQueryProduct};
    use crate::semantic::{BodySourceKey, BodySourceValue, SemanticQueryFailure};

    #[test]
    fn exact_body_binding_rejects_a_different_source_identity() {
        let product_key = BodySourceKey::new("/app/first.nct", DeclarationSyntaxLocator::Node(1));
        let source_key = BodySourceKey::new("/app/second.nct", DeclarationSyntaxLocator::Node(1));
        let source = BodySourceValue::for_test(source_key, Fingerprint::from_bytes(b"body"));
        let product = BodyNameQueryProduct {
            source: product_key,
            outcome: BodyNameQueryOutcome::Failed(Arc::new(
                SemanticQueryFailure::InvalidStageTransition("test rejection"),
            )),
            fingerprint: source.fingerprint(),
        };

        assert!(matches!(
            product.bind_exact_body(&source),
            Err(SemanticQueryFailure::BodySourceIdentityMismatch { .. })
        ));
    }
}

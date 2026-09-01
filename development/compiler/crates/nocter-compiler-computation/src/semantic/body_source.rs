//! Exact physical source inputs for reusable semantic body queries.

use nocter_computation::{
    ComputationKey, Fingerprint, Input, InputRetention, InputRevision, QueryValue,
};

use super::{SemanticQueryFailure, encode};

pub(super) struct BodySourceInput;

impl Input for BodySourceInput {
    type Key = BodySourceKey;
    type Value = BodySourceValue;

    const RETENTION: InputRetention = InputRetention::RevisionDerived;
}

/// Stable physical identity of one executable body beneath a declaration surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BodySourceKey {
    pub(super) path: Box<str>,
    pub(super) locator: nocter_syntax::DeclarationSyntaxLocator,
    stable: Box<[u8]>,
}

impl BodySourceKey {
    pub(super) fn new(path: &str, locator: nocter_syntax::DeclarationSyntaxLocator) -> Self {
        let mut stable = Vec::new();
        encode(path.as_bytes(), &mut stable);
        match locator {
            nocter_syntax::DeclarationSyntaxLocator::Node(index) => {
                stable.push(0);
                stable.extend_from_slice(&index.to_be_bytes());
            }
            nocter_syntax::DeclarationSyntaxLocator::Token(index) => {
                stable.push(1);
                stable.extend_from_slice(&index.to_be_bytes());
            }
        }
        Self {
            path: path.into(),
            locator,
            stable: stable.into_boxed_slice(),
        }
    }
}

impl ComputationKey for BodySourceKey {
    fn stable_bytes(&self) -> Box<[u8]> {
        self.stable.clone()
    }
}

pub(super) struct BodySourceValue {
    pub(super) key: BodySourceKey,
    fingerprint: Fingerprint,
}

#[derive(Clone, Copy)]
pub(super) struct ExactBodyIdentityInput<'a> {
    source: &'a BodySourceValue,
    identity: &'a nocter_declaration_lowering::ReusableBodyIdentity,
}

impl ExactBodyIdentityInput<'_> {
    #[must_use]
    pub(super) const fn body(&self) -> nocter_model::BodyId {
        self.identity.body()
    }

    #[must_use]
    pub(super) const fn fingerprint(&self) -> Fingerprint {
        self.source.fingerprint
    }
}

#[derive(Clone, Copy)]
pub(super) struct ExactBodyNamesInput<'a> {
    source: &'a BodySourceValue,
    names: &'a nocter_checking::ReusableBodyNames,
}

impl<'a> ExactBodyNamesInput<'a> {
    pub(super) const fn new(
        source: &'a BodySourceValue,
        names: &'a nocter_checking::ReusableBodyNames,
    ) -> Self {
        Self { source, names }
    }

    #[must_use]
    pub(super) const fn names(&self) -> &nocter_checking::ReusableBodyNames {
        self.names
    }

    #[must_use]
    pub(super) const fn fingerprint(&self) -> Fingerprint {
        self.source.fingerprint
    }
}

impl BodySourceValue {
    pub(super) fn bind_identity<'a>(
        &'a self,
        identity: &'a nocter_declaration_lowering::ReusableBodyIdentity,
    ) -> Result<ExactBodyIdentityInput<'a>, SemanticQueryFailure> {
        let expected = BodySourceKey::new(identity.canonical_path(), identity.locator());
        if self.key != expected {
            return Err(SemanticQueryFailure::BodySourceIdentityMismatch {
                demanded_path: self.key.path.clone(),
                demanded_locator: self.key.locator,
                semantic_path: expected.path,
                semantic_locator: expected.locator,
            });
        }
        Ok(ExactBodyIdentityInput {
            source: self,
            identity,
        })
    }
}

impl QueryValue for BodySourceValue {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

/// One exact per-body input staged with its containing semantic scope revision.
pub(crate) struct BodySourcePublication {
    key: BodySourceKey,
    value: BodySourceValue,
}

impl BodySourcePublication {
    #[must_use]
    pub(crate) fn new(path: &str, body: &nocter_syntax::BodySyntaxSurface) -> Self {
        let key = BodySourceKey::new(path, body.locator());
        Self {
            key: key.clone(),
            value: BodySourceValue {
                key,
                fingerprint: Fingerprint::from_bytes(body.canonical_bytes()),
            },
        }
    }

    pub(super) fn publish(self, revision: &mut InputRevision<'_>) {
        revision.set::<BodySourceInput>(&self.key, self.value);
    }
}

#[cfg(test)]
impl BodySourceValue {
    pub(super) fn for_test(key: BodySourceKey, fingerprint: Fingerprint) -> Self {
        Self { key, fingerprint }
    }
}

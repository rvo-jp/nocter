use std::collections::BTreeSet;
use std::fmt;

use nocter_checking::{CaptureMode, LocalBindingKind, NameTarget};
use nocter_model::{BodyId, BodyNodeId, BodyScopeId, CaptureId, LocalBindingId, TypeId};
use nocter_source_index::{SemanticEntity, SourceIndex};

use super::{SemanticEvidence, SemanticQueryContext};

/// The completeness of one protocol-independent semantic set query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticCoverage {
    Complete,
    Partial(Box<[SemanticBodyGap]>),
    Unavailable(SemanticSetUnavailability),
}

/// The reason no semantic domain can answer one set query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticSetUnavailability {
    NoSemanticEvidence,
}

impl SemanticCoverage {
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// One body domain that could not contribute facts required by a semantic set query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticBodyGap {
    body: BodyId,
    reason: TypedBodyUnavailability,
}

impl SemanticBodyGap {
    #[must_use]
    pub const fn body(self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn reason(self) -> TypedBodyUnavailability {
        self.reason
    }
}

/// Values returned by one semantic query together with proof of their coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticQuerySet<T> {
    values: Box<[T]>,
    coverage: SemanticCoverage,
}

impl<T> SemanticQuerySet<T> {
    pub(crate) const fn new(values: Box<[T]>, coverage: SemanticCoverage) -> Self {
        Self { values, coverage }
    }

    #[must_use]
    pub const fn values(&self) -> &[T] {
        &self.values
    }

    #[must_use]
    pub const fn coverage(&self) -> &SemanticCoverage {
        &self.coverage
    }

    #[must_use]
    pub fn into_values(self) -> Box<[T]> {
        self.values
    }
}

impl<T> std::ops::Deref for SemanticQuerySet<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl<'a, T> IntoIterator for &'a SemanticQuerySet<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}

impl<T> IntoIterator for SemanticQuerySet<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_vec().into_iter()
    }
}

/// The typed-body capability available to protocol-independent semantic queries.
#[derive(Debug)]
pub(crate) enum TypedBodyEvidence<'a> {
    Available(&'a nocter_checking::CheckedBody),
    Unavailable(TypedBodyUnavailability),
}

/// One query fact that is either proven by typed evidence or unavailable for an authored reason.
pub(crate) enum SemanticFact<T, U = TypedBodyUnavailability> {
    Available(T),
    Unavailable(U),
}

impl<T, U> SemanticFact<T, U> {
    pub(crate) fn into_result(self) -> Result<T, U> {
        match self {
            Self::Available(value) => Ok(value),
            Self::Unavailable(reason) => Err(reason),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScopeUnavailability {
    NamesRejected,
    NameResolutionNotReached,
}

/// The body-local facts editor queries may consume without inspecting checked-body storage.
#[derive(Clone, Copy)]
pub(crate) struct LocalBindingFact {
    ty: TypeId,
    readonly: bool,
}

impl LocalBindingFact {
    pub(crate) const fn ty(self) -> TypeId {
        self.ty
    }

    pub(crate) const fn readonly(self) -> bool {
        self.readonly
    }
}

/// Proof that every source-semantic occurrence required by a mutation is available.
#[derive(Clone, Copy)]
pub(crate) struct CompleteSemanticQuery<'a> {
    checked: &'a nocter_checking::CheckedProgram,
    source_index: &'a SourceIndex,
}

impl<'a> CompleteSemanticQuery<'a> {
    pub(crate) const fn checked(self) -> &'a nocter_checking::CheckedProgram {
        self.checked
    }

    pub(crate) const fn source_index(self) -> &'a SourceIndex {
        self.source_index
    }

    pub(crate) fn checked_operation(
        self,
        body: BodyId,
        node: BodyNodeId,
    ) -> Result<&'a nocter_checking::CheckedOperation, EvidenceIntegrityError> {
        if self
            .checked
            .graph()
            .declarations()
            .bodies()
            .get(body)
            .is_none()
        {
            return Err(EvidenceIntegrityError::MissingBodyDomain(body));
        }
        let checked_body = self
            .checked
            .bodies()
            .get(body)
            .ok_or(EvidenceIntegrityError::MissingBodyDomain(body))?;
        checked_body
            .nodes()
            .get(node)
            .map(nocter_checking::CheckedNode::operation)
            .ok_or(EvidenceIntegrityError::MissingBodyNode { body, node })
    }

    pub(crate) fn rename_family(self, selected: SemanticEntity) -> BTreeSet<SemanticEntity> {
        let mut entities = BTreeSet::from([selected]);
        let mut changed = true;
        while changed {
            changed = false;
            for (body_id, body) in self.checked.bodies().iter() {
                for (capture_id, capture) in body.captures().iter() {
                    let capture_entity = SemanticEntity::Capture(body_id, capture_id);
                    let source_entity = match capture.declaration().source() {
                        NameTarget::Parameter(parameter) => SemanticEntity::Parameter(parameter),
                        NameTarget::Local(local) => SemanticEntity::LocalBinding(body_id, local),
                        NameTarget::Capture(capture) => SemanticEntity::Capture(body_id, capture),
                        NameTarget::Exported(_) => continue,
                    };
                    if entities.contains(&capture_entity) || entities.contains(&source_entity) {
                        changed |= entities.insert(capture_entity);
                        changed |= entities.insert(source_entity);
                    }
                }
            }
        }
        entities
    }
}

/// An expected source-semantic reason why typed-body facts are unavailable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedBodyUnavailability {
    BodyRejected,
    NamesRejected,
    TypingNotReached,
}

/// An impossible mismatch between one semantic identity and its owning analysis evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceIntegrityError {
    MissingBodyDomain(BodyId),
    MissingBodyNode { body: BodyId, node: BodyNodeId },
    MissingLocalBinding { body: BodyId, local: LocalBindingId },
    MissingCapture { body: BodyId, capture: CaptureId },
    MissingBodyScope { body: BodyId, scope: BodyScopeId },
}

impl fmt::Display for EvidenceIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBodyDomain(body) => {
                write!(
                    formatter,
                    "analysis evidence has no body domain for {body:?}"
                )
            }
            Self::MissingBodyNode { body, node } => {
                write!(
                    formatter,
                    "analysis evidence has no body node {body:?}/{node:?}"
                )
            }
            Self::MissingLocalBinding { body, local } => write!(
                formatter,
                "analysis evidence has no local binding {body:?}/{local:?}"
            ),
            Self::MissingCapture { body, capture } => write!(
                formatter,
                "analysis evidence has no capture {body:?}/{capture:?}"
            ),
            Self::MissingBodyScope { body, scope } => write!(
                formatter,
                "analysis evidence has no body scope {body:?}/{scope:?}"
            ),
        }
    }
}

impl std::error::Error for EvidenceIntegrityError {}

impl<'a> SemanticQueryContext<'a> {
    pub(crate) const fn complete(self) -> Option<CompleteSemanticQuery<'a>> {
        match self.evidence {
            SemanticEvidence::Checked {
                checked,
                source_index,
            } => Some(CompleteSemanticQuery {
                checked,
                source_index,
            }),
            SemanticEvidence::Bodies(_)
            | SemanticEvidence::Names(_)
            | SemanticEvidence::Declarations(_) => None,
        }
    }

    /// Resolves one body identity through the explicit evidence owned by the current generation.
    ///
    /// Expected rejection and an unreached typing phase are ordinary unavailable outcomes. Only a
    /// body identity absent from its owning semantic domain is an integrity failure.
    pub(crate) fn typed_body_evidence(
        &self,
        body: BodyId,
    ) -> Result<TypedBodyEvidence<'a>, EvidenceIntegrityError> {
        if self.graph().declarations().bodies().get(body).is_none() {
            return Err(EvidenceIntegrityError::MissingBodyDomain(body));
        }
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => checked
                .bodies()
                .get(body)
                .map(TypedBodyEvidence::Available)
                .ok_or(EvidenceIntegrityError::MissingBodyDomain(body)),
            SemanticEvidence::Bodies(analysis) => match analysis
                .body_evidence(body)
                .ok_or(EvidenceIntegrityError::MissingBodyDomain(body))?
            {
                nocter_checking::BodyEvidence::Typed(body) => {
                    Ok(TypedBodyEvidence::Available(body))
                }
                nocter_checking::BodyEvidence::Rejected(_) => Ok(TypedBodyEvidence::Unavailable(
                    TypedBodyUnavailability::BodyRejected,
                )),
            },
            SemanticEvidence::Names(analysis) => match analysis
                .body_names()
                .evidence(body)
                .ok_or(EvidenceIntegrityError::MissingBodyDomain(body))?
            {
                nocter_checking::BodyNameEvidence::Resolved(_) => Ok(
                    TypedBodyEvidence::Unavailable(TypedBodyUnavailability::TypingNotReached),
                ),
                nocter_checking::BodyNameEvidence::Rejected(_) => Ok(
                    TypedBodyEvidence::Unavailable(TypedBodyUnavailability::NamesRejected),
                ),
            },
            SemanticEvidence::Declarations(_) => Ok(TypedBodyEvidence::Unavailable(
                TypedBodyUnavailability::TypingNotReached,
            )),
        }
    }

    /// Proves which declared body domains can contribute typed semantic occurrences.
    pub(crate) fn typed_body_coverage(&self) -> Result<SemanticCoverage, EvidenceIntegrityError> {
        let mut gaps = Vec::new();
        for (body, _) in self.graph().declarations().bodies().iter() {
            if let TypedBodyEvidence::Unavailable(reason) = self.typed_body_evidence(body)? {
                gaps.push(SemanticBodyGap { body, reason });
            }
        }
        if gaps.is_empty() {
            Ok(SemanticCoverage::Complete)
        } else {
            Ok(SemanticCoverage::Partial(gaps.into_boxed_slice()))
        }
    }

    pub(crate) fn local_binding_fact(
        &self,
        body: BodyId,
        local: LocalBindingId,
    ) -> Result<SemanticFact<LocalBindingFact>, EvidenceIntegrityError> {
        let typed = match self.typed_body_evidence(body)? {
            TypedBodyEvidence::Available(body) => body,
            TypedBodyEvidence::Unavailable(reason) => {
                return Ok(SemanticFact::Unavailable(reason));
            }
        };
        let local = typed
            .locals()
            .get(local)
            .ok_or(EvidenceIntegrityError::MissingLocalBinding { body, local })?;
        Ok(SemanticFact::Available(LocalBindingFact {
            ty: local.ty(),
            readonly: local.declaration().kind() != LocalBindingKind::Mutable,
        }))
    }

    pub(crate) fn capture_readonly_fact(
        &self,
        body: BodyId,
        capture: CaptureId,
    ) -> Result<SemanticFact<bool>, EvidenceIntegrityError> {
        let typed = match self.typed_body_evidence(body)? {
            TypedBodyEvidence::Available(body) => body,
            TypedBodyEvidence::Unavailable(reason) => {
                return Ok(SemanticFact::Unavailable(reason));
            }
        };
        let capture = typed
            .captures()
            .get(capture)
            .ok_or(EvidenceIntegrityError::MissingCapture { body, capture })?;
        Ok(SemanticFact::Available(
            capture.declaration().mode() == CaptureMode::Readonly,
        ))
    }

    pub(crate) fn checked_operation(
        &self,
        body: BodyId,
        node: BodyNodeId,
    ) -> Result<SemanticFact<&'a nocter_checking::CheckedOperation>, EvidenceIntegrityError> {
        let typed = match self.typed_body_evidence(body)? {
            TypedBodyEvidence::Available(body) => body,
            TypedBodyEvidence::Unavailable(reason) => {
                return Ok(SemanticFact::Unavailable(reason));
            }
        };
        let node = typed
            .nodes()
            .get(node)
            .ok_or(EvidenceIntegrityError::MissingBodyNode { body, node })?;
        Ok(SemanticFact::Available(node.operation()))
    }

    pub(crate) fn body_scope_fact(
        &self,
        body: BodyId,
        scope: BodyScopeId,
    ) -> Result<
        SemanticFact<&'a nocter_checking::BodyScope, ScopeUnavailability>,
        EvidenceIntegrityError,
    > {
        if self.graph().declarations().bodies().get(body).is_none() {
            return Err(EvidenceIntegrityError::MissingBodyDomain(body));
        }
        let names = match self.evidence {
            SemanticEvidence::Checked { checked, .. } => {
                let checked_body = checked
                    .bodies()
                    .get(body)
                    .ok_or(EvidenceIntegrityError::MissingBodyDomain(body))?;
                return checked_body
                    .scopes()
                    .get(scope)
                    .map(SemanticFact::Available)
                    .ok_or(EvidenceIntegrityError::MissingBodyScope { body, scope });
            }
            SemanticEvidence::Bodies(analysis) => {
                let names = analysis
                    .body_names()
                    .get(body)
                    .ok_or(EvidenceIntegrityError::MissingBodyDomain(body))?;
                return names
                    .scopes()
                    .get(scope)
                    .map(SemanticFact::Available)
                    .ok_or(EvidenceIntegrityError::MissingBodyScope { body, scope });
            }
            SemanticEvidence::Names(analysis) => analysis
                .body_names()
                .evidence(body)
                .ok_or(EvidenceIntegrityError::MissingBodyDomain(body))?,
            SemanticEvidence::Declarations(_) => {
                return Ok(SemanticFact::Unavailable(
                    ScopeUnavailability::NameResolutionNotReached,
                ));
            }
        };
        let Some(names) = names.usable_names() else {
            return Ok(SemanticFact::Unavailable(
                ScopeUnavailability::NamesRejected,
            ));
        };
        names
            .scopes()
            .get(scope)
            .map(SemanticFact::Available)
            .ok_or(EvidenceIntegrityError::MissingBodyScope { body, scope })
    }
}

use std::collections::BTreeSet;
use std::fmt;

use nocter_checking::{CaptureMode, LocalBindingKind, NameTarget};
use nocter_model::{BodyId, BodyNodeId, BodyScopeId, CaptureId, LocalBindingId, TypeId};
use nocter_source::{SourceId, SourceMap};
use nocter_source_index::{SemanticEntity, SourceIndex};
use nocter_syntax::{SyntaxOrigin, SyntaxTree};

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
    pub(in crate::query) const fn new(values: Box<[T]>, coverage: SemanticCoverage) -> Self {
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
pub(in crate::query) enum TypedBodyEvidence<'a> {
    Available(&'a nocter_checking::CheckedBody),
    Unavailable(TypedBodyUnavailability),
}

/// One query fact that is either proven by typed evidence or unavailable for an authored reason.
pub(in crate::query) enum SemanticFact<T, U = TypedBodyUnavailability> {
    Available(T),
    Unavailable(U),
}

impl<T, U> SemanticFact<T, U> {
    pub(in crate::query) fn into_result(self) -> Result<T, U> {
        match self {
            Self::Available(value) => Ok(value),
            Self::Unavailable(reason) => Err(reason),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::query) enum ScopeUnavailability {
    NamesRejected,
    NameResolutionNotReached,
}

/// The body-local facts editor queries may consume without inspecting checked-body storage.
#[derive(Clone, Copy)]
pub(in crate::query) struct LocalBindingFact {
    ty: TypeId,
    readonly: bool,
}

impl LocalBindingFact {
    pub(in crate::query) const fn ty(self) -> TypeId {
        self.ty
    }

    pub(in crate::query) const fn readonly(self) -> bool {
        self.readonly
    }
}

/// Proof that every source-semantic occurrence required by a mutation is available.
#[derive(Clone, Copy)]
pub(super) struct CompleteSemanticQuery<'a> {
    checked: &'a nocter_checking::CheckedProgram,
    source_index: &'a SourceIndex,
}

impl<'a> CompleteSemanticQuery<'a> {
    pub(super) const fn checked(self) -> &'a nocter_checking::CheckedProgram {
        self.checked
    }

    pub(super) const fn source_index(self) -> &'a SourceIndex {
        self.source_index
    }

    pub(in crate::query) fn checked_operation(
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

    pub(in crate::query) fn rename_family(
        self,
        selected: SemanticEntity,
    ) -> BTreeSet<SemanticEntity> {
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
    MissingSemanticEntity(SemanticEntity),
    MissingIndexedSource(SourceId),
    MissingSourceOwner(SourceId),
    InvalidSourceOwner {
        source: SourceId,
        module: nocter_model::ModuleId,
    },
    MissingSourceSyntax(SourceId),
    InvalidSourceOrigin {
        source: SourceId,
        syntax: SyntaxOrigin,
    },
    MissingBodyDomain(BodyId),
    MissingBodyNode {
        body: BodyId,
        node: BodyNodeId,
    },
    MissingLocalBinding {
        body: BodyId,
        local: LocalBindingId,
    },
    MissingCapture {
        body: BodyId,
        capture: CaptureId,
    },
    MissingBodyScope {
        body: BodyId,
        scope: BodyScopeId,
    },
}

impl fmt::Display for EvidenceIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSemanticEntity(entity) => {
                write!(formatter, "analysis evidence has no domain for {entity:?}")
            }
            Self::MissingIndexedSource(source) => {
                write!(formatter, "source projection references absent {source}")
            }
            Self::MissingSourceOwner(source) => {
                write!(
                    formatter,
                    "analysis evidence has no semantic owner for {source}"
                )
            }
            Self::InvalidSourceOwner { source, module } => write!(
                formatter,
                "analysis evidence assigns {source} to absent module {module:?}"
            ),
            Self::MissingSourceSyntax(source) => {
                write!(
                    formatter,
                    "analysis evidence has no syntax tree for {source}"
                )
            }
            Self::InvalidSourceOrigin { source, syntax } => write!(
                formatter,
                "source projection references absent syntax {syntax:?} in {source}"
            ),
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
    pub(in crate::query) fn validate_generation(
        &self,
        sources: &SourceMap,
        syntax_trees: &[SyntaxTree],
    ) -> Result<(), EvidenceIntegrityError> {
        for source in sources.iter() {
            let source = source.id();
            let module = self
                .source_ownership()
                .module_for_source(source)
                .map_err(|_| EvidenceIntegrityError::MissingSourceOwner(source))?;
            if self.graph().modules().get(module).is_none() {
                return Err(EvidenceIntegrityError::InvalidSourceOwner { source, module });
            }
        }
        for source in self.source_index().source_ids().collect::<BTreeSet<_>>() {
            if sources.get(source).is_none() {
                return Err(EvidenceIntegrityError::MissingIndexedSource(source));
            }
        }
        for origin in self.source_index().origins() {
            let source = origin.source();
            let tree = syntax_trees
                .iter()
                .find(|tree| tree.source() == source)
                .ok_or(EvidenceIntegrityError::MissingSourceSyntax(source))?;
            let valid = match origin.syntax() {
                SyntaxOrigin::Node(node) => {
                    nocter_source_index::SourceOrigin::from_node(tree, node) == Ok(origin)
                }
                SyntaxOrigin::Token(token) => {
                    nocter_source_index::SourceOrigin::from_token(tree, token) == Ok(origin)
                }
            };
            if !valid {
                return Err(EvidenceIntegrityError::InvalidSourceOrigin {
                    source,
                    syntax: origin.syntax(),
                });
            }
        }
        let entities = self
            .source_index()
            .semantic_entities()
            .collect::<BTreeSet<_>>();
        for entity in entities {
            self.validate_entity_domain(entity)?;
        }
        Ok(())
    }

    fn validate_entity_domain(&self, entity: SemanticEntity) -> Result<(), EvidenceIntegrityError> {
        let graph = self.graph();
        let declarations = graph.declarations();
        let present = match entity {
            SemanticEntity::Package(id) => graph.packages().get(id).is_some(),
            SemanticEntity::PackageTarget(id) => graph.package_targets().get(id).is_some(),
            SemanticEntity::Module(id) => graph.modules().get(id).is_some(),
            SemanticEntity::BuiltinType(_) => true,
            SemanticEntity::Import(id) => graph.imports().get(id).is_some(),
            SemanticEntity::DeclarationSite(id) => graph.declaration_sites().get(id).is_some(),
            SemanticEntity::NominalType(id) => declarations.nominal_types().get(id).is_some(),
            SemanticEntity::TypeAlias(id) => declarations.type_aliases().get(id).is_some(),
            SemanticEntity::Interface(id) => declarations.interfaces().get(id).is_some(),
            SemanticEntity::AssociatedType(id) => declarations.associated_types().get(id).is_some(),
            SemanticEntity::Constant(id) => declarations.constants().get(id).is_some(),
            SemanticEntity::Callable(id) => declarations.callables().get(id).is_some(),
            SemanticEntity::Construction(id) => declarations.constructions().get(id).is_some(),
            SemanticEntity::Instance(id) => declarations.instances().get(id).is_some(),
            SemanticEntity::InterfaceImplementation(id) => {
                declarations.interface_implementations().get(id).is_some()
            }
            SemanticEntity::Drop(id) => declarations.drops().get(id).is_some(),
            SemanticEntity::Test(id) => declarations.tests().get(id).is_some(),
            SemanticEntity::Field(id) => declarations.fields().get(id).is_some(),
            SemanticEntity::Variant(id) => declarations.variants().get(id).is_some(),
            SemanticEntity::GenericParameter(id) => {
                declarations.generic_parameters().get(id).is_some()
            }
            SemanticEntity::Parameter(id) => declarations.parameters().get(id).is_some(),
            SemanticEntity::Requirement(id) => declarations.requirements().get(id).is_some(),
            SemanticEntity::Body(id) => declarations.bodies().get(id).is_some(),
            SemanticEntity::BodyScope(body, scope) => self.scope_exists(body, scope),
            SemanticEntity::BodyNode(body, node) => self.node_exists(body, node),
            SemanticEntity::LocalBinding(body, local) => self.local_exists(body, local),
            SemanticEntity::Capture(body, capture) => self.capture_exists(body, capture),
            SemanticEntity::OpaqueType(id) => declarations.opaque_types().get(id).is_some(),
        };
        if present {
            Ok(())
        } else {
            Err(EvidenceIntegrityError::MissingSemanticEntity(entity))
        }
    }

    fn scope_exists(&self, body: BodyId, scope: BodyScopeId) -> bool {
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => checked
                .bodies()
                .get(body)
                .is_some_and(|body| body.scopes().get(scope).is_some()),
            SemanticEvidence::Bodies(analysis) => analysis
                .body_names()
                .get(body)
                .is_some_and(|names| names.scopes().get(scope).is_some()),
            SemanticEvidence::Names(analysis) => analysis
                .body_names()
                .evidence(body)
                .and_then(nocter_checking::BodyNameEvidence::usable_names)
                .is_some_and(|names| names.scopes().get(scope).is_some()),
            SemanticEvidence::Declarations(_) => false,
        }
    }

    fn node_exists(&self, body: BodyId, node: BodyNodeId) -> bool {
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => checked
                .bodies()
                .get(body)
                .is_some_and(|body| body.nodes().get(node).is_some()),
            SemanticEvidence::Bodies(analysis) => match analysis.body_evidence(body) {
                Some(nocter_checking::BodyEvidence::Typed(body)) => {
                    body.nodes().get(node).is_some()
                }
                // Rejected body construction retains only its explicit interruption contract.
                // Partial checked nodes and their source projections are discarded together, so
                // the retained body-node domain is empty rather than unknown.
                Some(nocter_checking::BodyEvidence::Rejected(_)) => false,
                None => false,
            },
            SemanticEvidence::Names(_) | SemanticEvidence::Declarations(_) => false,
        }
    }

    fn local_exists(&self, body: BodyId, local: LocalBindingId) -> bool {
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => checked
                .bodies()
                .get(body)
                .is_some_and(|body| body.locals().get(local).is_some()),
            SemanticEvidence::Bodies(analysis) => analysis
                .body_names()
                .get(body)
                .is_some_and(|names| names.locals().get(local).is_some()),
            SemanticEvidence::Names(analysis) => analysis
                .body_names()
                .evidence(body)
                .and_then(nocter_checking::BodyNameEvidence::usable_names)
                .is_some_and(|names| names.locals().get(local).is_some()),
            SemanticEvidence::Declarations(_) => false,
        }
    }

    fn capture_exists(&self, body: BodyId, capture: CaptureId) -> bool {
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => checked
                .bodies()
                .get(body)
                .is_some_and(|body| body.captures().get(capture).is_some()),
            SemanticEvidence::Bodies(analysis) => analysis
                .body_names()
                .get(body)
                .is_some_and(|names| names.captures().get(capture).is_some()),
            SemanticEvidence::Names(analysis) => analysis
                .body_names()
                .evidence(body)
                .and_then(nocter_checking::BodyNameEvidence::usable_names)
                .is_some_and(|names| names.captures().get(capture).is_some()),
            SemanticEvidence::Declarations(_) => false,
        }
    }

    pub(super) const fn complete(self) -> Option<CompleteSemanticQuery<'a>> {
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
    pub(in crate::query) fn typed_body_evidence(
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
    pub(in crate::query) fn typed_body_coverage(
        &self,
    ) -> Result<SemanticCoverage, EvidenceIntegrityError> {
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

    pub(in crate::query) fn local_binding_fact(
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

    pub(in crate::query) fn capture_readonly_fact(
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

    pub(in crate::query) fn checked_operation(
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

    pub(in crate::query) fn body_scope_fact(
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

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, BodyId};

    use super::EvidenceIntegrityError;
    use crate::GenerationId;
    use crate::tests::{TempTree, bundled_snapshot};

    #[test]
    fn unknown_body_identity_is_an_integrity_failure_inside_the_query_kernel() {
        let tree = TempTree::new();
        let (_, snapshot) =
            bundled_snapshot(&tree, "func subject(): i32 { 1 }\n", GenerationId::new(56));
        let query = snapshot
            .semantic_query()
            .expect("valid semantic index")
            .expect("semantic query");
        let domain_len = query.graph().declarations().bodies().iter().count();
        let mut identities = ArenaBuilder::<BodyId, ()>::new();
        let mut missing = None;
        for _ in 0..=domain_len {
            missing = Some(identities.insert(()));
        }
        let missing = missing.unwrap();

        assert_eq!(
            query.typed_body_evidence(missing).unwrap_err(),
            EvidenceIntegrityError::MissingBodyDomain(missing)
        );
    }

    #[test]
    fn rejected_body_has_no_retained_checked_node_domain() {
        let tree = TempTree::new();
        let (_, snapshot) = bundled_snapshot(
            &tree,
            "func invalid(input: i32?): i32 { input? }\n",
            GenerationId::new(57),
        );
        let query = snapshot
            .semantic_query()
            .expect("valid semantic index")
            .expect("semantic query");
        let (body, evidence) = match query.evidence {
            super::SemanticEvidence::Bodies(recovery) => recovery
                .body_evidence_iter()
                .find(|(_, evidence)| {
                    matches!(evidence, nocter_checking::BodyEvidence::Rejected(_))
                })
                .expect("rejected body evidence"),
            _ => panic!("expected body recovery"),
        };
        assert!(matches!(
            evidence,
            nocter_checking::BodyEvidence::Rejected(_)
        ));

        let mut nodes = ArenaBuilder::<nocter_model::BodyNodeId, ()>::new();
        let node = nodes.insert(());
        assert!(!query.node_exists(body, node));
    }
}

use std::collections::BTreeSet;
use std::fmt;

use nocter_declarations::{ProvenanceAnnotation, ProvenanceOrigin};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};
use nocter_syntax::{NodeKind, SyntaxTree};

use crate::AnalysisSnapshot;
use crate::query::SemanticQueryContext;
use crate::query::callable_source::project_callable_source;
use crate::query::evidence::{
    EvidenceIntegrityError, SemanticCoverage, SemanticQuerySet, SemanticSetUnavailability,
};
use crate::query::source_context::SourceContextError;

/// One compiler-owned inlay fact before editor-coordinate projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticInlayHint {
    position: ByteOffset,
    label: Box<str>,
    kind: SemanticInlayHintKind,
}

impl SemanticInlayHint {
    #[must_use]
    pub const fn position(&self) -> ByteOffset {
        self.position
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> SemanticInlayHintKind {
        self.kind
    }
}

/// Protocol-independent categories for compiler-owned inlay facts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticInlayHintKind {
    Type,
    Provenance,
}

/// An inconsistent checked-program or source projection encountered by an inlay query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticInlayHintError {
    SourceContext(SourceContextError),
    Evidence(EvidenceIntegrityError),
    MissingSyntax(SourceId),
    MissingCallable(nocter_model::CallableId),
    MissingCallableProvenance(nocter_model::CallableId),
    InvalidCallableSource(nocter_model::CallableId),
    MissingParameter(nocter_model::ParameterId),
    UnknownSymbol(nocter_model::Symbol),
    UnrenderableType(SemanticEntity),
}

impl fmt::Display for SemanticInlayHintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceContext(error) => error.fmt(formatter),
            Self::Evidence(error) => error.fmt(formatter),
            Self::MissingSyntax(source) => {
                write!(formatter, "inlay-hint syntax for {source:?} is absent")
            }
            Self::MissingCallable(callable) => {
                write!(formatter, "inlay-hint callable {callable:?} is absent")
            }
            Self::MissingCallableProvenance(callable) => {
                write!(
                    formatter,
                    "inlay-hint provenance for {callable:?} is absent"
                )
            }
            Self::InvalidCallableSource(callable) => {
                write!(
                    formatter,
                    "callable {callable:?} has no structural source projection"
                )
            }
            Self::MissingParameter(parameter) => {
                write!(formatter, "inlay-hint parameter {parameter:?} is absent")
            }
            Self::UnknownSymbol(symbol) => {
                write!(formatter, "inlay-hint symbol {symbol:?} is absent")
            }
            Self::UnrenderableType(entity) => {
                write!(formatter, "cannot render the inlay type for {entity:?}")
            }
        }
    }
}

impl std::error::Error for SemanticInlayHintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceContext(error) => Some(error),
            Self::Evidence(error) => Some(error),
            Self::MissingSyntax(_)
            | Self::MissingCallable(_)
            | Self::MissingCallableProvenance(_)
            | Self::InvalidCallableSource(_)
            | Self::MissingParameter(_)
            | Self::UnknownSymbol(_)
            | Self::UnrenderableType(_) => None,
        }
    }
}

impl From<SourceContextError> for SemanticInlayHintError {
    fn from(error: SourceContextError) -> Self {
        Self::SourceContext(error)
    }
}

impl From<EvidenceIntegrityError> for SemanticInlayHintError {
    fn from(error: EvidenceIntegrityError) -> Self {
        Self::Evidence(error)
    }
}

impl AnalysisSnapshot {
    /// Projects inferred local-binding types retained by the current semantic authority.
    ///
    /// Explicit binding annotations suppress the corresponding hint. Syntax participates only in
    /// that suppression decision; types and visible spellings remain semantic facts. Result
    /// provenance is emitted only when the whole-program provenance authority completed.
    ///
    /// # Errors
    ///
    /// Returns an internal query error when the immutable source, body, or type projections are
    /// inconsistent.
    pub fn semantic_inlay_hints(
        &self,
        source: SourceId,
        requested: TextRange,
    ) -> Result<SemanticQuerySet<SemanticInlayHint>, SemanticInlayHintError> {
        let Some(authority) = self.semantic_query()? else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticEvidence),
            ));
        };
        let coverage = authority.typed_body_coverage()?;
        let index = authority.source_index();
        let module = authority.module_for_source(source)?;
        let syntax = self
            .syntax_trees()
            .iter()
            .find(|tree| tree.source() == source)
            .ok_or(SemanticInlayHintError::MissingSyntax(source))?;
        let context = InlayContext {
            authority,
            index,
            source,
            spellings: self
                .queries
                .source_spellings(authority.graph(), module, index, source),
            syntax,
            requested,
        };
        let mut hints = context.local_type_hints()?;
        hints.extend(context.callable_provenance_hints()?);
        hints.sort_unstable_by_key(|hint| (hint.position, hint.kind));
        hints.dedup_by_key(|hint| (hint.position, hint.kind));
        Ok(SemanticQuerySet::new(hints.into_boxed_slice(), coverage))
    }
}

struct InlayContext<'a> {
    authority: SemanticQueryContext<'a>,
    index: &'a SourceIndex,
    source: SourceId,
    spellings: std::sync::Arc<crate::query::presentation::visible_spelling::VisibleSpellings>,
    syntax: &'a SyntaxTree,
    requested: TextRange,
}

impl InlayContext<'_> {
    fn local_type_hints(&self) -> Result<Vec<SemanticInlayHint>, SemanticInlayHintError> {
        let annotated = annotated_binding_targets(self.syntax);
        let mut hints = Vec::new();
        for binding in self.index.bindings_in(self.source) {
            if binding.role() != SourceRole::Declaration
                || annotated.contains(&binding.origin().span().range())
            {
                continue;
            }
            let SemanticEntity::LocalBinding(body, local) = binding.entity() else {
                continue;
            };
            let position = binding.origin().span().range().end();
            if !self.requested.contains_offset(position) {
                continue;
            }
            let Ok(checked_local) = self
                .authority
                .local_binding_fact(body, local)?
                .into_result()
            else {
                continue;
            };
            let entity = binding.entity();
            let rendered = crate::query::presentation::type_presentation_with_spellings(
                self.authority.graph(),
                self.authority.types(),
                checked_local.ty(),
                &self.spellings,
            )
            .ok_or(SemanticInlayHintError::UnrenderableType(entity))?;
            hints.push(SemanticInlayHint {
                position,
                label: format!(": {}", rendered.code()).into(),
                kind: SemanticInlayHintKind::Type,
            });
        }
        Ok(hints)
    }

    fn callable_provenance_hints(&self) -> Result<Vec<SemanticInlayHint>, SemanticInlayHintError> {
        let Some(complete) = self.authority.complete() else {
            return Ok(Vec::new());
        };
        let checked = complete.checked();
        let mut hints = Vec::new();
        for binding in self.index.bindings_in(self.source) {
            if binding.role() != SourceRole::Declaration {
                continue;
            }
            let SemanticEntity::Callable(callable) = binding.entity() else {
                continue;
            };
            let declaration = checked
                .graph()
                .declarations()
                .callables()
                .get(callable)
                .ok_or(SemanticInlayHintError::MissingCallable(callable))?;
            if declaration.provenance_annotation() != ProvenanceAnnotation::Elided {
                continue;
            }
            let provenance = checked
                .provenance()
                .callables()
                .get(callable)
                .ok_or(SemanticInlayHintError::MissingCallableProvenance(callable))?;
            if provenance.origins().is_empty() {
                continue;
            }
            let projection =
                project_callable_source(self.syntax, binding.origin().syntax(), declaration.kind())
                    .ok_or(SemanticInlayHintError::InvalidCallableSource(callable))?;
            let position = projection
                .result_end()
                .ok_or(SemanticInlayHintError::InvalidCallableSource(callable))?;
            if !self.requested.contains_offset(position) {
                continue;
            }
            let mut label = String::from(" from ");
            for (index, origin) in provenance.origins().iter().copied().enumerate() {
                if index != 0 {
                    label.push_str(" | ");
                }
                match origin {
                    ProvenanceOrigin::Receiver => label.push_str("self"),
                    ProvenanceOrigin::Parameter(parameter) => {
                        let parameter = checked
                            .graph()
                            .declarations()
                            .parameters()
                            .get(parameter)
                            .ok_or(SemanticInlayHintError::MissingParameter(parameter))?;
                        label.push_str(
                            checked
                                .graph()
                                .symbols()
                                .spelling(parameter.name())
                                .ok_or(SemanticInlayHintError::UnknownSymbol(parameter.name()))?,
                        );
                    }
                }
            }
            hints.push(SemanticInlayHint {
                position,
                label: label.into(),
                kind: SemanticInlayHintKind::Provenance,
            });
        }
        Ok(hints)
    }
}

fn annotated_binding_targets(syntax: &SyntaxTree) -> BTreeSet<TextRange> {
    let mut targets = BTreeSet::new();
    for (binding, node) in syntax.nodes() {
        if node.kind() != NodeKind::BindingStatement
            || nocter_syntax::direct_node(syntax, binding, NodeKind::TypeAnnotation).is_none()
        {
            continue;
        }
        let Some(pattern) = nocter_syntax::direct_node(syntax, binding, NodeKind::BindingPattern)
        else {
            continue;
        };
        targets.extend(
            nocter_syntax::descendant_identifier_iter(syntax, pattern)
                .map(nocter_syntax::SyntaxToken::range),
        );
    }
    targets
}

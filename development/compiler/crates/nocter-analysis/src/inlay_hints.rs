use std::collections::BTreeSet;
use std::fmt;

use nocter_checking::CheckedProgram;
use nocter_declarations::{ProvenanceAnnotation, ProvenanceOrigin};
use nocter_model::ModuleId;
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};
use nocter_syntax::{NodeKind, SyntaxElement, SyntaxTree};

use crate::AnalysisSnapshot;
use crate::callable_source::project_callable_source;
use crate::source_context::{SourceContext, SourceContextError};

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
    MissingSyntax(SourceId),
    MissingBody(nocter_model::BodyId),
    MissingLocal {
        body: nocter_model::BodyId,
        local: nocter_model::LocalBindingId,
    },
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
            Self::MissingSyntax(source) => {
                write!(formatter, "inlay-hint syntax for {source:?} is absent")
            }
            Self::MissingBody(body) => write!(formatter, "inlay-hint body {body:?} is absent"),
            Self::MissingLocal { body, local } => {
                write!(formatter, "inlay-hint local {body:?}/{local:?} is absent")
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
            Self::MissingSyntax(_)
            | Self::MissingBody(_)
            | Self::MissingLocal { .. }
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

impl AnalysisSnapshot {
    /// Projects inferred local-binding types retained by one successful checked generation.
    ///
    /// Explicit binding annotations suppress the corresponding hint. Syntax participates only in
    /// that suppression decision; types and visible spellings remain checked-program facts.
    ///
    /// # Errors
    ///
    /// Returns an internal query error when the immutable source, body, or type projections are
    /// inconsistent.
    pub fn semantic_inlay_hints(
        &self,
        source: SourceId,
        requested: TextRange,
    ) -> Result<Box<[SemanticInlayHint]>, SemanticInlayHintError> {
        let Some(target) = self.target() else {
            return Ok(Box::new([]));
        };
        let Some(index) = self.source_index() else {
            return Ok(Box::new([]));
        };
        let checked = target.program().checked();
        let module = SourceContext::resolve(index, source)?.module();
        let syntax = self
            .syntax_trees()
            .iter()
            .find(|tree| tree.source() == source)
            .ok_or(SemanticInlayHintError::MissingSyntax(source))?;
        let context = InlayContext {
            checked,
            index,
            source,
            module,
            syntax,
            requested,
        };
        let mut hints = context.local_type_hints()?;
        hints.extend(context.callable_provenance_hints()?);
        hints.sort_unstable_by_key(|hint| (hint.position, hint.kind));
        hints.dedup_by_key(|hint| (hint.position, hint.kind));
        Ok(hints.into_boxed_slice())
    }
}

struct InlayContext<'a> {
    checked: &'a CheckedProgram,
    index: &'a SourceIndex,
    source: SourceId,
    module: ModuleId,
    syntax: &'a SyntaxTree,
    requested: TextRange,
}

impl InlayContext<'_> {
    fn local_type_hints(&self) -> Result<Vec<SemanticInlayHint>, SemanticInlayHintError> {
        let annotated = annotated_binding_targets(self.syntax);
        let spellings = crate::presentation::visible_spelling::VisibleSpellings::for_source(
            self.checked.graph(),
            self.module,
            self.index,
            self.source,
        );
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
            let checked_body = self
                .checked
                .bodies()
                .get(body)
                .ok_or(SemanticInlayHintError::MissingBody(body))?;
            let checked_local = checked_body
                .locals()
                .get(local)
                .ok_or(SemanticInlayHintError::MissingLocal { body, local })?;
            let entity = binding.entity();
            let rendered = crate::presentation::type_presentation_with_spellings(
                self.checked,
                checked_local.ty(),
                &spellings,
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
        let mut hints = Vec::new();
        for binding in self.index.bindings_in(self.source) {
            if binding.role() != SourceRole::Declaration {
                continue;
            }
            let SemanticEntity::Callable(callable) = binding.entity() else {
                continue;
            };
            let declaration = self
                .checked
                .graph()
                .declarations()
                .callables()
                .get(callable)
                .ok_or(SemanticInlayHintError::MissingCallable(callable))?;
            if declaration.provenance_annotation() != ProvenanceAnnotation::Elided {
                continue;
            }
            let provenance = self
                .checked
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
                        let parameter = self
                            .checked
                            .graph()
                            .declarations()
                            .parameters()
                            .get(parameter)
                            .ok_or(SemanticInlayHintError::MissingParameter(parameter))?;
                        label.push_str(
                            self.checked
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
    syntax
        .nodes()
        .filter(|(_, node)| node.kind() == NodeKind::BindingStatement)
        .filter_map(|(binding, _)| {
            let mut target = None;
            let mut annotation = false;
            for child in syntax.children(binding) {
                let SyntaxElement::Node(child) = child else {
                    continue;
                };
                match syntax.node(*child)?.kind() {
                    NodeKind::BindingTarget => {
                        target = syntax.node(*child).map(nocter_syntax::SyntaxNode::range);
                    }
                    NodeKind::TypeAnnotation => annotation = true,
                    _ => {}
                }
            }
            annotation.then_some(target).flatten()
        })
        .collect()
}

use nocter_declarations::{CallableKind, NominalShape, ParameterRole};
use nocter_model::CallableCapability;
use nocter_source::{SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceAccess, SourceBinding, SourceRole};
use nocter_syntax::TokenKind;

use crate::AnalysisSnapshot;
use crate::query::evidence::{
    EvidenceIntegrityError, SemanticCoverage, SemanticQuerySet, SemanticSetUnavailability,
};
use crate::query::source_selection::select_source_binding;

/// Protocol-independent semantic classification of one source range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticHighlight {
    range: TextRange,
    kind: SemanticHighlightKind,
    declaration: bool,
    readonly: bool,
}

impl SemanticHighlight {
    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }

    #[must_use]
    pub const fn kind(self) -> SemanticHighlightKind {
        self.kind
    }

    #[must_use]
    pub const fn is_declaration(self) -> bool {
        self.declaration
    }

    #[must_use]
    pub const fn is_readonly(self) -> bool {
        self.readonly
    }
}

/// Closed compiler-owned highlight categories independent of any editor protocol's numeric legend.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticHighlightKind {
    Namespace,
    Type,
    Struct,
    Enum,
    Interface,
    TypeParameter,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Function,
    Method,
    Keyword,
    CharacterLiteral,
}

impl AnalysisSnapshot {
    /// Classifies every exact semantic binding plus accepted syntax-owned scalar literal available
    /// from the deepest current authority.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when a source occurrence names a semantic domain absent from the
    /// immutable evidence result.
    pub fn semantic_highlights(
        &self,
        source: SourceId,
    ) -> Result<SemanticQuerySet<SemanticHighlight>, EvidenceIntegrityError> {
        let Some(authority) = self.semantic_query()? else {
            return Ok(SemanticQuerySet::new(
                Box::new([]),
                SemanticCoverage::Unavailable(SemanticSetUnavailability::NoSemanticEvidence),
            ));
        };
        let coverage = authority.typed_body_coverage()?;
        let index = authority.source_index();
        let mut candidates = Vec::new();
        for binding in index.bindings_in(source) {
            if highlight(authority, binding)?.is_some() {
                candidates.push(binding);
            }
        }
        let mut highlights = Vec::new();
        let mut start = 0;
        while start < candidates.len() {
            let range = candidates[start].origin().span().range();
            let end = candidates[start..]
                .partition_point(|binding| binding.origin().span().range() == range)
                + start;
            if let Some(binding) =
                select_source_binding(candidates[start..end].iter().copied(), |_| true).unique()
                && let Some(highlight) = highlight(authority, &binding)?
            {
                highlights.push(highlight);
            }
            start = end;
        }
        if let Some(syntax) = self
            .syntax_trees()
            .iter()
            .find(|syntax| syntax.source() == source)
        {
            highlights.extend(
                syntax
                    .lexed()
                    .tokens()
                    .iter()
                    .copied()
                    .filter(|token| token.kind() == TokenKind::CharacterLiteral)
                    .map(|token| SemanticHighlight {
                        range: token.span().range(),
                        kind: SemanticHighlightKind::CharacterLiteral,
                        declaration: false,
                        readonly: false,
                    }),
            );
        }
        highlights.sort_unstable_by_key(|highlight| {
            (highlight.range.start().get(), highlight.range.end().get())
        });
        Ok(SemanticQuerySet::new(
            highlights.into_boxed_slice(),
            coverage,
        ))
    }
}

fn highlight(
    authority: crate::query::SemanticQueryContext<'_>,
    binding: &SourceBinding,
) -> Result<Option<SemanticHighlight>, EvidenceIntegrityError> {
    let Some((kind, readonly)) = classify(authority, binding)? else {
        return Ok(None);
    };
    if matches!(binding.entity(), SemanticEntity::Module(_))
        && binding.role() != SourceRole::Reference
    {
        return Ok(None);
    }
    let range = binding.origin().span().range();
    if range.is_empty() {
        return Ok(None);
    }
    Ok(Some(SemanticHighlight {
        range,
        kind,
        declaration: binding.role() != SourceRole::Reference,
        readonly,
    }))
}

fn classify(
    authority: crate::query::SemanticQueryContext<'_>,
    binding: &SourceBinding,
) -> Result<Option<(SemanticHighlightKind, bool)>, EvidenceIntegrityError> {
    let graph = authority.graph();
    let entity = binding.entity();
    let declarations = graph.declarations();
    let kind = match entity {
        SemanticEntity::BuiltinType(_) => SemanticHighlightKind::Type,
        SemanticEntity::Module(_) => SemanticHighlightKind::Namespace,
        SemanticEntity::NominalType(id) => {
            let declaration = declarations
                .nominal_types()
                .get(id)
                .ok_or(EvidenceIntegrityError::MissingSemanticEntity(entity))?;
            match declaration.shape() {
                NominalShape::Struct { .. } => SemanticHighlightKind::Struct,
                NominalShape::Enum { .. } => SemanticHighlightKind::Enum,
            }
        }
        SemanticEntity::TypeAlias(_) | SemanticEntity::AssociatedType(_) => {
            SemanticHighlightKind::Type
        }
        SemanticEntity::Interface(_) => SemanticHighlightKind::Interface,
        SemanticEntity::GenericParameter(_) => SemanticHighlightKind::TypeParameter,
        SemanticEntity::Constant(_) => {
            return Ok(Some((SemanticHighlightKind::Variable, true)));
        }
        SemanticEntity::Parameter(id) => {
            let parameter = declarations
                .parameters()
                .get(id)
                .ok_or(EvidenceIntegrityError::MissingSemanticEntity(entity))?;
            let readonly = match parameter.role() {
                ParameterRole::Ordinary { .. } | ParameterRole::ArgumentPack { .. } => true,
                ParameterRole::Receiver(capability) => capability == CallableCapability::Readonly,
            };
            return Ok(Some((SemanticHighlightKind::Parameter, readonly)));
        }
        SemanticEntity::LocalBinding(body, id) => {
            let Ok(local) = authority.local_binding_fact(body, id)?.into_result() else {
                return Ok(None);
            };
            return Ok(Some((SemanticHighlightKind::Variable, local.readonly())));
        }
        SemanticEntity::Capture(body, id) => {
            let Ok(readonly) = authority.capture_readonly_fact(body, id)?.into_result() else {
                return Ok(None);
            };
            return Ok(Some((SemanticHighlightKind::Variable, readonly)));
        }
        SemanticEntity::Field(_) | SemanticEntity::PlaceProjection(..) => {
            return Ok(Some((
                SemanticHighlightKind::Property,
                binding.access() == Some(SourceAccess::Readonly),
            )));
        }
        SemanticEntity::Variant(_) => SemanticHighlightKind::EnumMember,
        SemanticEntity::Callable(id) => {
            let callable = declarations
                .callables()
                .get(id)
                .ok_or(EvidenceIntegrityError::MissingSemanticEntity(entity))?;
            match callable.kind() {
                CallableKind::Function
                | CallableKind::Primitive
                | CallableKind::ConstructionFunction
                | CallableKind::Literal(_) => SemanticHighlightKind::Function,
                CallableKind::Method
                | CallableKind::Coercion
                | CallableKind::Equality
                | CallableKind::Ordering
                | CallableKind::Index
                | CallableKind::Expansion => SemanticHighlightKind::Method,
            }
        }
        SemanticEntity::Test(_) => SemanticHighlightKind::Function,
        SemanticEntity::OpaqueType(_) => SemanticHighlightKind::Keyword,
        SemanticEntity::Package(_)
        | SemanticEntity::PackageTarget(_)
        | SemanticEntity::Import(_)
        | SemanticEntity::DeclarationSite(_)
        | SemanticEntity::Construction(_)
        | SemanticEntity::Instance(_)
        | SemanticEntity::InterfaceImplementation(_)
        | SemanticEntity::Drop(_)
        | SemanticEntity::Requirement(_)
        | SemanticEntity::CapabilityEvidence(_)
        | SemanticEntity::Body(_)
        | SemanticEntity::BodyScope(..)
        | SemanticEntity::BodyNode(..) => return Ok(None),
    };
    Ok(Some((kind, false)))
}

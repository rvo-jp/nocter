use nocter_checking::{CaptureMode, LocalBindingKind};
use nocter_declarations::{CallableKind, NominalShape, ParameterRole};
use nocter_model::CallableCapability;
use nocter_source::{SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceAccess, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;
use crate::source_selection::select_source_binding;

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

/// Closed semantic categories independent of any editor protocol's numeric legend.
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
}

impl AnalysisSnapshot {
    /// Classifies every exact semantic binding available from the deepest current authority.
    #[must_use]
    pub fn semantic_highlights(&self, source: SourceId) -> Box<[SemanticHighlight]> {
        let Some(authority) = self.semantic_authority() else {
            return Box::new([]);
        };
        let index = authority.source_index();
        let candidates = index
            .bindings_in(source)
            .filter(|binding| highlight(authority, binding).is_some())
            .collect::<Vec<_>>();
        let mut highlights = Vec::new();
        let mut start = 0;
        while start < candidates.len() {
            let range = candidates[start].origin().span().range();
            let end = candidates[start..]
                .partition_point(|binding| binding.origin().span().range() == range)
                + start;
            if let Some(binding) =
                select_source_binding(candidates[start..end].iter().copied(), |_| true).unique()
                && let Some(highlight) = highlight(authority, &binding)
            {
                highlights.push(highlight);
            }
            start = end;
        }
        highlights.into_boxed_slice()
    }
}

fn highlight(
    authority: crate::semantic::SemanticAuthority<'_>,
    binding: &SourceBinding,
) -> Option<SemanticHighlight> {
    let (kind, readonly) = classify(authority, binding)?;
    if matches!(binding.entity(), SemanticEntity::Module(_))
        && binding.role() != SourceRole::Reference
    {
        return None;
    }
    let range = binding.origin().span().range();
    if range.is_empty() {
        return None;
    }
    Some(SemanticHighlight {
        range,
        kind,
        declaration: binding.role() != SourceRole::Reference,
        readonly,
    })
}

fn classify(
    authority: crate::semantic::SemanticAuthority<'_>,
    binding: &SourceBinding,
) -> Option<(SemanticHighlightKind, bool)> {
    let graph = authority.graph();
    let entity = binding.entity();
    let declarations = graph.declarations();
    let kind = match entity {
        SemanticEntity::BuiltinType(_) => SemanticHighlightKind::Type,
        SemanticEntity::Module(_) => SemanticHighlightKind::Namespace,
        SemanticEntity::NominalType(id) => match declarations.nominal_types().get(id)?.shape() {
            NominalShape::Struct { .. } => SemanticHighlightKind::Struct,
            NominalShape::Enum { .. } => SemanticHighlightKind::Enum,
        },
        SemanticEntity::TypeAlias(_) | SemanticEntity::AssociatedType(_) => {
            SemanticHighlightKind::Type
        }
        SemanticEntity::Interface(_) => SemanticHighlightKind::Interface,
        SemanticEntity::GenericParameter(_) => SemanticHighlightKind::TypeParameter,
        SemanticEntity::Constant(_) => {
            return Some((SemanticHighlightKind::Variable, true));
        }
        SemanticEntity::Parameter(id) => {
            let parameter = declarations.parameters().get(id)?;
            let readonly = match parameter.role() {
                ParameterRole::Ordinary { .. } | ParameterRole::ArgumentPack { .. } => true,
                ParameterRole::Receiver(capability) => capability == CallableCapability::Readonly,
            };
            return Some((SemanticHighlightKind::Parameter, readonly));
        }
        SemanticEntity::LocalBinding(body, id) => {
            let local = authority.body(body)?.locals().get(id)?;
            let readonly = local.declaration().kind() != LocalBindingKind::Mutable;
            return Some((SemanticHighlightKind::Variable, readonly));
        }
        SemanticEntity::Capture(body, id) => {
            let capture = authority.body(body)?.captures().get(id)?;
            let readonly = capture.declaration().mode() == CaptureMode::Readonly;
            return Some((SemanticHighlightKind::Variable, readonly));
        }
        SemanticEntity::Field(_) => {
            return Some((
                SemanticHighlightKind::Property,
                binding.access() == Some(SourceAccess::Readonly),
            ));
        }
        SemanticEntity::Variant(_) => SemanticHighlightKind::EnumMember,
        SemanticEntity::Callable(id) => match declarations.callables().get(id)?.kind() {
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
        },
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
        | SemanticEntity::Body(_)
        | SemanticEntity::BodyScope(..)
        | SemanticEntity::BodyNode(..) => return None,
    };
    Some((kind, false))
}

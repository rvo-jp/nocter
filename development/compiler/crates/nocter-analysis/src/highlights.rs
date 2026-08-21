use nocter_checking::{CaptureMode, CheckedProgram, LocalBindingKind};
use nocter_declarations::{CallableKind, NominalShape, ParameterRole};
use nocter_model::CallableCapability;
use nocter_source::{SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceAccess, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;

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
    /// Classifies every exact semantic binding in one source of the current successful snapshot.
    #[must_use]
    pub fn semantic_highlights(&self, source: SourceId) -> Box<[SemanticHighlight]> {
        let Some(target) = self.target() else {
            return Box::new([]);
        };
        let checked = target.program().checked();
        let Some(index) = self.source_index() else {
            return Box::new([]);
        };
        let mut highlights = index
            .bindings_in(source)
            .filter_map(|binding| highlight(checked, binding))
            .collect::<Vec<_>>();
        highlights
            .sort_unstable_by_key(|item| (item.range().start(), item.range().end(), item.kind()));
        highlights.dedup_by_key(|item| item.range());
        highlights.into_boxed_slice()
    }
}

fn highlight(checked: &CheckedProgram, binding: &SourceBinding) -> Option<SemanticHighlight> {
    let (kind, readonly) = classify(checked, binding)?;
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
    checked: &CheckedProgram,
    binding: &SourceBinding,
) -> Option<(SemanticHighlightKind, bool)> {
    let entity = binding.entity();
    let declarations = checked.graph().declarations();
    let kind = match entity {
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
        SemanticEntity::Parameter(id) => {
            let parameter = declarations.parameters().get(id)?;
            let readonly = match parameter.role() {
                ParameterRole::Ordinary { .. } => true,
                ParameterRole::Receiver(capability) => capability == CallableCapability::Readonly,
            };
            return Some((SemanticHighlightKind::Parameter, readonly));
        }
        SemanticEntity::LocalBinding(body, id) => {
            let local = checked.bodies().get(body)?.locals().get(id)?;
            let readonly = local.declaration().kind() != LocalBindingKind::Mutable;
            return Some((SemanticHighlightKind::Variable, readonly));
        }
        SemanticEntity::Capture(body, id) => {
            let capture = checked.bodies().get(body)?.captures().get(id)?;
            let readonly = capture.declaration().mode() == CaptureMode::Readonly;
            return Some((SemanticHighlightKind::Variable, readonly));
        }
        SemanticEntity::Field(_) | SemanticEntity::BuiltinField(_) => {
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
        | SemanticEntity::Conformance(_)
        | SemanticEntity::Drop(_)
        | SemanticEntity::Requirement(_)
        | SemanticEntity::Body(_)
        | SemanticEntity::BodyScope(..)
        | SemanticEntity::BodyNode(..) => return None,
    };
    Some((kind, false))
}

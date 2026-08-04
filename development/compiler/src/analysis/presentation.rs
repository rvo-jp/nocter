//! Structured semantic presentation shared by editor features.

use crate::ast::TypeExpr;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind, TypeSymbolKind};
use crate::typecheck::type_expr_presentation_label;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclarationKind {
    TypeAlias,
    Struct,
    Enum,
    Interface,
}

impl DeclarationKind {
    fn keyword(self) -> &'static str {
        match self {
            Self::TypeAlias => "type",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Interface => "interface",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeDeclarationPresentation {
    kind: DeclarationKind,
    displayed_type: String,
}

impl TypeDeclarationPresentation {
    pub(crate) fn render(&self) -> String {
        format!("{} {}", self.kind.keyword(), self.displayed_type)
    }
}

pub(crate) fn type_reference_presentation(
    symbol: &Symbol,
    contextual_type: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<TypeDeclarationPresentation> {
    let SymbolKind::Type(type_symbol) = &symbol.kind else {
        return None;
    };
    let kind = match type_symbol.kind {
        TypeSymbolKind::Alias => DeclarationKind::TypeAlias,
        TypeSymbolKind::Struct => DeclarationKind::Struct,
        TypeSymbolKind::Enum => DeclarationKind::Enum,
        TypeSymbolKind::Interface => DeclarationKind::Interface,
    };

    Some(TypeDeclarationPresentation {
        kind,
        displayed_type: type_expr_presentation_label(contextual_type, resolved),
    })
}

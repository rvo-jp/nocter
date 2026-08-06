use crate::ast::TypeExpr;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind, TypeSymbol, TypeSymbolKind};
use crate::typecheck::type_expr_presentation_label;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclarationKind {
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
    is_copy: bool,
    alias_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenericParameterPresentation {
    name: String,
    bounds: Vec<String>,
}

impl GenericParameterPresentation {
    pub(crate) fn render(&self) -> String {
        if self.bounds.is_empty() {
            format!("type parameter {}", self.name)
        } else {
            format!("type parameter {}: {}", self.name, self.bounds.join(" + "))
        }
    }
}

pub(crate) fn generic_parameter_presentation(
    parameter: &crate::typecheck::GenericParameterFact,
    resolved: &ResolveOutput,
) -> GenericParameterPresentation {
    GenericParameterPresentation {
        name: parameter.name.clone(),
        bounds: parameter
            .bounds
            .iter()
            .map(|bound| type_expr_presentation_label(bound, resolved))
            .collect(),
    }
}

impl TypeDeclarationPresentation {
    pub(crate) fn render(&self) -> String {
        let copy = if self.kind == DeclarationKind::Struct && self.is_copy {
            "copy "
        } else {
            ""
        };
        let target = self
            .alias_target
            .as_ref()
            .map(|target| format!(" = {target}"))
            .unwrap_or_default();
        format!(
            "{copy}{} {}{target}",
            self.kind.keyword(),
            self.displayed_type
        )
    }
}

pub(crate) fn type_declaration_presentation(
    symbol: &Symbol,
    resolved: &ResolveOutput,
) -> Option<TypeDeclarationPresentation> {
    let SymbolKind::Type(type_symbol) = &symbol.kind else {
        return None;
    };
    Some(TypeDeclarationPresentation {
        kind: declaration_kind(type_symbol),
        displayed_type: declared_type_label(symbol, type_symbol, resolved),
        is_copy: type_symbol.is_copy,
        alias_target: type_symbol
            .alias_target
            .as_ref()
            .map(|target| type_expr_presentation_label(target, resolved)),
    })
}

pub(crate) fn type_reference_presentation(
    symbol: &Symbol,
    contextual_type: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<TypeDeclarationPresentation> {
    let SymbolKind::Type(type_symbol) = &symbol.kind else {
        return None;
    };
    Some(TypeDeclarationPresentation {
        kind: declaration_kind(type_symbol),
        displayed_type: type_expr_presentation_label(contextual_type, resolved),
        is_copy: type_symbol.is_copy,
        alias_target: None,
    })
}

fn declaration_kind(symbol: &TypeSymbol) -> DeclarationKind {
    match symbol.kind {
        TypeSymbolKind::Alias => DeclarationKind::TypeAlias,
        TypeSymbolKind::Struct => DeclarationKind::Struct,
        TypeSymbolKind::Enum => DeclarationKind::Enum,
        TypeSymbolKind::Interface => DeclarationKind::Interface,
    }
}

fn declared_type_label(
    symbol: &Symbol,
    type_symbol: &TypeSymbol,
    resolved: &ResolveOutput,
) -> String {
    let visible_name = if crate::lexer::is_valid_identifier_name(&symbol.name) {
        symbol.name.clone()
    } else {
        crate::typecheck::type_symbol_presentation_label(type_symbol, resolved)
    };
    if type_symbol.generic_parameters.is_empty() {
        return visible_name;
    }
    let parameters = type_symbol
        .generic_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let Some(bounds) = type_symbol.generic_parameter_bounds.get(index) else {
                return parameter.clone();
            };
            if bounds.is_empty() {
                return parameter.clone();
            }
            format!(
                "{parameter}: {}",
                bounds
                    .iter()
                    .map(|bound| type_expr_presentation_label(bound, resolved))
                    .collect::<Vec<_>>()
                    .join(" + ")
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{visible_name}<{parameters}>")
}

pub(crate) fn type_owner_presentation_label(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
) -> String {
    let name = crate::typecheck::type_symbol_presentation_label(symbol, resolved);
    if symbol.generic_parameters.is_empty() {
        name
    } else {
        format!("{name}<{}>", symbol.generic_parameters.join(", "))
    }
}

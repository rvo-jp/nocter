use nocter_model::Symbol;
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{NodeId, SyntaxElement, SyntaxToken, SyntaxTree};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclarationId};

use super::{TypeBindingError, TypeBindingRule};

pub(super) fn declaration_module(
    namespaces: &PreparedNamespaces<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<nocter_model::ModuleId, TypeBindingError> {
    let reserved = &namespaces.imports.generics.headers.reserved;
    let surface = reserved
        .declarations
        .get(declaration.index())
        .ok_or(TypeBindingError::MissingSource(declaration))?;
    reserved
        .module_for_source(surface.source())
        .ok_or(TypeBindingError::MissingSource(declaration))
}

pub(super) fn require_arity(
    namespaces: &PreparedNamespaces<'_>,
    origin: SyntaxOrigin,
    entity: ReservedEntity,
    actual: usize,
) -> Result<(), TypeBindingError> {
    let generics = &namespaces.imports.generics;
    let expected = generics
        .headers
        .reserved
        .entities
        .iter()
        .position(|candidate| *candidate == Some(entity))
        .and_then(|index| generics.own.get(index))
        .map_or(0, |parameters| parameters.len());
    if expected == actual {
        Ok(())
    } else {
        Err(TypeBindingError::rule(
            TypeBindingRule::InvalidTypeArguments,
            origin,
        ))
    }
}

pub(super) fn token_symbol(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<Symbol, TypeBindingError> {
    let text = token_text(namespaces, tree, token)?;
    namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .symbols()
        .get(text)
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))
}

pub(super) fn token_text<'a>(
    namespaces: &'a PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    token: SyntaxToken,
) -> Result<&'a str, TypeBindingError> {
    let source = namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_map
        .get(token.source())
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))?;
    source
        .text_at(token.range())
        .ok_or(TypeBindingError::InconsistentSource(tree.source()))
}

pub(super) fn builtin_type(
    namespaces: &PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    node: NodeId,
) -> Result<nocter_model::BuiltinType, TypeBindingError> {
    let token = tree
        .children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => Some(*token),
            _ => None,
        })
        .ok_or(TypeBindingError::InvalidSyntax(node))?;
    match token_text(namespaces, tree, token)? {
        "bool" => Ok(nocter_model::BuiltinType::Bool),
        "i8" => Ok(nocter_model::BuiltinType::I8),
        "i16" => Ok(nocter_model::BuiltinType::I16),
        "i32" => Ok(nocter_model::BuiltinType::I32),
        "i64" => Ok(nocter_model::BuiltinType::I64),
        "u8" => Ok(nocter_model::BuiltinType::U8),
        "u16" => Ok(nocter_model::BuiltinType::U16),
        "u32" => Ok(nocter_model::BuiltinType::U32),
        "u64" => Ok(nocter_model::BuiltinType::U64),
        "usize" => Ok(nocter_model::BuiltinType::Usize),
        "isize" => Ok(nocter_model::BuiltinType::Isize),
        "str" => Ok(nocter_model::BuiltinType::Str),
        "error" => Ok(nocter_model::BuiltinType::Error),
        "void" => Ok(nocter_model::BuiltinType::Void),
        "never" => Ok(nocter_model::BuiltinType::Never),
        _ => Err(TypeBindingError::InvalidSyntax(node)),
    }
}

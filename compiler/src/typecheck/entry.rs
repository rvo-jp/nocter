use super::diagnostics::{invalid_entry_function_diagnostic, missing_entry_function_diagnostic};
use super::model::Type;
use super::type_expr::type_expr_to_type;
use crate::ast::{AstFile, FunctionDecl, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_default_entry_function(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry) = find_entry_function(ast) else {
        diagnostics.push(missing_entry_function_diagnostic(sources, ast.span));
        return;
    };

    if !entry.parameters.parameters.is_empty()
        || !is_valid_entry_return_type(&entry.return_type, resolved)
    {
        diagnostics.push(invalid_entry_function_diagnostic(sources, entry.span));
    }
}

pub(super) fn find_entry_function(ast: &AstFile) -> Option<&FunctionDecl> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == DEFAULT_ENTRY_NAME => Some(function),
        _ => None,
    })
}

pub(super) fn is_valid_entry_return_type(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    is_valid_entry_return_model_type(&type_expr_to_type(ty, resolved))
}

fn is_valid_entry_return_model_type(ty: &Type) -> bool {
    match ty {
        Type::I32 | Type::Void => true,
        Type::Primitive(name) if name == "usize" => true,
        Type::Fallible { success, .. } => is_valid_entry_return_model_type(success),
        _ => false,
    }
}

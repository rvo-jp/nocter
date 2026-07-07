use super::diagnostics::{invalid_entry_function_diagnostic, missing_entry_function_diagnostic};
use crate::ast::{AstFile, FunctionDecl, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::source::SourceMap;

pub(super) fn check_default_entry_function(
    sources: &SourceMap,
    ast: &AstFile,
    entry_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry) = find_entry_function(ast, entry_name) else {
        diagnostics.push(missing_entry_function_diagnostic(
            sources, ast.span, entry_name,
        ));
        return;
    };

    if !entry.parameters.parameters.is_empty() || !is_valid_entry_return_type(&entry.return_type) {
        diagnostics.push(invalid_entry_function_diagnostic(
            sources, entry.span, entry_name,
        ));
    }
}

pub(super) fn find_entry_function<'a>(
    ast: &'a AstFile,
    entry_name: &str,
) -> Option<&'a FunctionDecl> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == entry_name => Some(function),
        _ => None,
    })
}

pub(super) fn is_valid_entry_return_type(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Reference(reference) => reference.name == "i32" || reference.name == "void",
        TypeExpr::Fallible(fallible) => {
            matches!(fallible.success.as_ref(), TypeExpr::Reference(reference) if reference.name == "i32")
        }
        _ => false,
    }
}

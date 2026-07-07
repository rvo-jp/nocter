use super::*;

pub(super) fn check_program_entry(
    sources: &SourceMap,
    ast: &AstFile,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let programs: Vec<&ProgramDecl> = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Program(program) => Some(program),
            _ => None,
        })
        .collect();

    match programs.as_slice() {
        [] => {
            if let Some(main) = find_main_function(ast) {
                diagnostics.push(main_is_not_entry_diagnostic(sources, main));
            } else {
                diagnostics.push(missing_program_diagnostic(sources, ast.span));
            }
        }
        [program] => {
            if !is_valid_program_return_type(&program.return_type) {
                diagnostics.push(invalid_program_return_type_diagnostic(
                    sources,
                    program.return_type.span(),
                ));
            }
        }
        [first, second, ..] => {
            diagnostics.push(duplicate_program_diagnostic(
                sources,
                first.span,
                second.span,
            ));
        }
    }
}

fn find_main_function(ast: &AstFile) -> Option<&FunctionDecl> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == "main" => Some(function),
        _ => None,
    })
}

pub(super) fn is_valid_program_return_type(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Reference(reference) => reference.name == "i32" || reference.name == "void",
        TypeExpr::Fallible(fallible) => {
            matches!(fallible.success.as_ref(), TypeExpr::Reference(reference) if reference.name == "i32")
        }
        _ => false,
    }
}

//! Type checking, ownership, borrowing, move, and drop checks.

use crate::ast::{AstFile, FunctionDecl, Item, ProgramDecl, TypeExpr};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::{ByteSpan, SourceMap};

pub fn check(sources: &SourceMap, ast: &AstFile) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_program_entry(sources, ast, &mut diagnostics);

    diagnostics
}

fn check_program_entry(sources: &SourceMap, ast: &AstFile, diagnostics: &mut Vec<Diagnostic>) {
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

fn is_valid_program_return_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "i32" || reference.name == "void")
}

fn missing_program_diagnostic(sources: &SourceMap, span: ByteSpan) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0300",
        "executable root file must define exactly one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("add `program(): i32 { ... }` or `program(): void { ... }`".to_string());
    diagnostic
}

fn main_is_not_entry_diagnostic(sources: &SourceMap, function: &FunctionDecl) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0301",
        "`func main` is an ordinary function; Nocter executable entry uses `program`",
    );
    diagnostic.primary_span = sources.span_to_json(function.name_span).ok().map(Box::new);
    diagnostic.help = Some(
        "replace the entry declaration with `program(): i32 { ... }` or `program(): void { ... }`"
            .to_string(),
    );
    diagnostic
}

fn duplicate_program_diagnostic(
    sources: &SourceMap,
    first_span: ByteSpan,
    second_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0302",
        "executable root file must not define more than one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(second_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first `program` entry is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep exactly one top-level `program` declaration".to_string());
    diagnostic
}

fn invalid_program_return_type_diagnostic(
    sources: &SourceMap,
    return_type_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0303",
        "`program` return type must be `i32` or `void` in v0",
    );
    diagnostic.primary_span = sources.span_to_json(return_type_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `program(): i32` for an exit status or `program(): void` for status 0".to_string(),
    );
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn check_text(text: &str) -> Vec<Diagnostic> {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        check(&sources, &parsed.ast.unwrap())
    }

    #[test]
    fn accepts_program_i32() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_program_void() {
        let diagnostics = check_text(
            r#"program(): void {
    return
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_main_without_program() {
        let diagnostics = check_text(
            r#"func main(): i32 {
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0301");
    }

    #[test]
    fn diagnoses_invalid_program_return_type() {
        let diagnostics = check_text(
            r#"program(): u64 {
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0303");
    }

    #[test]
    fn diagnoses_duplicate_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

program(): void {
    return
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0302");
    }
}

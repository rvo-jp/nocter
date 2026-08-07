mod arrays;
mod bindings;
mod borrow_wrappers;
mod calls;
mod coercions;
mod control_flow;
mod drop_members;
mod entry;
mod fallible;
mod generic_bounds;
mod interfaces;
mod literals;
mod methods;
mod operators;
mod optional;
mod ownership;
mod provenance_contracts;
mod regions;
mod returns;
mod strings;
mod structs;
mod types;
mod variants;

use super::check;
use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::resolve;
use crate::source::SourceMap;

fn check_text(text: &str) -> Vec<Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(&sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ast = parsed.ast.unwrap();
    let resolved = resolve(&sources, &ast);
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(check(&sources, &ast, &resolved));
    diagnostics
}

#[test]
fn duplicate_native_test_names_are_rejected_without_entering_the_callable_namespace() {
    let diagnostics = check_text("test same { return }\ntest same { return }\n");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0400"
            && diagnostic
                .message
                .contains("test `same` is already declared")
    }));
}

mod arrays;
mod bindings;
mod borrow_wrappers;
mod calls;
mod coercions;
mod control_flow;
mod destructors;
mod entry;
mod fallible;
mod format_support;
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
use crate::resolve::{ImportSourceMap, PreludeSourceMap, resolve_compile_unit};
use crate::source::SourceMap;

fn check_text(text: &str) -> Vec<Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let str_source = sources.add_source(
        "std/str/index.nct",
        None,
        "instance str { pub method &self.len(): usize { return 0 } pub method &self.is_empty(): bool { return false } }\n",
    );
    let slice_source = sources.add_source(
        "std/slice/index.nct",
        None,
        "instance [T] { pub method &self.len(): usize { return 0 } pub method &self.is_empty(): bool { return false } }\n",
    );
    let ast = parse_test_source(&sources, source);
    let str_ast = parse_test_source(&sources, str_source);
    let slice_ast = parse_test_source(&sources, slice_source);
    let files = [ast.clone(), str_ast, slice_ast];
    let resolved = resolve_compile_unit(
        &sources,
        &ast,
        &files,
        &ImportSourceMap::new(),
        &PreludeSourceMap::new(),
    );
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(check(&sources, &ast, &resolved));
    diagnostics
}

fn parse_test_source(sources: &SourceMap, source: crate::source::SourceId) -> crate::ast::AstFile {
    let lexed = lex(sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.ast.unwrap()
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

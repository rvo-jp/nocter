use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{ResolveOutput, resolve};
use crate::source::SourceMap;

pub(super) fn resolve_text(text: &str) -> ResolveOutput {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(&sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    resolve(&sources, &parsed.ast.unwrap())
}

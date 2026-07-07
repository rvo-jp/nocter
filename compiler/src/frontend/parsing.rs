use crate::ast::AstFile;
use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse;
use crate::source::{SourceId, SourceMap};

pub(super) fn parse_source_for_check(
    sources: &SourceMap,
    source: SourceId,
) -> Result<AstFile, Vec<Diagnostic>> {
    let lexed = lex(sources, source);
    if !lexed.diagnostics.is_empty() {
        return Err(lexed.diagnostics);
    }

    let parsed = parse(sources, source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics);
    }

    let Some(ast) = parsed.ast else {
        return Err(vec![Diagnostic::error(
            "E0200",
            "parser did not produce an AST and did not report a diagnostic",
        )]);
    };

    Ok(ast)
}

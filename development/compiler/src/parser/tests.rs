mod constructs;
mod expressions;
mod items;
mod literals;
mod statements;
mod support;
mod types;

#[test]
fn rejects_a_token_stream_without_the_eof_contract() {
    let mut sources = crate::source::SourceMap::new();
    let source = sources.add_source("missing-eof.nct", None, "func main".to_string());
    let mut tokens = crate::lexer::lex(&sources, source).tokens;
    assert!(matches!(
        tokens.pop().map(|token| token.kind),
        Some(crate::lexer::TokenKind::Eof)
    ));

    let output = super::parse(&sources, source, &tokens);

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("ending in EOF"));
}

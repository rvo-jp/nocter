use super::super::{ParseOutput, parse};
use crate::ast::JsonAstNode;
use crate::lexer::lex;
use crate::source::SourceMap;

pub(super) fn parse_text(text: &str) -> ParseOutput {
    let (_, output) = parse_text_with_sources(text);
    output
}

pub(super) fn parse_text_with_sources(text: &str) -> (SourceMap, ParseOutput) {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let lexed = lex(&sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let output = parse(&sources, source, &lexed.tokens);
    (sources, output)
}

pub(super) fn find_json_node<'a>(node: &'a JsonAstNode, kind: &str) -> Option<&'a JsonAstNode> {
    if node.kind == kind {
        return Some(node);
    }

    node.items
        .iter()
        .find_map(|child| find_json_node(child, kind))
}

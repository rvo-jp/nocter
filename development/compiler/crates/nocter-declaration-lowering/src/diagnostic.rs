use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{NodeId, SyntaxOrigin, SyntaxTree};

use crate::CompileUnitInput;

pub(crate) fn origin_from_trees<'syntax>(
    trees: impl IntoIterator<Item = &'syntax SyntaxTree>,
    node: NodeId,
) -> Option<SourceOrigin> {
    origin_from_syntax(trees, SyntaxOrigin::Node(node))
}

pub(crate) fn origin_from_syntax<'syntax>(
    trees: impl IntoIterator<Item = &'syntax SyntaxTree>,
    syntax: SyntaxOrigin,
) -> Option<SourceOrigin> {
    let source = match syntax {
        SyntaxOrigin::Node(node) => node.source(),
        SyntaxOrigin::Token(token) => token.source(),
    };
    let tree = trees.into_iter().find(|tree| tree.source() == source)?;
    match syntax {
        SyntaxOrigin::Node(node) => SourceOrigin::from_node(tree, node).ok(),
        SyntaxOrigin::Token(token) => SourceOrigin::from_token(tree, token).ok(),
    }
}

pub(crate) fn input_trees<'input, 'syntax: 'input>(
    input: &'input CompileUnitInput<'syntax>,
) -> impl Iterator<Item = &'syntax SyntaxTree> + 'input {
    input.modules().iter().flat_map(|module| {
        module
            .sources()
            .iter()
            .map(crate::ModuleSourceInput::syntax)
    })
}

pub(crate) fn project_syntax_diagnostic(
    input: &CompileUnitInput<'_>,
    primary: SyntaxOrigin,
    related: impl IntoIterator<Item = SyntaxOrigin>,
    code: DiagnosticCode,
    message: &'static str,
    related_message: Option<&'static str>,
    help: &'static str,
) -> Option<SourceDiagnostic> {
    let primary = origin_from_syntax(input_trees(input), primary)?;
    let related = related.into_iter().collect::<Vec<_>>();
    let related_message = if related.is_empty() {
        ""
    } else {
        related_message?
    };
    let notes = related
        .into_iter()
        .map(|syntax| {
            origin_from_syntax(input_trees(input), syntax)
                .map(|origin| DiagnosticNote::new(related_message, origin))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(SourceDiagnostic::new(
        code,
        message,
        primary,
        notes,
        Some(help),
    ))
}

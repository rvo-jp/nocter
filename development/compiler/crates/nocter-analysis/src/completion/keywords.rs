use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxTree, TokenKind};

use super::{SemanticCompletion, SemanticCompletionKind};
use crate::AnalysisSnapshot;

/// Returns syntax-owned keywords whose grammar context is fixed at the cursor.
///
/// This layer deliberately does not enumerate declarations or infer semantic targets. It supplies
/// the contextual words that cannot enter the declaration/name indexes, including `test` and the
/// intrinsic `copy` requirement.
pub(super) fn completions(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    offset: ByteOffset,
) -> Box<[SemanticCompletion]> {
    let Some(tree) = snapshot
        .syntax_trees()
        .iter()
        .find(|tree| tree.source() == source)
    else {
        return Box::new([]);
    };
    let Some(source_file) = snapshot.sources().get(source) else {
        return Box::new([]);
    };

    if let Some(where_clause) = innermost_node(tree, offset, NodeKind::WhereClause)
        && has_visible_generic_syntax(tree, where_clause)
        && !has_descendant(tree, where_clause, NodeKind::CopyPredicate)
        && current_where_prefix(tree, source_file, where_clause, offset)
            .is_some_and(|prefix| "copy".starts_with(prefix))
    {
        return Box::new([SemanticCompletion::new(
            "copy",
            SemanticCompletionKind::Keyword,
            Some("intrinsic generic copy requirement".into()),
        )]);
    }

    if is_top_level_test_position(tree, source_file, offset) {
        return Box::new([SemanticCompletion::new(
            "test",
            SemanticCompletionKind::Keyword,
            Some("test name { ... }".into()),
        )]);
    }

    Box::new([])
}

fn current_where_prefix<'a>(
    tree: &SyntaxTree,
    source: &'a nocter_source::SourceFile,
    clause: NodeId,
    offset: ByteOffset,
) -> Option<&'a str> {
    let clause_range = tree.node(clause)?.range();
    let mut start = ByteOffset::new(clause_range.start().get().checked_add(5)?);
    if offset < start {
        return None;
    }
    for element in tree.children(clause) {
        let SyntaxElement::Token(token) = element else {
            continue;
        };
        if token.kind() == TokenKind::Punctuation(Punctuation::Comma)
            && token.range().end() <= offset
            && token.range().end() > start
        {
            start = token.range().end();
        }
    }
    let prefix = source.text_at(TextRange::new(start, offset))?.trim();
    prefix
        .chars()
        .all(|character| character == '_' || character.is_ascii_alphanumeric())
        .then_some(prefix)
}

fn has_visible_generic_syntax(tree: &SyntaxTree, where_clause: NodeId) -> bool {
    let Some(where_range) = tree
        .node(where_clause)
        .map(nocter_syntax::SyntaxNode::range)
    else {
        return false;
    };
    tree.nodes().any(|(_, candidate)| {
        declaration_container(candidate.kind())
            && candidate.range().contains_range(where_range)
            && tree.nodes().any(|(_, generic)| {
                matches!(
                    generic.kind(),
                    NodeKind::GenericParameters | NodeKind::PatternArguments
                ) && candidate.range().contains_range(generic.range())
                    && generic.range().end() <= where_range.start()
            })
    })
}

fn is_top_level_test_position(
    tree: &SyntaxTree,
    source: &nocter_source::SourceFile,
    offset: ByteOffset,
) -> bool {
    if tree.nodes().any(|(_, node)| {
        (node.kind() == NodeKind::Block || declaration_container(node.kind()))
            && node.range().contains_cursor(offset)
    }) {
        return false;
    }
    let Ok(end) = usize::try_from(offset.get()) else {
        return false;
    };
    let Some(before) = source.text().get(..end) else {
        return false;
    };
    let prefix = before
        .rsplit_once('\n')
        .map_or(before, |(_, line)| line)
        .trim();
    prefix
        .chars()
        .all(|character| character == '_' || character.is_ascii_alphanumeric())
        && "test".starts_with(prefix)
}

fn innermost_node(tree: &SyntaxTree, offset: ByteOffset, kind: NodeKind) -> Option<NodeId> {
    tree.nodes()
        .filter(|(_, node)| node.kind() == kind && node.range().contains_cursor(offset))
        .min_by_key(|(_, node)| node.range().len())
        .map(|(node, _)| node)
}

fn has_descendant(tree: &SyntaxTree, root: NodeId, kind: NodeKind) -> bool {
    let Some(root_range) = tree.node(root).map(nocter_syntax::SyntaxNode::range) else {
        return false;
    };
    tree.nodes()
        .any(|(_, node)| node.kind() == kind && root_range.contains_range(node.range()))
}

const fn declaration_container(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::PrimitiveDeclaration
            | NodeKind::TypeAliasDeclaration
            | NodeKind::StructDeclaration
            | NodeKind::EnumDeclaration
            | NodeKind::InterfaceDeclaration
            | NodeKind::ConstructDeclaration
            | NodeKind::InstanceDeclaration
            | NodeKind::ConformDeclaration
            | NodeKind::DropDeclaration
            | NodeKind::TestDeclaration
            | NodeKind::InterfaceMethod
            | NodeKind::ConstructionFunction
            | NodeKind::LiteralDeclaration
            | NodeKind::InherentMethod
            | NodeKind::ConformMethod
    )
}

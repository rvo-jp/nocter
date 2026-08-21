use nocter_source::SourceFile;
use nocter_syntax::{Punctuation, SyntaxElement, SyntaxTree, TokenKind};

pub(super) fn same_tree(
    left_source: &SourceFile,
    left: &SyntaxTree,
    right_source: &SourceFile,
    right: &SyntaxTree,
) -> bool {
    let left_nodes = left.nodes().collect::<Vec<_>>();
    let right_nodes = right.nodes().collect::<Vec<_>>();
    if left.root_id().index() != right.root_id().index() || left_nodes.len() != right_nodes.len() {
        return false;
    }
    left_nodes
        .into_iter()
        .zip(right_nodes)
        .all(|((left_id, left_node), (right_id, right_node))| {
            left_id.index() == right_id.index()
                && left_node.kind() == right_node.kind()
                && same_children(
                    left_node.kind(),
                    left_source,
                    left.children(left_id),
                    right_source,
                    right.children(right_id),
                )
        })
}

fn same_children(
    kind: nocter_syntax::NodeKind,
    left_source: &SourceFile,
    left: &[SyntaxElement],
    right_source: &SourceFile,
    right: &[SyntaxElement],
) -> bool {
    let left = normalized_children(kind, left);
    let right = normalized_children(kind, right);
    left.len() == right.len()
        && left
            .into_iter()
            .zip(right)
            .all(|(left, right)| same_element(left_source, left, right_source, right))
}

fn normalized_children(
    kind: nocter_syntax::NodeKind,
    children: &[SyntaxElement],
) -> Vec<&SyntaxElement> {
    let mut children = children
        .iter()
        .filter(|child| {
            !matches!(
                child,
                SyntaxElement::Token(token) if token.kind() == TokenKind::Newline
            )
        })
        .collect::<Vec<_>>();
    if matches!(
        kind,
        nocter_syntax::NodeKind::ClosureCaptures | nocter_syntax::NodeKind::ClosureParameters
    ) && children.last().is_some_and(|child| is_comma(child))
    {
        children.pop();
    } else if children.len() >= 2
        && is_comma(children[children.len() - 2])
        && is_closing_delimiter(children[children.len() - 1])
    {
        children.remove(children.len() - 2);
    }
    children
}

fn is_comma(element: &SyntaxElement) -> bool {
    matches!(
        element,
        SyntaxElement::Token(token)
            if token.kind() == TokenKind::Punctuation(Punctuation::Comma)
    )
}

fn is_closing_delimiter(element: &SyntaxElement) -> bool {
    matches!(
        element,
        SyntaxElement::Token(token)
            if matches!(
                token.kind(),
                TokenKind::Punctuation(
                    Punctuation::RightParen
                        | Punctuation::RightBrace
                        | Punctuation::RightBracket
                        | Punctuation::Greater
                )
            )
    )
}

fn same_element(
    left_source: &SourceFile,
    left: &SyntaxElement,
    right_source: &SourceFile,
    right: &SyntaxElement,
) -> bool {
    match (left, right) {
        (SyntaxElement::Node(left), SyntaxElement::Node(right)) => left.index() == right.index(),
        (SyntaxElement::Token(left), SyntaxElement::Token(right)) => {
            left.kind() == right.kind()
                && left_source.text_at(left.range()) == right_source.text_at(right.range())
        }
        (SyntaxElement::Missing(_), SyntaxElement::Missing(_)) => true,
        _ => false,
    }
}

use std::collections::{HashMap, HashSet};

use nocter_source::{SourceFile, TextRange};
use nocter_syntax::{Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxTree, TokenKind};

/// Locally proven token omissions for specification-owned redundant grouping rules.
pub(super) struct RewritePlan {
    omitted_tokens: HashSet<usize>,
    omitted_ranges: Vec<TextRange>,
}

impl RewritePlan {
    pub(super) fn build(syntax: &SyntaxTree) -> Self {
        let parents = parent_nodes(syntax);
        let mut omitted_tokens = HashSet::new();
        for (node_id, node) in syntax.nodes() {
            match node.kind() {
                NodeKind::GroupedExpression => {
                    let Some((left, expression, right)) = grouped_parts(syntax, node_id) else {
                        continue;
                    };
                    let removable =
                        is_negative_integer_group(syntax, &parents, node_id, expression)
                            || is_move_outcome_group(syntax, expression, right.lexical().index());
                    if removable {
                        omitted_tokens.insert(left.lexical().index());
                        omitted_tokens.insert(right.lexical().index());
                    }
                }
                NodeKind::GroupedType => {
                    let Some((left, inner, right)) = grouped_parts(syntax, node_id) else {
                        continue;
                    };
                    if is_optional_borrow_group(syntax, inner, right.lexical().index()) {
                        omitted_tokens.insert(left.lexical().index());
                        omitted_tokens.insert(right.lexical().index());
                    }
                }
                _ => {}
            }
        }
        let mut omitted_ranges = omitted_tokens
            .iter()
            .map(|index| syntax.lexed().tokens()[*index].span().range())
            .collect::<Vec<_>>();
        omitted_ranges.sort_by_key(|range| range.start());
        Self {
            omitted_tokens,
            omitted_ranges,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.omitted_ranges.is_empty()
    }

    pub(super) fn apply(&self, source: &SourceFile) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut output = String::with_capacity(source.text().len());
        let mut copied = 0;
        for range in &self.omitted_ranges {
            let start = usize::try_from(range.start().get()).ok()?;
            let end = usize::try_from(range.end().get()).ok()?;
            output.push_str(source.text().get(copied..start)?);
            copied = end;
        }
        output.push_str(source.text().get(copied..)?);
        Some(output)
    }

    /// Proves that applying the plan changed only the exact grouping tokens selected above.
    pub(super) fn preserves_tokens(
        &self,
        original_source: &SourceFile,
        original: &SyntaxTree,
        rewritten_source: &SourceFile,
        rewritten: &SyntaxTree,
    ) -> bool {
        let original = original
            .lexed()
            .tokens()
            .iter()
            .enumerate()
            .filter(|(index, token)| {
                token.kind() != TokenKind::Newline
                    && token.kind() != TokenKind::Eof
                    && !self.omitted_tokens.contains(index)
            })
            .map(|(_, token)| (token.kind(), original_source.text_at(token.span().range())));
        let rewritten = rewritten
            .lexed()
            .tokens()
            .iter()
            .filter(|token| token.kind() != TokenKind::Newline && token.kind() != TokenKind::Eof)
            .map(|token| (token.kind(), rewritten_source.text_at(token.span().range())));
        original.eq(rewritten)
    }
}

fn parent_nodes(syntax: &SyntaxTree) -> HashMap<NodeId, NodeId> {
    let mut parents = HashMap::new();
    for (parent, _) in syntax.nodes() {
        for child in syntax.children(parent) {
            if let SyntaxElement::Node(child) = child {
                parents.insert(*child, parent);
            }
        }
    }
    parents
}

fn grouped_parts(
    syntax: &SyntaxTree,
    node: NodeId,
) -> Option<(
    nocter_syntax::SyntaxToken,
    NodeId,
    nocter_syntax::SyntaxToken,
)> {
    let children = significant_children(syntax.children(node));
    let [
        SyntaxElement::Token(left),
        SyntaxElement::Node(expression),
        SyntaxElement::Token(right),
    ] = children.as_slice()
    else {
        return None;
    };
    (left.kind() == TokenKind::Punctuation(Punctuation::LeftParen)
        && right.kind() == TokenKind::Punctuation(Punctuation::RightParen))
    .then_some((*left, *expression, *right))
}

fn is_negative_integer_group(
    syntax: &SyntaxTree,
    parents: &HashMap<NodeId, NodeId>,
    group: NodeId,
    expression: NodeId,
) -> bool {
    let Some(parent) = parents.get(&group).copied() else {
        return false;
    };
    let Some(parent_node) = syntax.node(parent) else {
        return false;
    };
    let parent_children = significant_children(syntax.children(parent));
    if parent_node.kind() != NodeKind::UnaryExpression
        || !matches!(
            parent_children.as_slice(),
            [SyntaxElement::Token(operator), SyntaxElement::Node(operand)]
                if operator.kind() == TokenKind::Punctuation(Punctuation::Minus)
                    && *operand == group
        )
    {
        return false;
    }
    node_wraps_exact_kind(syntax, expression, NodeKind::ScalarLiteral).is_some_and(|literal| {
        matches!(
            significant_children(syntax.children(literal)).as_slice(),
            [SyntaxElement::Token(token)] if token.kind() == TokenKind::IntegerLiteral
        )
    })
}

fn is_move_outcome_group(
    syntax: &SyntaxTree,
    expression: NodeId,
    right_parenthesis: usize,
) -> bool {
    let Some(move_expression) = node_wraps_exact_kind(syntax, expression, NodeKind::MoveExpression)
    else {
        return false;
    };
    if !matches!(
        significant_children(syntax.children(move_expression)).as_slice(),
        [SyntaxElement::Token(keyword), SyntaxElement::Node(_)]
            if keyword.kind() == TokenKind::Keyword(Keyword::Move)
    ) {
        return false;
    }
    syntax.lexed().tokens()[right_parenthesis + 1..]
        .iter()
        .find(|token| token.kind() != TokenKind::Newline)
        .is_some_and(|token| {
            matches!(
                token.kind(),
                TokenKind::Punctuation(Punctuation::Question | Punctuation::Bang)
                    | TokenKind::Keyword(Keyword::Catch | Keyword::Otherwise)
            )
        })
}

fn is_optional_borrow_group(syntax: &SyntaxTree, inner: NodeId, right_parenthesis: usize) -> bool {
    if node_wraps_exact_kind(syntax, inner, NodeKind::BorrowType).is_none() {
        return false;
    }
    syntax.lexed().tokens()[right_parenthesis + 1..]
        .iter()
        .find(|token| token.kind() != TokenKind::Newline)
        .is_some_and(|token| token.kind() == TokenKind::Punctuation(Punctuation::Question))
}

fn node_wraps_exact_kind(
    syntax: &SyntaxTree,
    mut node: NodeId,
    expected: NodeKind,
) -> Option<NodeId> {
    loop {
        let current = syntax.node(node)?;
        if current.kind() == expected {
            return Some(node);
        }
        if !matches!(current.kind(), NodeKind::Expression | NodeKind::Type) {
            return None;
        }
        let [SyntaxElement::Node(child)] = significant_children(syntax.children(node)).as_slice()
        else {
            return None;
        };
        node = *child;
    }
}

fn significant_children(children: &[SyntaxElement]) -> Vec<&SyntaxElement> {
    children
        .iter()
        .filter(|child| {
            !matches!(
                child,
                SyntaxElement::Token(token) if token.kind() == TokenKind::Newline
            )
        })
        .collect()
}

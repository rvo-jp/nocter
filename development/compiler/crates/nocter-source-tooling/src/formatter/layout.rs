use std::collections::{HashMap, HashSet};

use nocter_source::SourceFile;
use nocter_syntax::{NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind};

/// Structure-sensitive edits selected from the original concrete syntax before text emission.
pub(super) struct LayoutPlan {
    omitted_tokens: HashSet<SyntaxToken>,
    comma_before: HashSet<SyntaxToken>,
    line_break_before: HashSet<SyntaxToken>,
    join_before: HashSet<SyntaxToken>,
    structural_opens: HashMap<SyntaxToken, usize>,
    structural_closes: HashMap<SyntaxToken, usize>,
}

impl LayoutPlan {
    pub(super) fn build(source: &SourceFile, syntax: &SyntaxTree, tokens: &[SyntaxToken]) -> Self {
        let mut plan = Self {
            omitted_tokens: HashSet::new(),
            comma_before: HashSet::new(),
            line_break_before: HashSet::new(),
            join_before: HashSet::new(),
            structural_opens: HashMap::new(),
            structural_closes: HashMap::new(),
        };
        let token_at_offset = tokens
            .iter()
            .copied()
            .map(|token| (token.range().start().get(), token))
            .collect::<HashMap<_, _>>();
        for (node_id, node) in syntax.nodes() {
            if node.kind() == NodeKind::WhereClause {
                plan_where_clause(
                    &mut plan,
                    syntax,
                    &token_at_offset,
                    syntax.children(node_id),
                    node.range().start().get(),
                );
                continue;
            }
            if matches!(
                node.kind(),
                NodeKind::ClosureCaptures | NodeKind::ClosureParameters
            ) {
                plan_single_line_segment(
                    &mut plan,
                    source,
                    syntax,
                    &token_at_offset,
                    syntax.children(node_id),
                    node.range(),
                );
                continue;
            }
            if !is_comma_list(node.kind()) {
                continue;
            }
            let children = syntax.children(node_id);
            let Some((opening, closing)) = delimiter_positions(children) else {
                continue;
            };
            if is_angle(&children[opening], Punctuation::Less) {
                let opening_token = syntax_token(&children[opening])
                    .expect("a generic-list opening delimiter is a syntax token");
                let closing_token = syntax_token(&children[closing])
                    .expect("a generic-list closing delimiter is a syntax token");
                *plan.structural_opens.entry(opening_token).or_insert(0) += 1;
                *plan.structural_closes.entry(closing_token).or_insert(0) += 1;
            }
            let multiline = source
                .text_at(node.range())
                .is_some_and(|text| text.contains('\n'));
            let comma_positions = children[opening + 1..closing]
                .iter()
                .enumerate()
                .filter_map(|(offset, child)| is_comma(child).then_some(opening + 1 + offset))
                .collect::<Vec<_>>();
            let trailing_comma = comma_positions
                .last()
                .copied()
                .filter(|position| children[*position + 1..closing].iter().all(is_newline));
            if multiline {
                if let Some(first) =
                    next_lexical_token(syntax, &token_at_offset, children, opening + 1, closing)
                {
                    plan.line_break_before.insert(first);
                }
                for comma in &comma_positions {
                    if let Some(next) =
                        next_lexical_token(syntax, &token_at_offset, children, comma + 1, closing)
                    {
                        plan.line_break_before.insert(next);
                    }
                }
                let closing_token = syntax_token(&children[closing])
                    .expect("a formatter list closing delimiter is a syntax token");
                if trailing_comma.is_none()
                    && next_lexical_token(syntax, &token_at_offset, children, opening + 1, closing)
                        .is_some()
                {
                    plan.comma_before.insert(closing_token);
                }
                plan.line_break_before.insert(closing_token);
            } else if let Some(comma) = trailing_comma {
                let token = syntax_token(&children[comma])
                    .expect("a formatter list comma is a syntax token");
                plan.omitted_tokens.insert(token);
            }
        }
        plan
    }

    pub(super) fn omits(&self, token: SyntaxToken) -> bool {
        self.omitted_tokens.contains(&token)
    }

    pub(super) fn inserts_comma_before(&self, token: SyntaxToken) -> bool {
        self.comma_before.contains(&token)
    }

    pub(super) fn breaks_before(&self, token: SyntaxToken) -> bool {
        self.line_break_before.contains(&token)
    }

    pub(super) fn joins_before(&self, token: SyntaxToken) -> bool {
        self.join_before.contains(&token)
    }

    pub(super) fn structural_opens(&self, token: SyntaxToken) -> usize {
        self.structural_opens.get(&token).copied().unwrap_or(0)
    }

    pub(super) fn structural_closes(&self, token: SyntaxToken) -> usize {
        self.structural_closes.get(&token).copied().unwrap_or(0)
    }
}

fn plan_where_clause(
    plan: &mut LayoutPlan,
    syntax: &SyntaxTree,
    token_at_offset: &HashMap<u32, SyntaxToken>,
    children: &[SyntaxElement],
    start: u32,
) {
    if let Some(first) = token_at_offset.get(&start).copied() {
        plan.join_before.insert(first);
    }
    for (position, child) in children.iter().enumerate() {
        if is_comma(child)
            && let Some(next) = next_lexical_token(
                syntax,
                token_at_offset,
                children,
                position + 1,
                children.len(),
            )
        {
            plan.join_before.insert(next);
        }
    }
}

fn plan_single_line_segment(
    plan: &mut LayoutPlan,
    source: &SourceFile,
    syntax: &SyntaxTree,
    token_at_offset: &HashMap<u32, SyntaxToken>,
    children: &[SyntaxElement],
    range: nocter_source::TextRange,
) {
    if source
        .text_at(range)
        .is_some_and(|text| text.contains('\n'))
    {
        return;
    }
    let Some(last) = children.iter().rposition(|child| !is_newline(child)) else {
        return;
    };
    if is_comma(&children[last])
        && let Some(token) = first_token_index(syntax, token_at_offset, &children[last])
    {
        plan.omitted_tokens.insert(token);
    }
}

fn delimiter_positions(children: &[SyntaxElement]) -> Option<(usize, usize)> {
    let opening = children.iter().position(is_opening_delimiter)?;
    let closing = children.iter().rposition(is_closing_delimiter)?;
    (opening < closing).then_some((opening, closing))
}

fn next_lexical_token(
    syntax: &SyntaxTree,
    token_at_offset: &HashMap<u32, SyntaxToken>,
    children: &[SyntaxElement],
    start: usize,
    end: usize,
) -> Option<SyntaxToken> {
    children[start..end]
        .iter()
        .find(|child| !is_newline(child))
        .and_then(|child| first_token_index(syntax, token_at_offset, child))
}

fn first_token_index(
    syntax: &SyntaxTree,
    token_at_offset: &HashMap<u32, SyntaxToken>,
    element: &SyntaxElement,
) -> Option<SyntaxToken> {
    match element {
        SyntaxElement::Token(token) => Some(*token),
        SyntaxElement::Node(node) => token_at_offset
            .get(&syntax.node(*node)?.range().start().get())
            .copied(),
        SyntaxElement::Missing(_) => None,
    }
}

fn syntax_token(element: &SyntaxElement) -> Option<SyntaxToken> {
    let SyntaxElement::Token(token) = element else {
        return None;
    };
    Some(*token)
}

fn is_newline(element: &SyntaxElement) -> bool {
    matches!(element, SyntaxElement::Token(token) if token.kind() == TokenKind::Newline)
}

fn is_comma(element: &SyntaxElement) -> bool {
    matches!(
        element,
        SyntaxElement::Token(token)
            if token.kind() == TokenKind::Punctuation(Punctuation::Comma)
    )
}

fn is_opening_delimiter(element: &SyntaxElement) -> bool {
    matches!(
        element,
        SyntaxElement::Token(token)
            if matches!(
                token.kind(),
                TokenKind::Punctuation(
                    Punctuation::LeftParen
                        | Punctuation::LeftBrace
                        | Punctuation::LeftBracket
                        | Punctuation::Less
                )
            )
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

fn is_angle(element: &SyntaxElement, punctuation: Punctuation) -> bool {
    matches!(
        element,
        SyntaxElement::Token(token)
            if token.kind() == TokenKind::Punctuation(punctuation)
    )
}

const fn is_comma_list(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::DirectiveRecord
            | NodeKind::ImportSelection
            | NodeKind::GenericParameters
            | NodeKind::Parameters
            | NodeKind::CallableParameters
            | NodeKind::TypeArguments
            | NodeKind::PatternArguments
            | NodeKind::OpaqueArguments
            | NodeKind::CallSuffix
            | NodeKind::StructInitializer
            | NodeKind::ArrayLiteral
            | NodeKind::SequenceBody
            | NodeKind::EnumPatternPayload
    )
}

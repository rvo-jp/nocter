use nocter_source::{ByteOffset, Span, TextRange};

use super::super::{CompletedMarker, Parser, newline};
use crate::{
    ExpectedSyntax, Keyword, NodeKind, ParseDiagnostic, ParseDiagnosticKind, Punctuation, TokenKind,
};

pub(super) struct Expression {
    pub(super) completed: CompletedMarker,
    // Only outer grouping propagates this structural restriction. Computation replaces it.
    direct_access: Option<Span>,
}

pub(super) fn expression(parser: &mut Parser<'_>, boundary: newline::Boundary) -> Expression {
    let marker = parser.start();
    let inner = binary(parser, boundary, 0);
    Expression {
        completed: parser.complete(marker, NodeKind::Expression),
        direct_access: inner.direct_access,
    }
}

const OPERATORS: &[(&[Punctuation], NodeKind)] = &[
    (&[Punctuation::LogicalOr], NodeKind::LogicalOrExpression),
    (&[Punctuation::LogicalAnd], NodeKind::LogicalAndExpression),
    (
        &[Punctuation::EqualEqual, Punctuation::BangEqual],
        NodeKind::EqualityExpression,
    ),
    (
        &[
            Punctuation::Less,
            Punctuation::LessEqual,
            Punctuation::Greater,
            Punctuation::GreaterEqual,
        ],
        NodeKind::OrderingExpression,
    ),
    (
        &[Punctuation::Plus, Punctuation::Minus],
        NodeKind::AdditiveExpression,
    ),
    (
        &[Punctuation::Star, Punctuation::Slash, Punctuation::Percent],
        NodeKind::MultiplicativeExpression,
    ),
];

fn binary(parser: &mut Parser<'_>, boundary: newline::Boundary, level: usize) -> Expression {
    let Some((operators, kind)) = OPERATORS.get(level) else {
        return prefix(parser, boundary);
    };
    let mut left = binary(parser, boundary, level + 1);
    loop {
        newline::before(parser, boundary, |token| {
            operators.iter().any(|operator| {
                *operator != Punctuation::Minus && token == TokenKind::Punctuation(*operator)
            })
        });
        if !operators
            .iter()
            .any(|operator| parser.at_punctuation(*operator))
        {
            return left;
        }
        let marker = parser.precede(left.completed);
        parser.bump();
        newline::after_incomplete(parser, boundary);
        binary(parser, boundary, level + 1);
        left = Expression {
            completed: parser.complete(marker, *kind),
            direct_access: None,
        };
        if matches!(
            kind,
            NodeKind::EqualityExpression | NodeKind::OrderingExpression
        ) {
            return left;
        }
    }
}

fn prefix(parser: &mut Parser<'_>, boundary: newline::Boundary) -> Expression {
    let mut wrappers = Vec::new();
    loop {
        let span = parser.current_span();
        let kind = match parser.current_kind() {
            TokenKind::Punctuation(Punctuation::LogicalAnd) => {
                let marker = parser.start();
                parser.split_current(
                    TokenKind::Punctuation(Punctuation::Ampersand),
                    TokenKind::Punctuation(Punctuation::Ampersand),
                );
                let first_prefix = Span::new(
                    span.source(),
                    TextRange::new(
                        span.range().start(),
                        ByteOffset::new(span.range().start().get() + 1),
                    ),
                );
                wrappers.push((marker, NodeKind::UpdateExpression, Some(first_prefix)));
                continue;
            }
            TokenKind::Punctuation(Punctuation::Ampersand) => NodeKind::UpdateExpression,
            TokenKind::Punctuation(Punctuation::Caret) => NodeKind::TransferExpression,
            TokenKind::Punctuation(Punctuation::Bang | Punctuation::Minus) => {
                NodeKind::UnaryExpression
            }
            _ => break,
        };
        let marker = parser.start();
        parser.bump();
        newline::after_incomplete(parser, boundary);
        let direct_access = matches!(
            kind,
            NodeKind::UpdateExpression | NodeKind::TransferExpression
        )
        .then_some(span);
        wrappers.push((marker, kind, direct_access));
    }
    let mut inner = postfix(parser);
    for (marker, kind, direct_access) in wrappers.into_iter().rev() {
        inner = Expression {
            completed: parser.complete(marker, kind),
            direct_access,
        };
    }
    inner
}

pub(super) fn postfix(parser: &mut Parser<'_>) -> Expression {
    let mut left = primary(parser);
    while parser.at_punctuation(Punctuation::LeftParen) {
        check_call_operand(parser, &left);
        let marker = parser.precede(left.completed);
        let suffix = parser.start();
        parser.bump();
        if parser.enter_nesting() {
            parser.comma_list(
                Punctuation::RightParen,
                true,
                ExpectedSyntax::Expression,
                argument,
            );
            parser.expect_punctuation(Punctuation::RightParen);
            parser.leave_nesting();
        } else {
            parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        }
        parser.complete(suffix, NodeKind::CallSuffix);
        left = Expression {
            completed: parser.complete(marker, NodeKind::PostfixExpression),
            direct_access: None,
        };
    }
    left
}

fn argument(parser: &mut Parser<'_>) {
    let operand = expression(parser, newline::Boundary::Delimited);
    check_call_operand(parser, &operand);
}

fn check_call_operand(parser: &mut Parser<'_>, operand: &Expression) {
    if let Some(span) = operand.direct_access {
        parser.diagnostics.push(ParseDiagnostic::new(
            ParseDiagnosticKind::Expected(ExpectedSyntax::UnmarkedCallOperand),
            span,
        ));
    }
}

fn primary(parser: &mut Parser<'_>) -> Expression {
    if parser.at_punctuation(Punctuation::LeftParen) {
        return grouped(parser);
    }
    if parser.at_keyword(Keyword::If) {
        return if_(parser);
    }
    let marker = parser.start();
    let kind = match parser.current_kind() {
        TokenKind::Identifier => {
            parser.expect_name();
            NodeKind::ReferenceExpression
        }
        TokenKind::IntegerLiteral | TokenKind::Keyword(Keyword::True | Keyword::False) => {
            parser.bump();
            NodeKind::ScalarLiteral
        }
        _ => {
            // Leave statement/delimiter boundaries to their owner, including a missing operand.
            if matches!(
                parser.current_kind(),
                TokenKind::Eof
                    | TokenKind::Newline
                    | TokenKind::Punctuation(
                        Punctuation::RightParen | Punctuation::RightBrace | Punctuation::Comma
                    )
            ) {
                parser.missing(ExpectedSyntax::Expression);
            } else {
                parser.error_token(ExpectedSyntax::Expression);
            }
            NodeKind::Error
        }
    };
    Expression {
        completed: parser.complete(marker, kind),
        direct_access: None,
    }
}

fn grouped(parser: &mut Parser<'_>) -> Expression {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        return Expression {
            completed: parser.complete(marker, NodeKind::GroupedExpression),
            direct_access: None,
        };
    }
    parser.eat_newlines();
    if parser.eat_punctuation(Punctuation::RightParen) {
        parser.leave_nesting();
        return Expression {
            completed: parser.complete(marker, NodeKind::UnitExpression),
            direct_access: None,
        };
    }
    let inner = expression(parser, newline::Boundary::Delimited);
    parser.eat_newlines();
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    Expression {
        completed: parser.complete(marker, NodeKind::GroupedExpression),
        direct_access: inner.direct_access,
    }
}

fn if_(parser: &mut Parser<'_>) -> Expression {
    let marker = parser.start();
    if !parser.enter_nesting() {
        parser.bump();
        return Expression {
            completed: parser.complete(marker, NodeKind::Error),
            direct_access: None,
        };
    }
    parser.bump();
    let condition = parser.start();
    expression(parser, newline::Boundary::Statement);
    parser.complete(condition, NodeKind::IfCondition);
    super::required_block(parser);
    if parser.at_keyword(Keyword::Else) {
        let alternative = parser.start();
        parser.bump();
        if parser.at_keyword(Keyword::If) {
            if_(parser);
        } else {
            super::required_block(parser);
        }
        parser.complete(alternative, NodeKind::ElseClause);
    }
    parser.leave_nesting();
    Expression {
        completed: parser.complete(marker, NodeKind::IfExpression),
        direct_access: None,
    }
}

use super::super::{CompletedMarker, Parser, newline};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) struct Expression {
    pub(super) completed: CompletedMarker,
}

pub(super) fn expression(parser: &mut Parser<'_>, boundary: newline::Boundary) -> Expression {
    parse_expression(parser, boundary, true)
}

pub(super) fn header_expression(parser: &mut Parser<'_>) -> Expression {
    parse_expression(parser, newline::Boundary::Statement, false)
}

fn parse_expression(
    parser: &mut Parser<'_>,
    boundary: newline::Boundary,
    allow_braces: bool,
) -> Expression {
    let marker = parser.start();
    binary(parser, boundary, 0, allow_braces);
    Expression {
        completed: parser.complete(marker, NodeKind::Expression),
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

fn binary(
    parser: &mut Parser<'_>,
    boundary: newline::Boundary,
    level: usize,
    allow_braces: bool,
) -> Expression {
    let Some((operators, kind)) = OPERATORS.get(level) else {
        return prefix(parser, boundary, allow_braces);
    };
    let mut left = binary(parser, boundary, level + 1, allow_braces);
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
        binary(parser, boundary, level + 1, allow_braces);
        left = Expression {
            completed: parser.complete(marker, *kind),
        };
        if matches!(
            kind,
            NodeKind::EqualityExpression | NodeKind::OrderingExpression
        ) {
            return left;
        }
    }
}

fn prefix(parser: &mut Parser<'_>, boundary: newline::Boundary, allow_braces: bool) -> Expression {
    let mut wrappers = Vec::new();
    while matches!(
        parser.current_kind(),
        TokenKind::Punctuation(Punctuation::Bang | Punctuation::Minus)
    ) {
        let marker = parser.start();
        parser.bump();
        newline::after_incomplete(parser, boundary);
        wrappers.push(marker);
    }
    let mut inner = postfix_in(parser, allow_braces);
    for marker in wrappers.into_iter().rev() {
        inner = Expression {
            completed: parser.complete(marker, NodeKind::UnaryExpression),
        };
    }
    inner
}

pub(super) fn postfix(parser: &mut Parser<'_>) -> Expression {
    postfix_in(parser, true)
}

fn postfix_in(parser: &mut Parser<'_>, allow_braces: bool) -> Expression {
    let mut left = primary(parser, allow_braces);
    while parser.at_punctuation(Punctuation::LeftParen) || parser.at_punctuation(Punctuation::Dot) {
        let marker = parser.precede(left.completed);
        let suffix = parser.start();
        if parser.eat_punctuation(Punctuation::Dot) {
            parser.expect_name();
            parser.complete(suffix, NodeKind::MemberSuffix);
        } else {
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
        }
        left = Expression {
            completed: parser.complete(marker, NodeKind::PostfixExpression),
        };
    }
    left
}

fn argument(parser: &mut Parser<'_>) {
    expression(parser, newline::Boundary::Delimited);
}

fn primary(parser: &mut Parser<'_>, allow_braces: bool) -> Expression {
    if parser.at_punctuation(Punctuation::LeftParen) {
        return grouped(parser);
    }
    if allow_braces && parser.at_keyword(Keyword::If) {
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
    let completed = parser.complete(marker, kind);
    if kind == NodeKind::ReferenceExpression
        && allow_braces
        && parser.at_punctuation(Punctuation::LeftBrace)
    {
        record_value(parser, completed)
    } else {
        Expression { completed }
    }
}

fn record_value(parser: &mut Parser<'_>, owner: CompletedMarker) -> Expression {
    let marker = parser.precede(owner);
    let initializer = parser.start();
    parser.bump();
    if parser.enter_nesting() {
        parser.comma_list(
            Punctuation::RightBrace,
            true,
            ExpectedSyntax::Expression,
            field,
        );
        parser.expect_punctuation(Punctuation::RightBrace);
        parser.leave_nesting();
    } else {
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
    }
    parser.complete(initializer, NodeKind::StructInitializer);
    Expression {
        completed: parser.complete(marker, NodeKind::StructLiteral),
    }
}

fn field(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    newline::after_incomplete(parser, newline::Boundary::Delimited);
    expression(parser, newline::Boundary::Delimited);
    parser.complete(marker, NodeKind::FieldInitializer);
}

fn grouped(parser: &mut Parser<'_>) -> Expression {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        return Expression {
            completed: parser.complete(marker, NodeKind::GroupedExpression),
        };
    }
    parser.eat_newlines();
    if parser.eat_punctuation(Punctuation::RightParen) {
        parser.leave_nesting();
        return Expression {
            completed: parser.complete(marker, NodeKind::UnitExpression),
        };
    }
    expression(parser, newline::Boundary::Delimited);
    parser.eat_newlines();
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    Expression {
        completed: parser.complete(marker, NodeKind::GroupedExpression),
    }
}

fn if_(parser: &mut Parser<'_>) -> Expression {
    let marker = parser.start();
    if !parser.enter_nesting() {
        parser.bump();
        return Expression {
            completed: parser.complete(marker, NodeKind::Error),
        };
    }
    parser.bump();
    let condition = parser.start();
    header_expression(parser);
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
    }
}

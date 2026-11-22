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
    let mut left = binary(parser, boundary, 0, allow_braces);
    loop {
        newline::before(parser, boundary, |kind| {
            matches!(
                kind,
                TokenKind::Keyword(Keyword::Otherwise | Keyword::Catch)
            )
        });
        if !matches!(
            parser.current_kind(),
            TokenKind::Keyword(Keyword::Otherwise | Keyword::Catch)
        ) {
            break;
        }
        let parent = parser.precede(left.completed);
        let clause = parser.start();
        let catch = parser.at_keyword(Keyword::Catch);
        parser.bump();
        newline::after_incomplete(parser, boundary);
        if catch {
            parser.expect_name_or_discard();
            newline::after_incomplete(parser, boundary);
        }
        super::required_block(parser);
        parser.complete(clause, NodeKind::RecoveryClause);
        left = Expression {
            completed: parser.complete(parent, NodeKind::RecoveryExpression),
        };
    }
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
    let mut awaits = Vec::new();
    while parser.at_keyword(Keyword::Await) {
        let marker = parser.start();
        parser.bump();
        newline::after_incomplete(parser, boundary);
        awaits.push(marker);
    }
    let mut inner = postfix_in(parser, allow_braces);
    for marker in awaits.into_iter().rev() {
        inner = Expression {
            completed: parser.complete(marker, NodeKind::AwaitExpression),
        };
    }
    if matches!(
        parser.current_kind(),
        TokenKind::Punctuation(Punctuation::Bang | Punctuation::Question)
    ) {
        let marker = parser.precede(inner.completed);
        parser.bump();
        inner = Expression {
            completed: parser.complete(marker, NodeKind::OutcomeExpression),
        };
    }
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
    let mut name_path = parser.at(TokenKind::Identifier);
    let mut left = primary(parser, allow_braces);
    while (allow_braces && name_path && parser.at_punctuation(Punctuation::LeftBrace))
        || parser.at_punctuation(Punctuation::LeftParen)
        || parser.at_punctuation(Punctuation::Dot)
        || ((parser.at_punctuation(Punctuation::Caret)
            || parser.at_punctuation(Punctuation::LeftBracket))
            && previous_is_joint(parser))
    {
        if allow_braces && name_path && parser.at_punctuation(Punctuation::LeftBrace) {
            left = record_value(parser, left.completed);
            name_path = false;
            continue;
        }
        let marker = parser.precede(left.completed);
        if parser.eat_punctuation(Punctuation::Caret) {
            name_path = false;
            left = Expression {
                completed: parser.complete(marker, NodeKind::TransferExpression),
            };
            continue;
        }
        let suffix = parser.start();
        if parser.eat_punctuation(Punctuation::Dot) {
            name_path &= parser.at(TokenKind::Identifier);
            if parser.at(TokenKind::IntegerLiteral) {
                parser.bump();
            } else {
                parser.expect_name();
            }
            parser.complete(suffix, NodeKind::MemberSuffix);
        } else if parser.eat_punctuation(Punctuation::LeftBracket) {
            name_path = false;
            if parser.enter_nesting() {
                parser.eat_newlines();
                expression(parser, newline::Boundary::Delimited);
                parser.eat_newlines();
                parser.expect_punctuation(Punctuation::RightBracket);
                parser.leave_nesting();
            } else {
                parser.recover_balanced(Punctuation::LeftBracket, Punctuation::RightBracket);
            }
            parser.complete(suffix, NodeKind::IndexSuffix);
        } else {
            name_path = false;
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
    if parser.at_punctuation(Punctuation::LeftBracket) {
        return array(parser);
    }
    if parser.at_punctuation(Punctuation::LeftParen) {
        if allow_braces
            && let Some((completed, _)) =
                parser.attempt_decided_with(closure, |_, (_, decided)| *decided)
        {
            return Expression { completed };
        }
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
        TokenKind::IntegerLiteral
        | TokenKind::Keyword(Keyword::True | Keyword::False | Keyword::None) => {
            parser.bump();
            NodeKind::ScalarLiteral
        }
        TokenKind::StringStart(_) => {
            super::super::root::string_literal(parser);
            NodeKind::StringExpression
        }
        _ => {
            // Leave statement/delimiter boundaries to their owner, including a missing operand.
            if matches!(
                parser.current_kind(),
                TokenKind::Eof
                    | TokenKind::Newline
                    | TokenKind::Punctuation(
                        Punctuation::RightParen
                            | Punctuation::RightBrace
                            | Punctuation::RightBracket
                            | Punctuation::Comma
                            | Punctuation::Semicolon
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
    Expression { completed }
}

fn array(parser: &mut Parser<'_>) -> Expression {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBracket, Punctuation::RightBracket);
        return Expression {
            completed: parser.complete(marker, NodeKind::ArrayLiteral),
        };
    }
    parser.eat_newlines();
    let mut kind = NodeKind::ArrayLiteral;
    if !parser.at_punctuation(Punctuation::RightBracket) {
        expression(parser, newline::Boundary::Delimited);
        parser.eat_newlines();
        if parser.eat_punctuation(Punctuation::Semicolon) {
            kind = NodeKind::ArrayRepeatLiteral;
            parser.eat_newlines();
            expression(parser, newline::Boundary::Delimited);
            parser.eat_newlines();
        } else {
            while parser.eat_punctuation(Punctuation::Comma) {
                parser.eat_newlines();
                if parser.at_punctuation(Punctuation::RightBracket) || parser.at(TokenKind::Eof) {
                    break;
                }
                expression(parser, newline::Boundary::Delimited);
                parser.eat_newlines();
            }
        }
    }
    parser.expect_punctuation(Punctuation::RightBracket);
    parser.leave_nesting();
    Expression {
        completed: parser.complete(marker, kind),
    }
}

fn closure(parser: &mut Parser<'_>) -> (CompletedMarker, bool) {
    let marker = parser.start();
    let head = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        return (parser.complete(head, NodeKind::Error), false);
    }
    parser.eat_newlines();
    parser.attempt(|parser| {
        let captures = parser.start();
        loop {
            let capture = parser.start();
            parser.expect_name();
            if parser.at_punctuation(Punctuation::Caret) && previous_is_joint(parser) {
                parser.bump();
            }
            parser.complete(capture, NodeKind::ClosureCapture);
            parser.eat_newlines();
            if !parser.eat_punctuation(Punctuation::Comma) {
                break;
            }
            parser.eat_newlines();
            if parser.at_punctuation(Punctuation::Semicolon) {
                break;
            }
        }
        parser.expect_punctuation(Punctuation::Semicolon);
        parser.complete(captures, NodeKind::ClosureCaptures);
    });
    let parameters = parser.start();
    parser.comma_list(
        Punctuation::RightParen,
        true,
        ExpectedSyntax::Parameter,
        |parser| {
            let parameter = parser.start();
            if (parser.at_contextual(crate::ContextualSpelling::Edit)
                || parser.at_contextual(crate::ContextualSpelling::Take))
                && parser.nth_kind(1) == TokenKind::Identifier
            {
                let operation = parser.start();
                parser.bump();
                parser.complete(operation, NodeKind::InputOperation);
            }
            parser.expect_name();
            if parser.eat_punctuation(Punctuation::Colon) {
                super::type_(parser);
            }
            parser.complete(parameter, NodeKind::ClosureParameter);
        },
    );
    parser.expect_punctuation(Punctuation::RightParen);
    parser.complete(parameters, NodeKind::ClosureParameters);
    parser.complete(head, NodeKind::ClosureHead);
    if parser.eat_punctuation(Punctuation::Colon) {
        let result = parser.start();
        super::type_(parser);
        parser.complete(result, NodeKind::ClosureResult);
    }
    let decided = parser.at_punctuation(Punctuation::LeftBrace);
    if decided {
        super::required_block(parser);
    }
    parser.leave_nesting();
    (
        parser.complete(marker, NodeKind::ClosureExpression),
        decided,
    )
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
    let tuple = parser.eat_punctuation(Punctuation::Comma);
    if tuple {
        parser.eat_newlines();
        expression(parser, newline::Boundary::Delimited);
        parser.eat_newlines();
        while parser.eat_punctuation(Punctuation::Comma) {
            parser.eat_newlines();
            if parser.at_punctuation(Punctuation::RightParen) {
                break;
            }
            expression(parser, newline::Boundary::Delimited);
            parser.eat_newlines();
        }
    }
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    Expression {
        completed: parser.complete(
            marker,
            if tuple {
                NodeKind::TupleExpression
            } else {
                NodeKind::GroupedExpression
            },
        ),
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

fn previous_is_joint(parser: &Parser<'_>) -> bool {
    parser.cursor > 0 && parser.tokens[parser.cursor - 1].is_joint_to_next()
}

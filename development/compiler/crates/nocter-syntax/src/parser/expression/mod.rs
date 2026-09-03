mod control;
mod primary;
#[cfg(test)]
mod tests;

use super::{CompletedMarker, Parser, newline, place, types};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ExpressionMode {
    Ordinary,
    Header,
    Delimited,
}

impl ExpressionMode {
    fn newline_boundary(self) -> newline::Boundary {
        match self {
            Self::Delimited => newline::Boundary::Delimited,
            Self::Ordinary | Self::Header => newline::Boundary::Statement,
        }
    }
}

pub(super) fn expression(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    let marker = parser.start();
    recovery(parser, mode);
    parser.complete(marker, NodeKind::Expression)
}

pub(super) fn postfix(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    let mut left = primary::primary(parser, mode);
    loop {
        if parser.at_punctuation(Punctuation::LeftParen) {
            let parent = parser.precede(left);
            call_suffix(parser);
            left = parser.complete(parent, NodeKind::PostfixExpression);
            continue;
        }

        newline::before(parser, mode.newline_boundary(), |kind| {
            kind == TokenKind::Punctuation(Punctuation::Dot)
        });
        if parser.at_punctuation(Punctuation::Dot) {
            let parent = parser.precede(left);
            let suffix = parser.start();
            parser.bump();
            newline::after_incomplete(parser, mode.newline_boundary());
            let kind = if parser.at(TokenKind::IntegerLiteral) {
                parser.bump();
                NodeKind::TupleElementSuffix
            } else {
                parser.expect_name();
                NodeKind::MemberSuffix
            };
            parser.complete(suffix, kind);
            left = parser.complete(parent, NodeKind::PostfixExpression);
            continue;
        }

        if parser.at_punctuation(Punctuation::LeftBracket) && previous_is_joint(parser) {
            let parent = parser.precede(left);
            index_suffix(parser);
            left = parser.complete(parent, NodeKind::PostfixExpression);
            continue;
        }
        break;
    }
    left
}

fn recovery(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    let mut left = logical_or(parser, mode);
    if mode == ExpressionMode::Header {
        return left;
    }

    loop {
        newline::before(parser, mode.newline_boundary(), |kind| {
            matches!(
                kind,
                TokenKind::Keyword(Keyword::Catch | Keyword::Otherwise)
            )
        });
        if !parser.at_keyword(Keyword::Catch) && !parser.at_keyword(Keyword::Otherwise) {
            break;
        }
        let parent = parser.precede(left);
        recovery_clause(parser, mode);
        left = parser.complete(parent, NodeKind::RecoveryExpression);
    }
    left
}

fn recovery_clause(parser: &mut Parser<'_>, mode: ExpressionMode) {
    let marker = parser.start();
    if parser.eat_keyword(Keyword::Catch) {
        newline::after_incomplete(parser, mode.newline_boundary());
        parser.expect_name_or_discard();
    } else {
        parser.expect_keyword(Keyword::Otherwise);
        newline::after_incomplete(parser, mode.newline_boundary());
    }
    super::block::required(parser);
    parser.complete(marker, NodeKind::RecoveryClause);
}

fn logical_or(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    repeated_binary(
        parser,
        mode,
        logical_and,
        &[Punctuation::LogicalOr],
        NodeKind::LogicalOrExpression,
    )
}

fn logical_and(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    repeated_binary(
        parser,
        mode,
        equality,
        &[Punctuation::LogicalAnd],
        NodeKind::LogicalAndExpression,
    )
}

fn equality(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    single_binary(
        parser,
        mode,
        ordering,
        &[Punctuation::EqualEqual, Punctuation::BangEqual],
        NodeKind::EqualityExpression,
    )
}

fn ordering(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    single_binary(
        parser,
        mode,
        shift,
        &[
            Punctuation::Less,
            Punctuation::LessEqual,
            Punctuation::Greater,
            Punctuation::GreaterEqual,
        ],
        NodeKind::OrderingExpression,
    )
}

fn shift(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    repeated_binary(
        parser,
        mode,
        additive,
        &[Punctuation::ShiftLeft, Punctuation::ShiftRight],
        NodeKind::ShiftExpression,
    )
}

fn additive(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    repeated_binary(
        parser,
        mode,
        multiplicative,
        &[Punctuation::Plus, Punctuation::Minus],
        NodeKind::AdditiveExpression,
    )
}

fn multiplicative(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    repeated_binary(
        parser,
        mode,
        conversion,
        &[Punctuation::Star, Punctuation::Slash, Punctuation::Percent],
        NodeKind::MultiplicativeExpression,
    )
}

fn conversion(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    let mut left = unary(parser, mode);
    loop {
        newline::before(parser, mode.newline_boundary(), |kind| {
            kind == TokenKind::Keyword(Keyword::As)
        });
        if !parser.eat_keyword(Keyword::As) {
            break;
        }
        let parent = parser.precede(left);
        newline::after_incomplete(parser, mode.newline_boundary());
        types::type_(parser);
        left = parser.complete(parent, NodeKind::ConversionExpression);
    }
    left
}

fn unary(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    let mut wrappers = Vec::new();
    loop {
        if parser.at_punctuation(Punctuation::LogicalAnd) {
            let marker = parser.start();
            parser.split_current(
                TokenKind::Punctuation(Punctuation::Ampersand),
                TokenKind::Punctuation(Punctuation::Ampersand),
            );
            wrappers.push(marker);
            continue;
        }
        if matches!(
            parser.current_kind(),
            TokenKind::Punctuation(
                Punctuation::Bang
                    | Punctuation::Minus
                    | Punctuation::Ampersand
                    | Punctuation::ReadWrite
            )
        ) {
            let marker = parser.start();
            parser.bump();
            newline::after_incomplete(parser, mode.newline_boundary());
            wrappers.push(marker);
            continue;
        }
        break;
    }

    let mut inner = if parser.at_keyword(Keyword::Move) {
        move_expression(parser, mode)
    } else {
        outcome(parser, mode)
    };
    for marker in wrappers.into_iter().rev() {
        inner = parser.complete(marker, NodeKind::UnaryExpression);
    }
    inner
}

fn move_expression(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    newline::after_incomplete(parser, mode.newline_boundary());
    place::named(parser);
    if !parser.eat_punctuation(Punctuation::Question) {
        parser.eat_punctuation(Punctuation::Bang);
    }
    parser.complete(marker, NodeKind::MoveExpression)
}

fn outcome(parser: &mut Parser<'_>, mode: ExpressionMode) -> CompletedMarker {
    let inner = postfix(parser, mode);
    if parser.at_punctuation(Punctuation::Question) || parser.at_punctuation(Punctuation::Bang) {
        let marker = parser.precede(inner);
        parser.bump();
        parser.complete(marker, NodeKind::OutcomeExpression)
    } else {
        inner
    }
}

fn repeated_binary(
    parser: &mut Parser<'_>,
    mode: ExpressionMode,
    operand: fn(&mut Parser<'_>, ExpressionMode) -> CompletedMarker,
    operators: &[Punctuation],
    kind: NodeKind,
) -> CompletedMarker {
    let mut left = operand(parser, mode);
    loop {
        newline::before(parser, mode.newline_boundary(), |token| {
            operators.iter().any(|operator| {
                *operator != Punctuation::Minus && token == TokenKind::Punctuation(*operator)
            })
        });
        if !operators
            .iter()
            .any(|operator| parser.at_punctuation(*operator))
        {
            break;
        }
        let marker = parser.precede(left);
        parser.bump();
        newline::after_incomplete(parser, mode.newline_boundary());
        operand(parser, mode);
        left = parser.complete(marker, kind);
    }
    left
}

fn single_binary(
    parser: &mut Parser<'_>,
    mode: ExpressionMode,
    operand: fn(&mut Parser<'_>, ExpressionMode) -> CompletedMarker,
    operators: &[Punctuation],
    kind: NodeKind,
) -> CompletedMarker {
    let left = operand(parser, mode);
    newline::before(parser, mode.newline_boundary(), |token| {
        operators
            .iter()
            .any(|operator| token == TokenKind::Punctuation(*operator))
    });
    if operators
        .iter()
        .any(|operator| parser.at_punctuation(*operator))
    {
        let marker = parser.precede(left);
        parser.bump();
        newline::after_incomplete(parser, mode.newline_boundary());
        operand(parser, mode);
        parser.complete(marker, kind)
    } else {
        left
    }
}

fn call_suffix(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        parser.complete(marker, NodeKind::CallSuffix);
        return;
    }
    parser.comma_list(
        Punctuation::RightParen,
        true,
        ExpectedSyntax::Expression,
        call_argument,
    );
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::CallSuffix);
}

fn call_argument(parser: &mut Parser<'_>) {
    if !parser.at_punctuation(Punctuation::Expansion) {
        let key = expression(parser, ExpressionMode::Delimited);
        if parser.at_punctuation(Punctuation::Colon) {
            let marker = parser.precede(key);
            parser.bump();
            newline::after_incomplete(parser, newline::Boundary::Delimited);
            expression(parser, ExpressionMode::Delimited);
            parser.complete(marker, NodeKind::KeyedArgument);
        }
        return;
    }
    let marker = parser.start();
    parser.bump();
    newline::after_incomplete(parser, newline::Boundary::Delimited);
    if parser.at_punctuation(Punctuation::ReadWrite) {
        parser.error_token(ExpectedSyntax::Expression);
    } else {
        expression(parser, ExpressionMode::Delimited);
    }
    parser.complete(marker, NodeKind::SpreadExpression);
}

fn index_suffix(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBracket, Punctuation::RightBracket);
        parser.complete(marker, NodeKind::IndexSuffix);
        return;
    }
    parser.eat_newlines();
    expression(parser, ExpressionMode::Delimited);
    parser.eat_newlines();
    parser.expect_punctuation(Punctuation::RightBracket);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::IndexSuffix);
}

fn previous_is_joint(parser: &Parser<'_>) -> bool {
    parser.cursor > 0 && parser.tokens[parser.cursor - 1].is_joint_to_next()
}

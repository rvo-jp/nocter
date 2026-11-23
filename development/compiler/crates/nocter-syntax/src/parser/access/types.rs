//! One recursive type grammar for every private-core annotation position.
use super::{Parser, expression, newline};
use crate::{ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

fn starts_callable(parser: &Parser<'_>) -> bool {
    parser.at_keyword(Keyword::Func)
        || ((parser.at_contextual(ContextualSpelling::Mut)
            || parser.at_contextual(ContextualSpelling::Once))
            && parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::LeftParen))
}

pub(super) fn type_(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_keyword(Keyword::Future) {
        super::super::types::future_type(parser, type_);
        parser.complete(marker, NodeKind::Type);
        return;
    }
    if starts_callable(parser) {
        callable(parser);
        parser.complete(marker, NodeKind::Type);
        return;
    }
    let shape = parser.start();
    if parser.eat_punctuation(Punctuation::LeftBracket) {
        if parser.enter_nesting() {
            parser.eat_newlines();
            type_(parser);
            parser.eat_newlines();
            parser.expect_punctuation(Punctuation::Semicolon);
            parser.eat_newlines();
            expression::expression(parser, newline::Boundary::Delimited);
            parser.eat_newlines();
            parser.expect_punctuation(Punctuation::RightBracket);
            parser.leave_nesting();
            parser.complete(shape, NodeKind::FixedArrayType);
        } else {
            parser.recover_balanced(Punctuation::LeftBracket, Punctuation::RightBracket);
            parser.complete(shape, NodeKind::Error);
        }
    } else if parser.eat_punctuation(Punctuation::LeftParen) {
        if parser.enter_nesting() {
            parser.eat_newlines();
            type_(parser);
            parser.eat_newlines();
            let tuple = parser.eat_punctuation(Punctuation::Comma);
            if tuple {
                parser.eat_newlines();
                type_(parser);
                parser.eat_newlines();
                while parser.eat_punctuation(Punctuation::Comma) {
                    parser.eat_newlines();
                    if parser.at_punctuation(Punctuation::RightParen) {
                        break;
                    }
                    type_(parser);
                    parser.eat_newlines();
                }
            }
            parser.expect_punctuation(Punctuation::RightParen);
            parser.leave_nesting();
            parser.complete(
                shape,
                if tuple {
                    NodeKind::TupleType
                } else {
                    NodeKind::GroupedType
                },
            );
        } else {
            parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
            parser.complete(shape, NodeKind::Error);
        }
    } else {
        if matches!(
            parser.current_kind(),
            TokenKind::Identifier | TokenKind::Keyword(Keyword::Void)
        ) {
            let qualified = parser.at(TokenKind::Identifier);
            parser.bump();
            if qualified {
                loop {
                    if parser.at_punctuation(Punctuation::Less) {
                        generic_arguments(parser);
                    }
                    if !parser.eat_punctuation(Punctuation::Dot) {
                        break;
                    }
                    parser.expect_name();
                }
            }
        } else {
            parser.error_token(ExpectedSyntax::Type);
        }
        parser.complete(shape, NodeKind::NamedType);
    }
    while parser.eat_punctuation(Punctuation::Question) {}
    parser.eat_punctuation(Punctuation::Bang);
    parser.complete(marker, NodeKind::Type);
}

pub(super) fn generic_arguments(parser: &mut Parser<'_>) {
    super::super::types::generic_arguments(
        parser,
        super::super::types::ListRecovery::Stop,
        |parser| {
            super::super::types::generic_argument(parser, type_, expression::generic_argument);
        },
    );
}

fn callable(parser: &mut Parser<'_>) {
    let contract = parser.start();
    if !parser.enter_nesting() {
        parser.recover_to_line_end();
        parser.complete(contract, NodeKind::Error);
        return;
    }
    parser.bump();
    let parameters = parser.start();
    parser.expect_punctuation(Punctuation::LeftParen);
    if parser.enter_nesting() {
        parser.comma_list(
            Punctuation::RightParen,
            true,
            ExpectedSyntax::Parameter,
            callable_parameter,
        );
        parser.expect_punctuation(Punctuation::RightParen);
        parser.leave_nesting();
    } else {
        parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
    }
    parser.complete(parameters, NodeKind::CallableParameters);
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
    parser.complete(contract, NodeKind::CallableType);
    parser.leave_nesting();
}

fn callable_parameter(parser: &mut Parser<'_>) {
    let parameter = parser.start();
    super::input_operation(parser, |next| {
        matches!(
            next,
            TokenKind::Identifier
                | TokenKind::Keyword(Keyword::Void | Keyword::Func | Keyword::Future)
                | TokenKind::Punctuation(Punctuation::LeftParen | Punctuation::LeftBracket)
        )
    });
    if matches!(
        parser.current_kind(),
        TokenKind::Identifier | TokenKind::Punctuation(Punctuation::LeftParen)
    ) {
        let _ = parser.attempt_decided_with(
            |parser| {
                super::super::statement::binding_pattern(parser);
                parser.eat_punctuation(Punctuation::Colon)
            },
            |_, decided| *decided,
        );
    }
    type_(parser);
    parser.complete(parameter, NodeKind::CallableParameter);
}

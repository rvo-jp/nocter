use super::Parser;
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn type_(parser: &mut Parser<'_>) {
    let marker = parser.start();
    prefix_type(parser);
    outcome_suffix(parser);
    parser.complete(marker, NodeKind::Type);
}

pub(super) fn callable_result(parser: &mut Parser<'_>) {
    if parser.at_identifier_text("some") {
        opaque_result(parser);
        outcome_suffix(parser);
    } else {
        type_(parser);
    }
}

pub(super) fn generic_parameters(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    type_delimited_list(parser, false, ExpectedSyntax::Name, |parser| {
        parser.expect_name();
    });
    parser.expect_type_greater();
    parser.complete(marker, NodeKind::GenericParameters);
}

pub(super) fn parameters(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_punctuation(Punctuation::LeftParen);
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightParen)]);
        parser.expect_punctuation(Punctuation::RightParen);
        parser.complete(marker, NodeKind::Parameters);
        return;
    }
    parser.comma_list(
        Punctuation::RightParen,
        true,
        ExpectedSyntax::Parameter,
        parameter,
    );
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::Parameters);
}

pub(super) fn provenance_clause(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    while parser.eat_punctuation(Punctuation::Pipe) {
        parser.expect_name();
    }
    parser.complete(marker, NodeKind::ProvenanceClause);
}

pub(super) fn capability(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if at_callable_type(parser) {
        callable_type(parser);
    } else if parser.at(TokenKind::Identifier) {
        named_type(parser);
    } else {
        parser.error_token(ExpectedSyntax::Capability);
    }
    parser.complete(marker, NodeKind::Capability);
}

fn parameter(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
    parser.complete(marker, NodeKind::Parameter);
}

fn prefix_type(parser: &mut Parser<'_>) {
    let mut wrappers = Vec::new();
    let mut callable_allowed = true;

    loop {
        if callable_allowed && at_callable_type(parser) {
            callable_type(parser);
            break;
        }
        if parser.at_punctuation(Punctuation::Star) {
            let marker = parser.start();
            parser.bump();
            wrappers.push((marker, NodeKind::PointerType));
            callable_allowed = true;
            continue;
        }
        if !at_readonly_borrow(parser) && !parser.at_punctuation(Punctuation::ReadWrite) {
            type_atom(parser);
            break;
        }
        let marker = parser.start();
        if parser.at_punctuation(Punctuation::LogicalAnd) {
            parser.split_current(
                TokenKind::Punctuation(Punctuation::Ampersand),
                TokenKind::Punctuation(Punctuation::Ampersand),
            );
        } else {
            parser.bump();
        }
        wrappers.push((marker, NodeKind::BorrowType));
        callable_allowed = false;
    }

    for (marker, kind) in wrappers.into_iter().rev() {
        parser.complete(marker, kind);
    }
}

fn at_readonly_borrow(parser: &Parser<'_>) -> bool {
    parser.at_punctuation(Punctuation::Ampersand) || parser.at_punctuation(Punctuation::LogicalAnd)
}

fn at_callable_type(parser: &Parser<'_>) -> bool {
    parser.at_keyword(Keyword::Func)
        || parser.at_punctuation(Punctuation::Ampersand)
            && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func)
        || parser.at_punctuation(Punctuation::ReadWrite)
            && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func)
}

fn callable_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_punctuation(Punctuation::Ampersand)
        || parser.at_punctuation(Punctuation::ReadWrite)
    {
        parser.bump();
    }
    parser.expect_keyword(Keyword::Func);
    let parameters = parser.start();
    parser.expect_punctuation(Punctuation::LeftParen);
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightParen)]);
        parser.expect_punctuation(Punctuation::RightParen);
        parser.complete(parameters, NodeKind::CallableParameters);
        parser.complete(marker, NodeKind::CallableType);
        return;
    }
    parser.comma_list(
        Punctuation::RightParen,
        true,
        ExpectedSyntax::Parameter,
        callable_parameter,
    );
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    parser.complete(parameters, NodeKind::CallableParameters);
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
    if parser.at_identifier_text("from") {
        provenance_clause(parser);
    }
    parser.complete(marker, NodeKind::CallableType);
}

fn callable_parameter(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at(TokenKind::Identifier)
        && parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::Colon)
    {
        parser.bump();
        parser.bump();
    }
    type_(parser);
    parser.complete(marker, NodeKind::CallableParameter);
}

fn type_atom(parser: &mut Parser<'_>) {
    match parser.current_kind() {
        TokenKind::Identifier => named_type(parser),
        TokenKind::Keyword(Keyword::Void | Keyword::Never) => parser.bump(),
        TokenKind::Punctuation(Punctuation::LeftBracket) => bracket_type(parser),
        TokenKind::Punctuation(Punctuation::LeftParen) => grouped_type(parser),
        _ => parser.error_token(ExpectedSyntax::Type),
    }
}

fn named_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if parser.at_punctuation(Punctuation::Less) {
        parser.attempt(type_arguments);
    }
    while parser.eat_punctuation(Punctuation::Dot) {
        parser.expect_name();
        if parser.at_punctuation(Punctuation::Less) {
            parser.attempt(type_arguments);
        }
    }
    parser.complete(marker, NodeKind::NamedType);
}

fn type_arguments(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_until(&[
            TokenKind::Punctuation(Punctuation::Greater),
            TokenKind::Punctuation(Punctuation::ShiftRight),
        ]);
        parser.expect_type_greater();
        parser.complete(marker, NodeKind::TypeArguments);
        return;
    }
    type_delimited_list(parser, false, ExpectedSyntax::Type, type_);
    parser.expect_type_greater();
    parser.leave_nesting();
    parser.complete(marker, NodeKind::TypeArguments);
}

fn bracket_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightBracket)]);
        parser.expect_punctuation(Punctuation::RightBracket);
        parser.complete(marker, NodeKind::SliceType);
        return;
    }
    type_(parser);
    let kind = if parser.eat_punctuation(Punctuation::Semicolon) {
        parser.expect(TokenKind::IntegerLiteral);
        NodeKind::FixedArrayType
    } else {
        NodeKind::SliceType
    };
    parser.expect_punctuation(Punctuation::RightBracket);
    parser.leave_nesting();
    parser.complete(marker, kind);
}

fn grouped_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_until(&[TokenKind::Punctuation(Punctuation::RightParen)]);
        parser.expect_punctuation(Punctuation::RightParen);
        parser.complete(marker, NodeKind::GroupedType);
        return;
    }
    type_(parser);
    parser.expect_punctuation(Punctuation::RightParen);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::GroupedType);
}

fn outcome_suffix(parser: &mut Parser<'_>) {
    if parser.at_punctuation(Punctuation::Question) {
        parser.bump();
        parser.eat_punctuation(Punctuation::Bang);
    } else if parser.at_punctuation(Punctuation::Bang) {
        parser.bump();
    }
}

fn opaque_result(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    if parser.at_punctuation(Punctuation::Less) {
        let arguments = parser.start();
        parser.bump();
        if !parser.enter_nesting() {
            parser.recover_until(&[
                TokenKind::Punctuation(Punctuation::Greater),
                TokenKind::Punctuation(Punctuation::ShiftRight),
            ]);
            parser.expect_type_greater();
            parser.complete(arguments, NodeKind::OpaqueArguments);
            parser.complete(marker, NodeKind::OpaqueResult);
            return;
        }
        type_delimited_list(parser, false, ExpectedSyntax::Type, opaque_argument);
        parser.expect_type_greater();
        parser.leave_nesting();
        parser.complete(arguments, NodeKind::OpaqueArguments);
    }
    parser.complete(marker, NodeKind::OpaqueResult);
}

fn opaque_argument(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at(TokenKind::Identifier)
        && parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::Equal)
    {
        parser.bump();
        parser.bump();
    }
    type_(parser);
    parser.complete(marker, NodeKind::OpaqueArgument);
}

fn type_delimited_list(
    parser: &mut Parser<'_>,
    allow_empty: bool,
    expected_element: ExpectedSyntax,
    parse_element: fn(&mut Parser<'_>),
) {
    parser.eat_newlines();
    if at_type_list_end(parser) {
        if !allow_empty {
            parser.missing(expected_element);
        }
        return;
    }

    loop {
        let before = parser.cursor;
        parse_element(parser);
        if parser.cursor == before && parser.split.is_none() {
            parser.error_token(expected_element);
        }
        if parser.eat_punctuation(Punctuation::Comma) {
            parser.eat_newlines();
            if at_type_list_end(parser) {
                return;
            }
            continue;
        }
        parser.eat_newlines();
        if at_type_list_end(parser) {
            return;
        }
        parser.missing(ExpectedSyntax::Punctuation(Punctuation::Comma));
    }
}

fn at_type_list_end(parser: &Parser<'_>) -> bool {
    parser.at_punctuation(Punctuation::Greater)
        || parser.at_punctuation(Punctuation::ShiftRight)
        || parser.at(TokenKind::Eof)
}

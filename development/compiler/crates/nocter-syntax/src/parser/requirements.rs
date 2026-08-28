use super::{Parser, types};
use crate::{ExpectedSyntax, NodeKind, Punctuation, TokenKind};

pub(super) fn where_clause(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    predicate(parser);
    while parser.eat_punctuation(Punctuation::Comma) {
        parser.eat_newlines();
        predicate(parser);
    }
    parser.complete(marker, NodeKind::WhereClause);
}

fn predicate(parser: &mut Parser<'_>) {
    if parser.at_punctuation(Punctuation::LeftParen) {
        parenthesized_predicate(parser);
    } else if parser.at_punctuation(Punctuation::Ampersand)
        || parser.at_punctuation(Punctuation::ReadWrite)
    {
        if !parser.attempt(coercion_predicate) {
            type_equality_predicate(parser);
        }
    } else if parser.at_identifier_text("copy") && parser.nth_kind(1) == TokenKind::Identifier {
        copy_predicate(parser);
    } else if parser.at(TokenKind::Identifier)
        && parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::Colon)
    {
        callable_predicate(parser);
    } else if parser.at(TokenKind::Identifier) && parser.attempt(interface_predicate) {
    } else {
        type_equality_predicate(parser);
    }
}

fn callable_predicate(parser: &mut Parser<'_>) {
    let marker = parser.start();
    types::type_(parser);
    parser.expect_punctuation(Punctuation::Colon);
    types::type_(parser);
    parser.complete(marker, NodeKind::CallablePredicate);
}

fn interface_predicate(parser: &mut Parser<'_>) {
    let marker = parser.start();
    types::type_(parser);
    parser.expect_keyword(crate::Keyword::Impl);
    types::interface_application(parser);
    parser.complete(marker, NodeKind::InterfacePredicate);
}

fn copy_predicate(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    parser.complete(marker, NodeKind::CopyPredicate);
}

fn type_equality_predicate(parser: &mut Parser<'_>) {
    let marker = parser.start();
    let before = parser.cursor;
    types::type_(parser);
    if parser.cursor == before && parser.split.is_none() {
        parser.error_token(ExpectedSyntax::Predicate);
    }
    parser.expect_punctuation(Punctuation::Equal);
    types::type_(parser);
    parser.complete(marker, NodeKind::TypeEqualityPredicate);
}

fn parenthesized_predicate(parser: &mut Parser<'_>) {
    if parser.nth_kind(1) == TokenKind::Punctuation(Punctuation::Expansion) {
        expansion_predicate(parser);
    } else {
        operator_predicate(parser);
    }
}

fn operator_predicate(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    let readonly = if parser.eat_punctuation(Punctuation::Ampersand) {
        true
    } else {
        parser.expect_punctuation(Punctuation::ReadWrite);
        false
    };
    types::type_(parser);

    if parser.eat_punctuation(Punctuation::LeftBracket) {
        types::type_(parser);
        parser.expect_punctuation(Punctuation::RightBracket);
    } else if readonly
        && (parser.eat_punctuation(Punctuation::EqualEqual)
            || parser.eat_punctuation(Punctuation::Less))
    {
        parser.expect_punctuation(Punctuation::Ampersand);
        types::type_(parser);
    } else {
        parser.error_token(ExpectedSyntax::Predicate);
    }

    parser.expect_punctuation(Punctuation::RightParen);
    parser.expect_punctuation(Punctuation::Colon);
    types::type_(parser);
    parser.complete(marker, NodeKind::OperatorPredicate);
}

fn coercion_predicate(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    types::type_(parser);
    parser.expect_keyword(crate::Keyword::As);
    types::type_(parser);
    parser.complete(marker, NodeKind::CoercionPredicate);
}

fn expansion_predicate(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    parser.bump();
    if parser.at_punctuation(Punctuation::Ampersand)
        || parser.at_punctuation(Punctuation::ReadWrite)
    {
        parser.bump();
    }
    types::type_(parser);
    parser.expect_punctuation(Punctuation::RightParen);
    parser.expect_punctuation(Punctuation::Colon);
    types::type_(parser);
    parser.complete(marker, NodeKind::ExpansionPredicate);
}

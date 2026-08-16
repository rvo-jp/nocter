use super::{Parser, requirements, root, types};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn item(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_punctuation(Punctuation::Hash) {
        target_directive(parser);
    }
    targetable_item(parser);
    parser.complete(marker, NodeKind::Item);
}

fn target_directive(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if parser.at_identifier_text("target") {
        parser.bump();
    } else {
        parser.error_token(ExpectedSyntax::Name);
    }
    parser.expect_punctuation(Punctuation::Colon);
    root::string_literal(parser);
    if parser.eat_newlines() == 0 {
        parser.missing(ExpectedSyntax::Newline);
    }
    parser.complete(marker, NodeKind::TargetDirective);
}

fn targetable_item(parser: &mut Parser<'_>) {
    let keyword = item_keyword(parser);
    match keyword {
        Some(Keyword::Func) => function(parser),
        Some(Keyword::Primitive) => primitive(parser),
        Some(Keyword::Type) => type_alias(parser),
        _ => parser.error_token(ExpectedSyntax::TargetableItem),
    }
}

fn item_keyword(parser: &Parser<'_>) -> Option<Keyword> {
    let mut cursor = parser.cursor;
    if parser.tokens[cursor].kind() == TokenKind::Keyword(Keyword::Pub) {
        cursor = skip_visibility(parser, cursor);
    }
    match parser.tokens[cursor].kind() {
        TokenKind::Keyword(keyword) => Some(keyword),
        _ => None,
    }
}

fn skip_visibility(parser: &Parser<'_>, mut cursor: usize) -> usize {
    cursor += 1;
    if parser.tokens[cursor].kind() != TokenKind::Punctuation(Punctuation::LeftParen) {
        return cursor;
    }
    cursor += 1;
    while !matches!(
        parser.tokens[cursor].kind(),
        TokenKind::Punctuation(Punctuation::RightParen) | TokenKind::Newline | TokenKind::Eof
    ) {
        cursor += 1;
    }
    if parser.tokens[cursor].kind() == TokenKind::Punctuation(Punctuation::RightParen) {
        cursor += 1;
    }
    cursor
}

fn function(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.expect_keyword(Keyword::Func);
    parser.expect_name();
    if parser.at_punctuation(Punctuation::Less) {
        types::generic_parameters(parser);
    }
    types::parameters(parser);
    callable_tail(parser);
    optional_empty_block(parser);
    parser.complete(marker, NodeKind::FunctionDeclaration);
}

fn primitive(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.expect_keyword(Keyword::Primitive);
    parser.expect_name();
    if parser.at_punctuation(Punctuation::Less) {
        types::generic_parameters(parser);
    }
    types::parameters(parser);
    callable_tail(parser);
    parser.complete(marker, NodeKind::PrimitiveDeclaration);
}

fn type_alias(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.expect_keyword(Keyword::Type);
    parser.expect_name();
    if parser.at_punctuation(Punctuation::Less) {
        types::generic_parameters(parser);
    }
    parser.expect_punctuation(Punctuation::Equal);
    types::type_(parser);
    if parser.at_identifier_text("where") {
        requirements::where_clause(parser);
    }
    parser.complete(marker, NodeKind::TypeAliasDeclaration);
}

fn callable_tail(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_punctuation(Punctuation::Colon);
    types::callable_result(parser);
    if parser.at_identifier_text("from") {
        types::provenance_clause(parser);
    }
    if parser.at_identifier_text("where") {
        requirements::where_clause(parser);
    }
    parser.complete(marker, NodeKind::CallableTail);
}

fn optional_visibility(parser: &mut Parser<'_>) {
    if parser.at_keyword(Keyword::Pub) {
        root::visibility(parser);
    }
}

fn optional_empty_block(parser: &mut Parser<'_>) {
    if !parser.at_punctuation(Punctuation::LeftBrace) {
        return;
    }
    let marker = parser.start();
    parser.bump();
    parser.eat_newlines();
    if !parser.at_punctuation(Punctuation::RightBrace) {
        parser.error_token(ExpectedSyntax::Punctuation(Punctuation::RightBrace));
        while !parser.at_punctuation(Punctuation::RightBrace) && !parser.at(TokenKind::Eof) {
            parser.error_token(ExpectedSyntax::Punctuation(Punctuation::RightBrace));
        }
    }
    parser.expect_punctuation(Punctuation::RightBrace);
    parser.complete(marker, NodeKind::Block);
}

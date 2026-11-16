//! Declarative package data shared by explicitly selecting source grammars.
use super::{Parser, root::string_literal};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};
use nocter_language::PackageDirectiveName;

pub(super) fn prefix(parser: &mut Parser<'_>) {
    while at_package_directive_start(parser) {
        package_directive(parser);
        parser.require_line_end();
    }
}

fn package_directive(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();

    if is_package_directive_name(parser) {
        parser.bump();
    } else {
        parser.error_token(ExpectedSyntax::PackageDirectiveName);
    }
    parser.expect_punctuation(Punctuation::Colon);
    directive_value(parser);
    parser.complete(marker, NodeKind::PackageDirective);
}

fn at_package_directive_start(parser: &Parser<'_>) -> bool {
    if !parser.at_punctuation(Punctuation::Hash) {
        return false;
    }
    let Some(next) = parser.tokens.get(parser.cursor + 1) else {
        return false;
    };
    matches!(
        next.kind(),
        TokenKind::Identifier | TokenKind::Keyword(Keyword::Test)
    ) && parser
        .source
        .text_at(next.span().range())
        .and_then(PackageDirectiveName::from_spelling)
        .is_some()
}

fn is_package_directive_name(parser: &Parser<'_>) -> bool {
    matches!(
        parser.current_kind(),
        TokenKind::Identifier | TokenKind::Keyword(Keyword::Test)
    ) && PackageDirectiveName::from_spelling(parser.current_text()).is_some()
}

fn directive_value(parser: &mut Parser<'_>) {
    let marker = parser.start();
    match parser.current_kind() {
        TokenKind::StringStart(_) => string_literal(parser),
        TokenKind::IntegerLiteral => parser.bump(),
        TokenKind::Punctuation(Punctuation::LeftBrace) => directive_record(parser),
        _ => parser.error_token(ExpectedSyntax::DirectiveValue),
    }
    parser.complete(marker, NodeKind::DirectiveValue);
}

fn directive_record(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
        parser.complete(marker, NodeKind::DirectiveRecord);
        return;
    }

    parser.comma_list(
        Punctuation::RightBrace,
        true,
        ExpectedSyntax::Name,
        directive_field,
    );
    parser.expect_punctuation(Punctuation::RightBrace);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::DirectiveRecord);
}

fn directive_field(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    directive_value(parser);
    parser.complete(marker, NodeKind::DirectiveField);
}

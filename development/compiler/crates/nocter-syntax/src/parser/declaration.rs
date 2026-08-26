mod construct;
mod instance;
mod nominal;

use super::{Parser, block, expression, requirements, root, types};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn item(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_punctuation(Punctuation::Hash) {
        target_directive(parser);
        targetable_item(parser);
    } else {
        declaration(parser);
    }
    parser.complete(marker, NodeKind::Item);
}

fn declaration(parser: &mut Parser<'_>) {
    match declaration_kind(parser) {
        Some(DeclarationKind::Constant) => constant(parser),
        Some(DeclarationKind::Function) => function(parser, false),
        Some(DeclarationKind::PrimitiveFunction) => function(parser, true),
        Some(DeclarationKind::PrimitiveType) => primitive_type(parser),
        Some(DeclarationKind::TypeAlias) => type_alias(parser),
        Some(DeclarationKind::Struct) => nominal::struct_declaration(parser),
        Some(DeclarationKind::Enum) => nominal::enum_declaration(parser),
        Some(DeclarationKind::Interface) => nominal::interface_declaration(parser),
        Some(DeclarationKind::Construct) => construct::declaration(parser),
        Some(DeclarationKind::Instance) => instance::declaration(parser),
        Some(DeclarationKind::Drop) => drop_declaration(parser),
        Some(DeclarationKind::Test) => test_declaration(parser),
        None => parser.error_token(ExpectedSyntax::Item),
    }
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
    match targetable_kind(parser) {
        Some(DeclarationKind::Constant) => constant(parser),
        Some(DeclarationKind::Function) => function(parser, false),
        Some(DeclarationKind::PrimitiveFunction) => function(parser, true),
        Some(DeclarationKind::PrimitiveType) => primitive_type(parser),
        Some(DeclarationKind::TypeAlias) => type_alias(parser),
        Some(DeclarationKind::Struct) => nominal::struct_declaration(parser),
        Some(DeclarationKind::Enum) => nominal::enum_declaration(parser),
        Some(DeclarationKind::Interface) => nominal::interface_declaration(parser),
        _ => parser.error_token(ExpectedSyntax::TargetableItem),
    }
}

#[derive(Clone, Copy)]
enum DeclarationKind {
    Constant,
    Function,
    PrimitiveFunction,
    PrimitiveType,
    TypeAlias,
    Struct,
    Enum,
    Interface,
    Construct,
    Instance,
    Drop,
    Test,
}

fn declaration_kind(parser: &Parser<'_>) -> Option<DeclarationKind> {
    targetable_kind(parser).or_else(|| match parser.current_kind() {
        TokenKind::Keyword(Keyword::Construct) => Some(DeclarationKind::Construct),
        TokenKind::Keyword(Keyword::Instance) => Some(DeclarationKind::Instance),
        TokenKind::Keyword(Keyword::Test) => Some(DeclarationKind::Test),
        TokenKind::Identifier if parser.current_text() == "drop" => Some(DeclarationKind::Drop),
        _ => None,
    })
}

fn targetable_kind(parser: &Parser<'_>) -> Option<DeclarationKind> {
    let mut cursor = parser.cursor;
    if parser.tokens[cursor].kind() == TokenKind::Keyword(Keyword::Pub) {
        cursor = skip_visibility(parser, cursor);
    }

    if parser.tokens[cursor].kind() == TokenKind::Identifier
        && parser.source.text_at(parser.tokens[cursor].span().range()) == Some("copy")
        && parser.tokens[cursor + 1].kind() == TokenKind::Keyword(Keyword::Struct)
    {
        return Some(DeclarationKind::Struct);
    }

    match parser.tokens[cursor].kind() {
        TokenKind::Keyword(Keyword::Const) => Some(DeclarationKind::Constant),
        TokenKind::Keyword(Keyword::Func) => Some(DeclarationKind::Function),
        TokenKind::Keyword(Keyword::Primitive)
            if parser.tokens[cursor + 1].kind() == TokenKind::Keyword(Keyword::Func) =>
        {
            Some(DeclarationKind::PrimitiveFunction)
        }
        TokenKind::Keyword(Keyword::Primitive)
            if parser.tokens[cursor + 1].kind() == TokenKind::Keyword(Keyword::Type) =>
        {
            Some(DeclarationKind::PrimitiveType)
        }
        TokenKind::Keyword(Keyword::Type) => Some(DeclarationKind::TypeAlias),
        TokenKind::Keyword(Keyword::Struct) => Some(DeclarationKind::Struct),
        TokenKind::Keyword(Keyword::Enum) => Some(DeclarationKind::Enum),
        TokenKind::Keyword(Keyword::Interface) => Some(DeclarationKind::Interface),
        _ => None,
    }
}

fn constant(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.expect_keyword(Keyword::Const);
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    types::type_(parser);
    if parser.eat_punctuation(Punctuation::Equal) {
        expression::expression(parser, expression::ExpressionMode::Header);
    }
    parser.complete(marker, NodeKind::ConstantDeclaration);
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

fn function(parser: &mut Parser<'_>, primitive: bool) {
    let marker = parser.start();
    optional_visibility(parser);
    if primitive {
        parser.expect_keyword(Keyword::Primitive);
    }
    parser.expect_keyword(Keyword::Func);
    parser.expect_name();
    if parser.at_punctuation(Punctuation::Less) {
        types::generic_parameters(parser);
    }
    types::parameters(parser);
    callable_tail(parser);
    if primitive {
        if parser.at_punctuation(Punctuation::LeftBrace) {
            parser.error_token(ExpectedSyntax::Newline);
        }
    } else {
        block::optional(parser);
    }
    parser.complete(marker, NodeKind::FunctionDeclaration);
}

fn primitive_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.expect_keyword(Keyword::Primitive);
    parser.expect_keyword(Keyword::Type);
    if parser.at(TokenKind::Identifier)
        || parser.at_keyword(Keyword::Void)
        || parser.at_keyword(Keyword::Never)
    {
        parser.bump();
    } else {
        parser.error_token(ExpectedSyntax::Name);
    }
    parser.complete(marker, NodeKind::PrimitiveTypeDeclaration);
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

pub(super) fn callable_tail(parser: &mut Parser<'_>) {
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

pub(super) fn optional_visibility(parser: &mut Parser<'_>) {
    if parser.at_keyword(Keyword::Pub) {
        root::visibility(parser);
    }
}

pub(super) fn method_signature(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_keyword(Keyword::Method);
    receiver(parser, true);
    parser.expect_punctuation(Punctuation::Dot);
    parser.expect_name();
    if parser.at_punctuation(Punctuation::Less) {
        types::generic_parameters(parser);
    }
    types::parameters(parser);
    callable_tail(parser);
    parser.complete(marker, NodeKind::MethodSignature);
}

pub(super) fn receiver(parser: &mut Parser<'_>, allow_owned: bool) {
    let marker = parser.start();
    if parser.at_punctuation(Punctuation::Ampersand)
        || parser.at_punctuation(Punctuation::ReadWrite)
    {
        parser.bump();
        parser.expect_identifier_text("self");
    } else if allow_owned && parser.at_identifier_text("self") {
        parser.bump();
    } else {
        parser.error_token(ExpectedSyntax::Receiver);
    }
    parser.complete(marker, NodeKind::Receiver);
}

fn drop_declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_identifier_text("drop");
    types::declaration_type_pattern(parser);
    parser.expect_punctuation(Punctuation::LeftParen);
    if parser.eat_punctuation(Punctuation::ReadWrite) {
        parser.expect_identifier_text("self");
    } else {
        parser.error_token(ExpectedSyntax::Receiver);
    }
    parser.expect_punctuation(Punctuation::RightParen);
    block::required(parser);
    parser.complete(marker, NodeKind::DropDeclaration);
}

fn test_declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_keyword(Keyword::Test);
    parser.expect_name();
    block::required(parser);
    parser.complete(marker, NodeKind::TestDeclaration);
}

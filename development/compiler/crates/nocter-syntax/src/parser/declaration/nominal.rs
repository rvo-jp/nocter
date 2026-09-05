use super::{
    Parser, block, optional_noalloc, optional_visibility, requirements, skip_visibility, types,
};
use crate::{ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn struct_declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    if parser.at_contextual(ContextualSpelling::Copy) {
        parser.bump();
    }
    parser.expect_keyword(Keyword::Struct);
    parser.expect_name();
    generic_parameters_and_requirements(parser);
    if parser.at_punctuation(Punctuation::LeftBrace) {
        parser.braced_line_sequence(ExpectedSyntax::DeclarationMember, struct_field);
    }
    parser.complete(marker, NodeKind::StructDeclaration);
}

pub(super) fn enum_declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.expect_keyword(Keyword::Enum);
    parser.expect_name();
    generic_parameters_and_requirements(parser);
    if parser.at_punctuation(Punctuation::LeftBrace) {
        parser.braced_line_sequence(ExpectedSyntax::DeclarationMember, enum_variant);
    }
    parser.complete(marker, NodeKind::EnumDeclaration);
}

pub(super) fn interface_declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.expect_keyword(Keyword::Interface);
    parser.expect_name();
    generic_parameters_and_requirements(parser);
    parser.braced_line_sequence(ExpectedSyntax::DeclarationMember, interface_member);
    parser.complete(marker, NodeKind::InterfaceDeclaration);
}

fn generic_parameters_and_requirements(parser: &mut Parser<'_>) {
    if parser.at_punctuation(Punctuation::Less) {
        types::generic_parameters(parser);
    }
    if parser.at_contextual(ContextualSpelling::Where) {
        requirements::where_clause(parser);
    }
}

fn struct_field(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    types::type_(parser);
    parser.complete(marker, NodeKind::StructField);
}

fn enum_variant(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    if parser.at_punctuation(Punctuation::LeftParen) {
        let payload = parser.start();
        types::parameters(parser);
        parser.complete(payload, NodeKind::EnumPayload);
    }
    parser.complete(marker, NodeKind::EnumVariant);
}

fn interface_member(parser: &mut Parser<'_>) {
    let mut cursor = parser.cursor;
    let has_visibility = parser.tokens[cursor].kind() == TokenKind::Keyword(Keyword::Pub);
    let has_bare_public_visibility = has_visibility
        && parser.tokens[cursor + 1].kind() != TokenKind::Punctuation(Punctuation::LeftParen);
    if has_visibility {
        cursor = skip_visibility(parser, cursor);
    }
    if parser.tokens[cursor].kind() == TokenKind::Keyword(Keyword::NoAlloc) {
        cursor += 1;
    }
    let is_default = parser.contextual_at(cursor, ContextualSpelling::Default);
    if is_default {
        cursor += 1;
    }

    match parser.tokens[cursor].kind() {
        TokenKind::Keyword(Keyword::Type) if has_bare_public_visibility && !is_default => {
            associated_type(parser);
        }
        TokenKind::Keyword(Keyword::Method) if has_bare_public_visibility || is_default => {
            interface_method(parser, is_default);
        }
        _ => parser.error_token(ExpectedSyntax::DeclarationMember),
    }
}

fn associated_type(parser: &mut Parser<'_>) {
    let marker = parser.start();
    optional_visibility(parser);
    parser.bump();
    parser.expect_name();
    if parser.at_keyword(Keyword::Impl) {
        let bounds = parser.start();
        parser.bump();
        types::interface_application(parser);
        while parser.eat_punctuation(Punctuation::Plus) {
            types::interface_application(parser);
        }
        parser.complete(bounds, NodeKind::InterfaceBounds);
    }
    parser.complete(marker, NodeKind::AssociatedTypeDeclaration);
}

fn interface_method(parser: &mut Parser<'_>, is_default: bool) {
    let marker = parser.start();
    optional_visibility(parser);
    optional_noalloc(parser);
    if is_default {
        let modifier = parser.start();
        parser.expect_contextual(ContextualSpelling::Default);
        parser.complete(modifier, NodeKind::InterfaceDefaultModifier);
    }
    super::method_signature(parser);
    if is_default {
        block::optional(parser);
    } else if parser.at_punctuation(Punctuation::LeftBrace) {
        parser.missing(ExpectedSyntax::Contextual(
            ContextualSpelling::Default.as_str(),
        ));
        block::optional(parser);
    }
    parser.complete(marker, NodeKind::InterfaceMethod);
}

//! Nominal declaration bodies keep ordered authored fields and variants.
use super::{Parser, directives, type_};
use crate::{ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, Punctuation};

pub(super) fn record(parser: &mut Parser<'_>) {
    declaration(
        parser,
        Keyword::Struct,
        NodeKind::StructDeclaration,
        record_field,
    );
}

pub(super) fn enumeration(parser: &mut Parser<'_>) {
    declaration(
        parser,
        Keyword::Enum,
        NodeKind::EnumDeclaration,
        enum_variant,
    );
}

fn declaration(
    parser: &mut Parser<'_>,
    keyword: Keyword,
    kind: NodeKind,
    member: fn(&mut Parser<'_>),
) {
    let marker = parser.start();
    if parser.at_contextual(ContextualSpelling::Copy) {
        parser.bump();
    }
    parser.expect_keyword(keyword);
    parser.expect_name();
    if parser.at_punctuation(Punctuation::LeftBrace) {
        parser.braced_line_sequence(ExpectedSyntax::DeclarationMember, member);
    }
    parser.complete(marker, kind);
}

fn record_field(parser: &mut Parser<'_>) {
    let field = parser.start();
    directives::prefix(parser, false);
    field_data(parser);
    parser.complete(field, NodeKind::StructField);
}

fn enum_variant(parser: &mut Parser<'_>) {
    let variant = parser.start();
    parser.expect_name();
    if parser.at_punctuation(Punctuation::LeftParen) {
        let payload = parser.start();
        parser.bump();
        if parser.enter_nesting() {
            parser.comma_list(
                Punctuation::RightParen,
                false,
                ExpectedSyntax::Parameter,
                |parser| {
                    let field = parser.start();
                    field_data(parser);
                    parser.complete(field, NodeKind::Parameter);
                },
            );
            parser.expect_punctuation(Punctuation::RightParen);
            parser.leave_nesting();
        } else {
            parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        }
        parser.complete(payload, NodeKind::EnumPayload);
    }
    parser.complete(variant, NodeKind::EnumVariant);
}

fn field_data(parser: &mut Parser<'_>) {
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    type_(parser);
}

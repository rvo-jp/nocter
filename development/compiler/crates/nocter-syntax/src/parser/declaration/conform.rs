use super::{Parser, block, method_signature, requirements, types};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_keyword(Keyword::Conform);
    types::declaration_type_pattern(parser);
    parser.expect_keyword(Keyword::For);
    types::declaration_type_pattern(parser);
    if parser.at_identifier_text("where") {
        requirements::where_clause(parser);
    }
    parser.braced_line_sequence(ExpectedSyntax::DeclarationMember, member);
    parser.complete(marker, NodeKind::ConformDeclaration);
}

fn member(parser: &mut Parser<'_>) {
    match parser.current_kind() {
        TokenKind::Keyword(Keyword::Type) => associated_type_binding(parser),
        TokenKind::Keyword(Keyword::Method) => conform_method(parser),
        _ => parser.error_token(ExpectedSyntax::DeclarationMember),
    }
}

fn associated_type_binding(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Equal);
    types::type_(parser);
    parser.complete(marker, NodeKind::AssociatedTypeBinding);
}

fn conform_method(parser: &mut Parser<'_>) {
    let marker = parser.start();
    method_signature(parser);
    block::optional(parser);
    parser.complete(marker, NodeKind::ConformMethod);
}

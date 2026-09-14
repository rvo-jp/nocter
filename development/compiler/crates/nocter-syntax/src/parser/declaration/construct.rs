use super::{
    Parser, block, callable_tail, optional_async, optional_blocking, optional_const,
    optional_noalloc, root, types,
};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, StringDelimiter, TokenKind};

pub(super) fn declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_keyword(Keyword::Construct);
    types::declaration_type_pattern(parser);
    parser.braced_nonempty_line_sequence(ExpectedSyntax::DeclarationMember, member);
    parser.complete(marker, NodeKind::ConstructDeclaration);
}

fn member(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_keyword(Keyword::Pub) {
        root::visibility(parser);
    }
    if at_const_function(parser) {
        optional_const(parser);
    }
    optional_noalloc(parser);
    if parser.at_keyword(Keyword::Blocking)
        && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func)
    {
        optional_blocking(parser);
    }
    if parser.at_keyword(Keyword::Async) && parser.nth_kind(1) == TokenKind::Keyword(Keyword::Func)
    {
        optional_async(parser);
    }
    let kind = match parser.current_kind() {
        TokenKind::Keyword(Keyword::Func) => {
            construction_function(parser);
            NodeKind::ConstructionFunction
        }
        TokenKind::Keyword(Keyword::Literal) => {
            literal_declaration(parser);
            NodeKind::LiteralDeclaration
        }
        _ => {
            parser.error_token(ExpectedSyntax::DeclarationMember);
            NodeKind::Error
        }
    };
    parser.complete(marker, kind);
}

fn at_const_function(parser: &Parser<'_>) -> bool {
    if !parser.at_keyword(Keyword::Const) {
        return false;
    }
    let mut offset = 1;
    if parser.nth_kind(offset) == TokenKind::Keyword(Keyword::NoAlloc) {
        offset += 1;
    }
    if parser.nth_kind(offset) == TokenKind::Keyword(Keyword::Blocking) {
        offset += 1;
    }
    if parser.nth_kind(offset) == TokenKind::Keyword(Keyword::Async) {
        offset += 1;
    }
    parser.nth_kind(offset) == TokenKind::Keyword(Keyword::Func)
}

fn construction_function(parser: &mut Parser<'_>) {
    parser.bump();
    parser.expect_name();
    if parser.at_punctuation(Punctuation::Less) {
        types::generic_parameters(parser);
    }
    types::parameters(parser);
    callable_tail(parser);
    block::optional(parser);
}

fn literal_declaration(parser: &mut Parser<'_>) {
    parser.bump();
    literal_shape(parser);
    types::parameters(parser);
    callable_tail(parser);
    block::optional(parser);
}

fn literal_shape(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.eat_punctuation(Punctuation::LeftBracket) {
        parser.eat_punctuation(Punctuation::Colon);
        parser.expect_punctuation(Punctuation::RightBracket);
    } else if parser.at(TokenKind::StringStart(StringDelimiter::SingleLine))
        && parser.nth_kind(1) == TokenKind::StringEnd(StringDelimiter::SingleLine)
        && parser.tokens[parser.cursor].is_joint_to_next()
    {
        parser.bump();
        parser.bump();
    } else {
        parser.error_token(ExpectedSyntax::LiteralShape);
    }
    parser.complete(marker, NodeKind::LiteralShape);
}

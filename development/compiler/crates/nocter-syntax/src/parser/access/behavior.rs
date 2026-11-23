use super::{
    Parser, callable_modifier, callable_tail, directives, function_with_prefix, required_block,
};
use crate::{ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn declaration(parser: &mut Parser<'_>) {
    let declaration = parser.start();
    parser.expect_keyword(Keyword::Impl);
    let owner = parser.start();
    parser.expect_name();
    parser.complete(owner, NodeKind::DeclarationTypePattern);
    parser.braced_line_sequence(ExpectedSyntax::DeclarationMember, member);
    parser.complete(declaration, NodeKind::TypeBehaviorDeclaration);
}

fn member(parser: &mut Parser<'_>) {
    let declaration = parser.start();
    directives::prefix(parser, false);
    if parser.contextual_at(
        parser.cursor + usize::from(parser.at_keyword(Keyword::Const)),
        ContextualSpelling::Coerce,
    ) {
        coercion(parser, declaration);
        return;
    }
    let head = match parser.current_kind() {
        TokenKind::Keyword(Keyword::Const | Keyword::Async) => parser.nth_kind(1),
        head => head,
    };
    match head {
        TokenKind::Keyword(Keyword::Func) => {
            function_with_prefix(parser, declaration, NodeKind::ConstructionFunction);
        }
        TokenKind::Keyword(Keyword::Method) => method(parser, declaration),
        TokenKind::Keyword(Keyword::Literal) if parser.at_keyword(Keyword::Literal) => {
            structural_grant(parser, declaration);
        }
        _ => {
            parser.error_token(ExpectedSyntax::DeclarationMember);
            parser.complete(declaration, NodeKind::Error);
        }
    }
}

fn structural_grant(parser: &mut Parser<'_>, declaration: crate::parser::Marker) {
    parser.expect_keyword(Keyword::Literal);
    if parser.expect_punctuation(Punctuation::LeftBrace) {
        parser.comma_list(
            Punctuation::RightBrace,
            true,
            ExpectedSyntax::Name,
            |parser| {
                parser.expect_name();
            },
        );
        parser.expect_punctuation(Punctuation::RightBrace);
    }
    parser.complete(declaration, NodeKind::StructuralLiteralGrant);
}

fn method(parser: &mut Parser<'_>, method: crate::parser::Marker) {
    callable_modifier(parser);
    parser.expect_keyword(Keyword::Method);
    receiver(parser);
    parser.expect_punctuation(Punctuation::Dot);
    parser.expect_name();
    callable_tail(parser);
    if parser.at_punctuation(Punctuation::LeftBrace) {
        required_block(parser);
    }
    parser.complete(method, NodeKind::InherentMethod);
}

fn coercion(parser: &mut Parser<'_>, declaration: crate::parser::Marker) {
    callable_modifier(parser);
    parser.expect_contextual(ContextualSpelling::Coerce);
    receiver(parser);
    parser.expect_keyword(Keyword::As);
    super::type_(parser);
    if parser.at_punctuation(Punctuation::LeftBrace) {
        required_block(parser);
    }
    parser.complete(declaration, NodeKind::CoercionDeclaration);
}

fn receiver(parser: &mut Parser<'_>) {
    let receiver = parser.start();
    if parser.at_contextual(ContextualSpelling::Edit)
        || parser.at_contextual(ContextualSpelling::Take)
    {
        let operation = parser.start();
        parser.bump();
        parser.complete(operation, NodeKind::InputOperation);
    }
    parser.expect_contextual(ContextualSpelling::LowerSelf);
    parser.complete(receiver, NodeKind::Receiver);
}

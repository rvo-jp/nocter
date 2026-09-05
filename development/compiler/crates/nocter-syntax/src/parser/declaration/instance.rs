use super::{
    Parser, block, method_signature, optional_noalloc, optional_visibility, receiver, requirements,
    types,
};
use crate::{ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn declaration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_keyword(Keyword::Instance);
    types::declaration_type_pattern(parser);
    if parser.at_contextual(ContextualSpelling::Where) {
        requirements::where_clause(parser);
    }
    parser.braced_line_sequence(ExpectedSyntax::DeclarationMember, member);
    parser.complete(marker, NodeKind::InstanceDeclaration);
}

fn member(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_keyword(Keyword::Impl) {
        parser.bump();
        types::interface_application(parser);
        parser.complete(marker, NodeKind::InterfaceImplementation);
        return;
    }
    optional_visibility(parser);
    optional_noalloc(parser);
    let kind = match parser.current_kind() {
        TokenKind::Keyword(Keyword::Method) => {
            method_signature(parser);
            block::optional(parser);
            NodeKind::InherentMethod
        }
        TokenKind::Identifier if parser.at_contextual(ContextualSpelling::Coerce) => {
            coercion(parser);
            NodeKind::CoercionDeclaration
        }
        TokenKind::Keyword(Keyword::Operator) => operator(parser),
        _ => {
            parser.error_token(ExpectedSyntax::DeclarationMember);
            NodeKind::Error
        }
    };
    parser.complete(marker, kind);
}

fn coercion(parser: &mut Parser<'_>) {
    parser.expect_contextual(ContextualSpelling::Coerce);
    receiver(parser, false);
    parser.expect_keyword(Keyword::As);
    types::type_(parser);
    if parser.at_contextual(ContextualSpelling::From) {
        let provenance = parser.start();
        parser.bump();
        parser.expect_contextual(ContextualSpelling::LowerSelf);
        parser.complete(provenance, NodeKind::CoercionProvenance);
    }
    block::optional(parser);
}

fn operator(parser: &mut Parser<'_>) -> NodeKind {
    parser.bump();
    parser.expect_punctuation(Punctuation::LeftParen);
    if parser.eat_punctuation(Punctuation::Expansion) {
        expansion_operator(parser);
        return NodeKind::ExpansionOperator;
    }

    let readonly = parser.at_punctuation(Punctuation::Ampersand);
    receiver(parser, false);
    if parser.eat_punctuation(Punctuation::LeftBracket) {
        index_operator(parser);
        NodeKind::IndexOperator
    } else if readonly && parser.eat_punctuation(Punctuation::EqualEqual) {
        comparison_operator(parser);
        NodeKind::EqualityOperator
    } else if readonly && parser.eat_punctuation(Punctuation::Less) {
        comparison_operator(parser);
        NodeKind::OrderingOperator
    } else {
        parser.error_token(ExpectedSyntax::DeclarationMember);
        parser.recover_until(&[
            TokenKind::Punctuation(Punctuation::RightParen),
            TokenKind::Newline,
        ]);
        parser.eat_punctuation(Punctuation::RightParen);
        NodeKind::Error
    }
}

fn comparison_operator(parser: &mut Parser<'_>) {
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Colon);
    parser.expect_punctuation(Punctuation::Ampersand);
    parser.expect_contextual(ContextualSpelling::UpperSelf);
    parser.expect_punctuation(Punctuation::RightParen);
    parser.expect_punctuation(Punctuation::Colon);
    parser.expect_identifier_text("bool");
    optional_requirements(parser);
    block::optional(parser);
}

fn index_operator(parser: &mut Parser<'_>) {
    types::parameter(parser);
    parser.expect_punctuation(Punctuation::RightBracket);
    parser.expect_punctuation(Punctuation::RightParen);
    parser.expect_punctuation(Punctuation::Colon);
    types::borrow_type(parser);
    optional_provenance_and_requirements(parser);
    block::optional(parser);
}

fn expansion_operator(parser: &mut Parser<'_>) {
    receiver(parser, true);
    parser.expect_punctuation(Punctuation::RightParen);
    parser.expect_punctuation(Punctuation::Colon);
    types::type_(parser);
    optional_provenance_and_requirements(parser);
    block::optional(parser);
}

fn optional_provenance_and_requirements(parser: &mut Parser<'_>) {
    if parser.at_contextual(ContextualSpelling::From) {
        types::provenance_clause(parser);
    }
    optional_requirements(parser);
}

fn optional_requirements(parser: &mut Parser<'_>) {
    if parser.at_contextual(ContextualSpelling::Where) {
        requirements::where_clause(parser);
    }
}

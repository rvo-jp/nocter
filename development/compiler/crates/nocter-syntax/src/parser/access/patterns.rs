use super::{Parser, expression::header_expression, newline};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn if_condition(parser: &mut Parser<'_>) {
    header_expression(parser);
    let boundary = newline::Boundary::Statement;
    newline::before_token(parser, boundary, TokenKind::Keyword(Keyword::Is));
    if parser.eat_keyword(Keyword::Is) {
        newline::after_incomplete(parser, boundary);
        enumeration(parser);
    }
}

fn enumeration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Dot);
    parser.expect_name();
    while parser.eat_punctuation(Punctuation::Dot) {
        if !parser.expect_name() {
            break;
        }
    }
    if parser.at_punctuation(Punctuation::LeftParen) {
        let payload = parser.start();
        parser.bump();
        if parser.enter_nesting() {
            parser.comma_list(
                Punctuation::RightParen,
                false,
                ExpectedSyntax::BindingPattern,
                super::super::statement::binding_pattern,
            );
            parser.expect_punctuation(Punctuation::RightParen);
            parser.leave_nesting();
        } else {
            parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
        }
        parser.complete(payload, NodeKind::EnumPatternPayload);
    }
    parser.complete(marker, NodeKind::EnumPattern);
}

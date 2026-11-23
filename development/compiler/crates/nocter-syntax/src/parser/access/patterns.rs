use super::{Parser, expression::header_expression, newline};
use crate::{ContextualSpelling, ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn if_condition(parser: &mut Parser<'_>) {
    if !marked_condition(parser) {
        header_expression(parser);
        if !enum_separator(parser) {
            return;
        }
    }
    newline::after_incomplete(parser, newline::Boundary::Statement);
    enumeration(parser);
}

fn enum_separator(parser: &mut Parser<'_>) -> bool {
    newline::before_token(
        parser,
        newline::Boundary::Statement,
        TokenKind::Keyword(Keyword::Is),
    );
    parser.eat_keyword(Keyword::Is)
}

fn marked_condition(parser: &mut Parser<'_>) -> bool {
    if !super::at_input_operation(parser)
        || parser.tokens[parser.cursor].is_joint_to_next()
        || matches!(parser.nth_kind(1), TokenKind::Newline | TokenKind::Eof)
    {
        return false;
    }
    // A failed outer trial can re-enter nested if headers during ordinary expression parsing.
    // Retain failed trials at the same token and nesting depth to avoid repeating their branches.
    let site = (parser.cursor, parser.nesting);
    if parser.unmarked_conditions.contains(&site) {
        return false;
    }
    let selected = parser
        .attempt_with(
            |parser| {
                super::consume_input_operation(parser);
                header_expression(parser);
                enum_separator(parser)
            },
            |_, separated| *separated,
        )
        .is_some();
    if !selected {
        parser.unmarked_conditions.insert(site);
    }
    selected
}

fn enumeration(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_contextual(ContextualSpelling::UpperSelf) {
        parser.bump();
    } else {
        parser.expect_name();
    }
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

use super::{ExpressionMode, expression};
use crate::parser::{CompletedMarker, Parser, block, newline};
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

pub(super) fn if_expression(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.expect_keyword(Keyword::If);
    let condition = parser.start();
    expression(parser, ExpressionMode::Header);
    newline::before_token(
        parser,
        newline::Boundary::Statement,
        TokenKind::Keyword(Keyword::Is),
    );
    if parser.eat_keyword(Keyword::Is) {
        newline::after_incomplete(parser, newline::Boundary::Statement);
        enum_pattern(parser);
    }
    parser.complete(condition, NodeKind::IfCondition);
    block::required(parser);
    if parser.at_keyword(Keyword::Else) {
        let else_clause = parser.start();
        parser.bump();
        if parser.at_keyword(Keyword::If) {
            if_expression(parser);
        } else {
            block::required(parser);
        }
        parser.complete(else_clause, NodeKind::ElseClause);
    }
    parser.complete(marker, NodeKind::IfExpression)
}

pub(super) fn match_expression(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.expect_keyword(Keyword::Match);
    expression(parser, ExpressionMode::Header);
    parser.braced_line_sequence(ExpectedSyntax::EnumPattern, match_arm);
    parser.complete(marker, NodeKind::MatchExpression)
}

fn match_arm(parser: &mut Parser<'_>) {
    let marker = parser.start();
    if parser.at_identifier_text("_") {
        parser.bump();
    } else {
        enum_pattern(parser);
    }
    block::required(parser);
    parser.complete(marker, NodeKind::MatchArm);
}

fn enum_pattern(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name();
    parser.expect_punctuation(Punctuation::Dot);
    parser.expect_name();
    if parser.at_punctuation(Punctuation::LeftParen) {
        let payload = parser.start();
        parser.bump();
        if !parser.enter_nesting() {
            parser.recover_balanced(Punctuation::LeftParen, Punctuation::RightParen);
            parser.complete(payload, NodeKind::EnumPatternPayload);
            parser.complete(marker, NodeKind::EnumPattern);
            return;
        }
        parser.comma_list(
            Punctuation::RightParen,
            true,
            ExpectedSyntax::Name,
            payload_slot,
        );
        parser.expect_punctuation(Punctuation::RightParen);
        parser.leave_nesting();
        parser.complete(payload, NodeKind::EnumPatternPayload);
    }
    parser.complete(marker, NodeKind::EnumPattern);
}

fn payload_slot(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_name_or_discard();
    parser.complete(marker, NodeKind::PayloadSlot);
}

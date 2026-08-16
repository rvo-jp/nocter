use super::{CompletedMarker, Parser, root, statement};
use crate::{ExpectedSyntax, Keyword, NodeKind, ParseDiagnosticKind, Punctuation, TokenKind};

pub(super) fn optional(parser: &mut Parser<'_>) -> Option<CompletedMarker> {
    parser
        .at_punctuation(Punctuation::LeftBrace)
        .then(|| block(parser))
}

pub(super) fn required(parser: &mut Parser<'_>) -> CompletedMarker {
    if parser.at_punctuation(Punctuation::LeftBrace) {
        block(parser)
    } else {
        let marker = parser.start();
        parser.missing(ExpectedSyntax::Block);
        parser.complete(marker, NodeKind::Block)
    }
}

fn block(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
        return parser.complete(marker, NodeKind::Block);
    }

    parser.eat_newlines();
    parse_use_prefix(parser);
    if !parser.at_punctuation(Punctuation::RightBrace) && !parser.at(TokenKind::Eof) {
        executable_sequence(parser);
    }
    parser.eat_newlines();
    parser.expect_punctuation(Punctuation::RightBrace);
    parser.leave_nesting();
    parser.complete(marker, NodeKind::Block)
}

fn parse_use_prefix(parser: &mut Parser<'_>) {
    while parser.at_keyword(Keyword::Use) {
        block_use(parser);
        if parser.at_punctuation(Punctuation::RightBrace) || parser.at(TokenKind::Eof) {
            return;
        }
        if parser.eat_newlines() == 0 {
            parser.missing(ExpectedSyntax::Newline);
            parser.recover_until(&[
                TokenKind::Newline,
                TokenKind::Punctuation(Punctuation::RightBrace),
            ]);
            parser.eat_newlines();
        }
    }
}

fn block_use(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.expect_keyword(Keyword::Use);
    root::use_tree(parser);
    parser.complete(marker, NodeKind::BlockUseDeclaration);
}

fn executable_sequence(parser: &mut Parser<'_>) {
    let marker = parser.start();
    loop {
        if parser.at_keyword(Keyword::Use) {
            parser.diagnostic(ParseDiagnosticKind::LateUseDeclaration);
            block_use(parser);
        } else {
            let executable = statement::executable(parser);
            if executable.is_expression {
                let wrapper = parser.precede(executable.completed);
                let kind = if only_newlines_before_close(parser) {
                    NodeKind::BodyResult
                } else {
                    NodeKind::ExpressionStatement
                };
                parser.complete(wrapper, kind);
            }
        }

        if parser.at_punctuation(Punctuation::RightBrace) || parser.at(TokenKind::Eof) {
            break;
        }
        if parser.eat_newlines() == 0 {
            parser.missing(ExpectedSyntax::Newline);
            parser.recover_until(&[
                TokenKind::Newline,
                TokenKind::Punctuation(Punctuation::RightBrace),
            ]);
            parser.eat_newlines();
        }
        if parser.at_punctuation(Punctuation::RightBrace) || parser.at(TokenKind::Eof) {
            break;
        }
    }
    parser.complete(marker, NodeKind::ExecutableSequence);
}

fn only_newlines_before_close(parser: &Parser<'_>) -> bool {
    let mut distance = 0;
    while parser.nth_kind(distance) == TokenKind::Newline {
        distance += 1;
    }
    parser.nth_kind(distance) == TokenKind::Punctuation(Punctuation::RightBrace)
}

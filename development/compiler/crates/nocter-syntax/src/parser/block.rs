use super::Parser;
use crate::{ExpectedSyntax, NodeKind, Punctuation, TokenKind};

pub(super) fn optional(parser: &mut Parser<'_>) {
    if parser.at_punctuation(Punctuation::LeftBrace) {
        block(parser);
    }
}

pub(super) fn required(parser: &mut Parser<'_>) {
    if parser.at_punctuation(Punctuation::LeftBrace) {
        block(parser);
    } else {
        parser.missing(ExpectedSyntax::Block);
    }
}

fn block(parser: &mut Parser<'_>) {
    let marker = parser.start();
    parser.bump();
    if !parser.enter_nesting() {
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
        parser.complete(marker, NodeKind::Block);
        return;
    }

    parser.eat_newlines();
    if parser.at_punctuation(Punctuation::RightBrace) {
        parser.bump();
    } else if parser.at(TokenKind::Eof) {
        parser.missing(ExpectedSyntax::Punctuation(Punctuation::RightBrace));
    } else {
        // Executable block grammar is implemented as its own later conformance boundary. Consume
        // the complete owning block now so an unsupported body cannot corrupt its declaration's
        // surrounding line sequence.
        parser.diagnostic(crate::ParseDiagnosticKind::Expected(
            ExpectedSyntax::Punctuation(Punctuation::RightBrace),
        ));
        parser.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
    }
    parser.leave_nesting();
    parser.complete(marker, NodeKind::Block);
}

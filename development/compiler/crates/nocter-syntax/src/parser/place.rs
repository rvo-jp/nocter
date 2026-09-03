use super::{CompletedMarker, Parser};
use crate::{NodeKind, Punctuation, TokenKind};

pub(super) fn named(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.expect_name();
    while parser.eat_punctuation(Punctuation::Dot) {
        let suffix = parser.start();
        let kind = if parser.at(TokenKind::IntegerLiteral) {
            parser.bump();
            NodeKind::TupleElementSuffix
        } else {
            parser.expect_name();
            NodeKind::MemberSuffix
        };
        parser.complete(suffix, kind);
    }
    parser.complete(marker, NodeKind::NamedPlace)
}

pub(super) fn allocator(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    named(parser);
    parser.complete(marker, NodeKind::AllocatorPlace)
}

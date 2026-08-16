use super::{CompletedMarker, Parser};
use crate::{NodeKind, Punctuation};

pub(super) fn named(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    parser.expect_name();
    while parser.eat_punctuation(Punctuation::Dot) {
        parser.expect_name();
    }
    parser.complete(marker, NodeKind::NamedPlace)
}

pub(super) fn allocator(parser: &mut Parser<'_>) -> CompletedMarker {
    let marker = parser.start();
    named(parser);
    parser.complete(marker, NodeKind::AllocatorPlace)
}

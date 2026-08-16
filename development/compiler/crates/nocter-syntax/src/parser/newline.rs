use super::Parser;
use crate::TokenKind;

/// The delimiter boundary that owns a possible continuation newline.
///
/// Statement-level expressions admit exactly one physical newline and never cross a blank line.
/// A delimited expression admits every newline owned by that delimiter.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Boundary {
    Statement,
    Delimited,
}

pub(super) fn before(
    parser: &mut Parser<'_>,
    boundary: Boundary,
    is_leader: impl Fn(TokenKind) -> bool,
) {
    let count = newline_count(parser);
    if count == 0 || !is_leader(parser.nth_kind(count)) || !is_admitted(boundary, count) {
        return;
    }
    consume(parser, count);
}

pub(super) fn before_token(parser: &mut Parser<'_>, boundary: Boundary, leader: TokenKind) {
    before(parser, boundary, |kind| kind == leader);
}

pub(super) fn after_incomplete(parser: &mut Parser<'_>, boundary: Boundary) {
    let count = newline_count(parser);
    if is_admitted(boundary, count) {
        consume(parser, count);
    }
}

fn newline_count(parser: &Parser<'_>) -> usize {
    let mut count = 0;
    while parser.nth_kind(count) == TokenKind::Newline {
        count += 1;
    }
    count
}

fn is_admitted(boundary: Boundary, count: usize) -> bool {
    count > 0 && (boundary == Boundary::Delimited || count == 1)
}

fn consume(parser: &mut Parser<'_>, count: usize) {
    for _ in 0..count {
        parser.bump();
    }
}

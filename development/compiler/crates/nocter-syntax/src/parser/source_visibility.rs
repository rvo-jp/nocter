use super::Parser;
use crate::{ExpectedSyntax, Keyword, NodeKind, Punctuation, TokenKind};

/// Parses one exact relative physical source path used for direct source visibility.
///
/// Source-visibility paths deliberately have no shared production with module paths. This keeps `.nct`
/// required here while making it impossible for `use` parsing to reinterpret a source path.
pub(super) fn declaration(parser: &mut Parser<'_>) {
    let declaration = parser.start();
    parser.expect_keyword(Keyword::See);

    let path = parser.start();
    source_path_prefix(parser);
    expect_source_segment(parser);
    while parser.eat_punctuation(Punctuation::Slash) {
        expect_source_segment(parser);
    }
    parser.expect_punctuation(Punctuation::Dot);
    parser.expect_identifier_text("nct");
    parser.complete(path, NodeKind::SourceVisibilityPath);

    parser.complete(declaration, NodeKind::SourceVisibilityDeclaration);
}

fn source_path_prefix(parser: &mut Parser<'_>) {
    loop {
        parser.expect_punctuation(Punctuation::Dot);
        if parser.eat_punctuation(Punctuation::Dot) {
            parser.expect_punctuation(Punctuation::Slash);
            if parser.at_punctuation(Punctuation::Dot) {
                continue;
            }
            return;
        }
        parser.expect_punctuation(Punctuation::Slash);
        return;
    }
}

fn expect_source_segment(parser: &mut Parser<'_>) -> bool {
    if parser.at(TokenKind::Identifier) && is_source_segment(parser.current_text()) {
        parser.bump();
        true
    } else {
        parser.error_token(ExpectedSyntax::ModuleSegment);
        false
    }
}

fn is_source_segment(text: &str) -> bool {
    text != "_"
        && text.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

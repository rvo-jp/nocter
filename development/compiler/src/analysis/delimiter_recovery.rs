//! Syntax-neutral delimiter recovery for editor-only analysis overlays.

use crate::lexer::{Token, TokenKind, lex};
use crate::source::SourceMap;

pub(crate) fn block_recovery_text(text: &str, offset: usize) -> Option<String> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    close_unmatched_braces(text)
}

pub(crate) fn close_unmatched_braces(text: &str) -> Option<String> {
    let missing = unmatched_open_braces(&tokens(text));
    if missing == 0 {
        return None;
    }
    let mut recovered = String::with_capacity(text.len() + missing);
    recovered.push_str(text);
    recovered.extend(std::iter::repeat_n('}', missing));
    Some(recovered)
}

pub(crate) fn unmatched_open_braces(tokens: &[Token]) -> usize {
    tokens.iter().fold(0usize, |depth, token| {
        if punctuation(token, "{") {
            depth + 1
        } else if punctuation(token, "}") {
            depth.saturating_sub(1)
        } else {
            depth
        }
    })
}

fn tokens(text: &str) -> Vec<Token> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("delimiter-recovery.nct", None, text.to_string());
    lex(&sources, source).tokens
}

fn punctuation(token: &Token, expected: &str) -> bool {
    matches!(token.kind, TokenKind::Punctuation(actual) if actual == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_nested_blocks_without_counting_braces_in_strings_or_comments() {
        let text = "func main(): i32 {\n    let text = \"{\" // }\n    if true {\n        1\n";
        let recovered = block_recovery_text(text, text.len()).expect("expected recovery");

        assert!(recovered.ends_with("}}"), "{recovered}");
    }

    #[test]
    fn leaves_balanced_source_unchanged() {
        assert!(block_recovery_text("func main(): i32 { 0 }", 0).is_none());
    }
}

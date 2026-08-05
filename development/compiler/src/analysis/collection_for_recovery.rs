//! Source overlays for incomplete `for binding in source` headers.

use crate::lexer::{Keyword, Token, TokenKind, lex};
use crate::source::SourceMap;

const SOURCE_PLACEHOLDER: &str = "__nocter_iteration_source_placeholder";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionForRecovery {
    pub(crate) text: String,
    pub(crate) cursor: usize,
    pub(crate) insertion_start: usize,
    pub(crate) insertion_len: usize,
}

pub(crate) fn collection_for_recovery_text(text: &str, offset: usize) -> Option<String> {
    collection_for_recovery_overlay(text, offset).map(|recovery| recovery.text)
}

pub(crate) fn collection_for_recovery_overlay(
    text: &str,
    offset: usize,
) -> Option<CollectionForRecovery> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }

    let tokens = tokens(text);
    let for_index = tokens.iter().enumerate().rev().find_map(|(index, token)| {
        (token.span.start <= offset && token.kind == TokenKind::Keyword(Keyword::For))
            .then_some(index)
    })?;
    let header_tokens = tokens
        .iter()
        .skip(for_index + 1)
        .take_while(|token| token.span.end <= offset)
        .collect::<Vec<_>>();
    if header_tokens
        .iter()
        .any(|token| matches!(token.kind, TokenKind::Newline))
    {
        return None;
    }
    let header = header_tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .collect::<Vec<_>>();
    let [binding, in_token, source @ ..] = header.as_slice() else {
        return None;
    };
    if binding.kind != TokenKind::Identifier
        || in_token.kind != TokenKind::Keyword(Keyword::In)
        || source.iter().any(|token| punctuation(token, "{"))
    {
        return None;
    }

    let source_needs_placeholder = source.is_empty()
        || matches!(source, [token] if punctuation(token, "&"))
        || matches!(source, [token] if token.kind == TokenKind::Keyword(Keyword::Move))
        || source.last().is_some_and(|token| punctuation(token, "."));
    let mut insertion = String::new();
    if source_needs_placeholder {
        if needs_separator(text, offset) {
            insertion.push(' ');
        }
        insertion.push_str(SOURCE_PLACEHOLDER);
    }
    if next_non_whitespace(text, offset) != Some(b'{') {
        insertion.push_str(" {}");
    }
    if insertion.is_empty() {
        return None;
    }

    let mut recovered = String::with_capacity(text.len() + insertion.len());
    recovered.push_str(&text[..offset]);
    recovered.push_str(&insertion);
    recovered.push_str(&text[offset..]);
    Some(CollectionForRecovery {
        text: recovered,
        cursor: offset,
        insertion_start: offset,
        insertion_len: insertion.len(),
    })
}

pub(crate) fn collection_for_document_recovery(text: &str) -> Option<CollectionForRecovery> {
    let mut line_end = text.len();
    loop {
        if let Some(recovery) = collection_for_recovery_overlay(text, line_end) {
            return Some(recovery);
        }
        let prefix = text.get(..line_end)?;
        let newline = prefix.rfind('\n')?;
        line_end = newline;
        if line_end > 0 && text.as_bytes().get(line_end - 1) == Some(&b'\r') {
            line_end -= 1;
        }
    }
}

fn tokens(text: &str) -> Vec<Token> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("collection-for-recovery.nct", None, text.to_string());
    lex(&sources, source).tokens
}

fn punctuation(token: &Token, expected: &str) -> bool {
    matches!(token.kind, TokenKind::Punctuation(actual) if actual == expected)
}

fn needs_separator(text: &str, offset: usize) -> bool {
    text.as_bytes()
        .get(..offset)
        .and_then(|prefix| prefix.last())
        .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'&' && *byte != b'.')
}

fn next_non_whitespace(text: &str, offset: usize) -> Option<u8> {
    text.as_bytes()
        .get(offset..)?
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplies_a_source_and_body_after_in() {
        let text = "func run(values: Vec<i32>): void {\n    for item in \n}\n";
        let offset = text.find("in \n").unwrap() + "in ".len();
        let recovered = collection_for_recovery_overlay(text, offset).unwrap();
        assert!(
            recovered
                .text
                .contains("for item in __nocter_iteration_source_placeholder {}\n")
        );
        assert_eq!(recovered.cursor, offset);
    }

    #[test]
    fn supplies_an_operand_after_readonly_borrow() {
        let text = "func run(values: Vec<i32>): void {\n    for item in &\n}\n";
        let offset = text.find("&\n").unwrap() + 1;
        let recovered = collection_for_recovery_overlay(text, offset).unwrap();
        assert!(
            recovered
                .text
                .contains("for item in &__nocter_iteration_source_placeholder {}\n")
        );
    }

    #[test]
    fn preserves_a_partial_source_and_adds_only_the_body() {
        let text = "func run(values: Vec<i32>): void {\n    for item in val\n}\n";
        let offset = text.find("val\n").unwrap() + 3;
        let recovered = collection_for_recovery_overlay(text, offset).unwrap();
        assert!(recovered.text.contains("for item in val {}\n"));
    }
}

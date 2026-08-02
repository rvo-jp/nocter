//! Recovery overlays for incomplete lexical-region edits.

use crate::lexer::{Keyword, Token, TokenKind, lex};
use crate::source::SourceMap;

const REGION_ALLOCATOR_PLACEHOLDER: &str = "__nocter_region_allocator_placeholder";

pub(crate) fn region_recovery_text(text: &str, offset: usize) -> Option<String> {
    region_recovery_overlay(text, offset).map(|(text, _)| text)
}

pub(crate) fn region_recovery_overlay(text: &str, offset: usize) -> Option<(String, usize)> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }

    let mut sources = SourceMap::new();
    let source = sources.add_source("region-recovery.nct", None, text.to_string());
    let tokens = lex(&sources, source).tokens;
    let region = latest_keyword_before(&tokens, Keyword::Region, offset)?;
    let using = tokens
        .iter()
        .enumerate()
        .skip(region + 1)
        .find_map(|(index, token)| {
            matches!(token.kind, TokenKind::Keyword(Keyword::Using)).then_some(index)
        })?;
    let body_open = tokens
        .iter()
        .enumerate()
        .skip(using + 1)
        .find_map(|(index, token)| punctuation(token, "{").then_some(index));

    let cursor_edits_header = offset >= tokens[using].span.end
        && body_open.is_none_or(|body_open| tokens[body_open].span.start > offset);
    if !cursor_edits_header && let Some(body_open) = body_open {
        if region_body_closed_before(&tokens, body_open, offset) {
            return None;
        }
        let missing = unmatched_open_braces(&tokens);
        if missing == 0 {
            return None;
        }
        let mut recovered = String::with_capacity(text.len() + missing);
        recovered.push_str(text);
        recovered.extend(std::iter::repeat_n('}', missing));
        return Some((recovered, offset));
    }

    let allocator_tokens = tokens
        .iter()
        .skip(using + 1)
        .filter(|token| token.span.end <= offset && !matches!(token.kind, TokenKind::Newline))
        .collect::<Vec<_>>();
    let needs_placeholder = allocator_tokens.is_empty()
        || allocator_tokens
            .last()
            .is_some_and(|token| punctuation(token, "."));
    let has_following_body = text
        .get(offset..)
        .and_then(|suffix| suffix.bytes().find(|byte| !byte.is_ascii_whitespace()))
        == Some(b'{');

    let mut insertion = String::new();
    let mut recovered_offset = offset;
    if needs_placeholder {
        if allocator_tokens.is_empty()
            && text
                .as_bytes()
                .get(..offset)
                .and_then(|prefix| prefix.last())
                .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            insertion.push(' ');
            recovered_offset += 1;
        }
        insertion.push_str(REGION_ALLOCATOR_PLACEHOLDER);
    }
    if !has_following_body {
        insertion.push_str(" {}");
    }
    if insertion.is_empty() {
        return None;
    }

    let missing = unmatched_open_braces(&tokens);
    let mut recovered = String::with_capacity(text.len() + insertion.len() + missing);
    recovered.push_str(&text[..offset]);
    recovered.push_str(&insertion);
    recovered.push_str(&text[offset..]);
    recovered.extend(std::iter::repeat_n('}', missing));
    Some((recovered, recovered_offset))
}

fn latest_keyword_before(tokens: &[Token], keyword: Keyword, offset: usize) -> Option<usize> {
    tokens.iter().enumerate().rev().find_map(|(index, token)| {
        (token.span.start <= offset && token.kind == TokenKind::Keyword(keyword)).then_some(index)
    })
}

fn region_body_closed_before(tokens: &[Token], body_open: usize, offset: usize) -> bool {
    let mut depth = 0usize;
    for token in tokens.iter().skip(body_open) {
        if token.span.end > offset {
            break;
        }
        if punctuation(token, "{") {
            depth += 1;
        } else if punctuation(token, "}") {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return true;
            }
        }
    }
    false
}

fn unmatched_open_braces(tokens: &[Token]) -> usize {
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

fn punctuation(token: &Token, expected: &str) -> bool {
    matches!(token.kind, TokenKind::Punctuation(actual) if actual == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_missing_allocator_and_region_body() {
        let text = "func run(parent: Arena): void {\n    region temp using \n}\n";
        let offset = text.find("using ").unwrap() + "using ".len();
        let recovered = region_recovery_text(text, offset).expect("expected recovery");

        assert!(
            recovered.contains("region temp using __nocter_region_allocator_placeholder {}"),
            "{recovered}"
        );
    }

    #[test]
    fn closes_incomplete_region_and_enclosing_function_body() {
        let text = "func run(parent: Arena): void {\n    region temp using parent {\n        let value = 1\n";
        let offset = text.len();
        let recovered = region_recovery_text(text, offset).expect("expected recovery");

        assert!(recovered.ends_with("}}"), "{recovered}");
    }
}

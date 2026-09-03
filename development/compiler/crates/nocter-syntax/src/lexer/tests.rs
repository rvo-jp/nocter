use std::collections::BTreeSet;

use nocter_source::{SourceMap, SourceName};

use super::*;

fn lex_text(text: &str) -> LexedFile {
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("test.nct"), text.as_bytes())
        .unwrap();
    lex(sources.get(id).unwrap())
}

fn kinds(text: &str) -> Vec<TokenKind> {
    lex_text(text).tokens.into_iter().map(Token::kind).collect()
}

#[test]
fn recognizes_reserved_and_contextual_spellings_separately() {
    assert_eq!(
        kinds("interface where some self drop"),
        vec![
            TokenKind::Keyword(Keyword::Interface),
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn keyword_enum_matches_the_normative_specification_list() {
    let lexical_spec = include_str!("../../../../../../spec/13-lexical-grammar.md");
    let keyword_block = lexical_spec
        .split_once("Reserved keyword tokens:\n\n```text\n")
        .unwrap()
        .1
        .split_once("\n```")
        .unwrap()
        .0;
    let specified: BTreeSet<_> = keyword_block.lines().collect();
    let implemented: BTreeSet<_> = Keyword::ALL
        .iter()
        .map(|keyword| keyword.as_str())
        .collect();

    assert_eq!(implemented, specified);
}

#[test]
fn invalid_escape_reports_once() {
    let lexed = lex_text("\"bad \\q escape\"");

    assert_eq!(
        lexed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.kind)
            .collect::<Vec<_>>(),
        vec![LexDiagnosticKind::InvalidEscape]
    );
}

#[test]
fn invalid_unicode_escapes_preserve_source_character_boundaries() {
    for source in [
        "\"bad \\β escape\" name",
        "\"bad \\x界 escape\" name",
        "b'\\β' name",
    ] {
        let lexed = lex_text(source);

        assert_eq!(
            lexed
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.kind == LexDiagnosticKind::InvalidEscape)
                .count(),
            1,
            "{source:?}"
        );
        assert!(lexed.tokens.iter().any(|token| {
            token.kind() == TokenKind::Identifier
                && token.span().range().end() == offset(source.len())
        }));
    }
}

#[test]
fn uses_longest_match_for_punctuation() {
    assert_eq!(
        kinds("&+ ..< ... == != <= >= && || << >> += -= *= /= %="),
        vec![
            TokenKind::Punctuation(Punctuation::ReadWrite),
            TokenKind::Punctuation(Punctuation::Range),
            TokenKind::Punctuation(Punctuation::Expansion),
            TokenKind::Punctuation(Punctuation::EqualEqual),
            TokenKind::Punctuation(Punctuation::BangEqual),
            TokenKind::Punctuation(Punctuation::LessEqual),
            TokenKind::Punctuation(Punctuation::GreaterEqual),
            TokenKind::Punctuation(Punctuation::LogicalAnd),
            TokenKind::Punctuation(Punctuation::LogicalOr),
            TokenKind::Punctuation(Punctuation::ShiftLeft),
            TokenKind::Punctuation(Punctuation::ShiftRight),
            TokenKind::Punctuation(Punctuation::PlusEqual),
            TokenKind::Punctuation(Punctuation::MinusEqual),
            TokenKind::Punctuation(Punctuation::StarEqual),
            TokenKind::Punctuation(Punctuation::SlashEqual),
            TokenKind::Punctuation(Punctuation::PercentEqual),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn every_punctuation_variant_is_lexable_from_its_canonical_spelling() {
    for punctuation in Punctuation::ALL {
        assert_eq!(
            kinds(punctuation.as_str()),
            vec![TokenKind::Punctuation(*punctuation), TokenKind::Eof]
        );
    }
}

#[test]
fn records_joint_tokens_without_reconstructing_whitespace() {
    let lexed = lex_text("Vec[0] Vec [0] Vec/* gap */[0]");
    let identifiers: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|token| token.kind() == TokenKind::Identifier)
        .copied()
        .collect();

    assert!(identifiers[0].is_joint_to_next());
    assert!(!identifiers[1].is_joint_to_next());
    assert!(!identifiers[2].is_joint_to_next());
}

#[test]
fn preserves_newlines_from_line_and_block_comments() {
    assert_eq!(
        kinds("a // comment\nb/* first\nsecond */c"),
        vec![
            TokenKind::Identifier,
            TokenKind::Newline,
            TokenKind::Identifier,
            TokenKind::Newline,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn classifies_documentation_comments_without_emitting_comment_tokens() {
    let lexed =
        lex_text("//! file\n/// item\n//// plain\n/*! file */\n/** item */\n/**/\n/*** plain */");
    assert_eq!(
        lexed
            .comments
            .iter()
            .map(|comment| comment.kind)
            .collect::<Vec<_>>(),
        vec![
            CommentKind::FileDocumentation,
            CommentKind::ItemDocumentation,
            CommentKind::Line,
            CommentKind::FileDocumentation,
            CommentKind::ItemDocumentation,
            CommentKind::Block,
            CommentKind::Block,
        ]
    );
}

#[test]
fn tokenizes_nested_strings_and_braces_in_interpolation() {
    assert_eq!(
        kinds("\"value: ${render(User { id: \"x\" })}\""),
        vec![
            TokenKind::StringStart(StringDelimiter::SingleLine),
            TokenKind::StringText,
            TokenKind::InterpolationStart,
            TokenKind::Identifier,
            TokenKind::Punctuation(Punctuation::LeftParen),
            TokenKind::Identifier,
            TokenKind::Punctuation(Punctuation::LeftBrace),
            TokenKind::Identifier,
            TokenKind::Punctuation(Punctuation::Colon),
            TokenKind::StringStart(StringDelimiter::SingleLine),
            TokenKind::StringText,
            TokenKind::StringEnd(StringDelimiter::SingleLine),
            TokenKind::Punctuation(Punctuation::RightBrace),
            TokenKind::Punctuation(Punctuation::RightParen),
            TokenKind::InterpolationEnd,
            TokenKind::StringEnd(StringDelimiter::SingleLine),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn validates_integer_and_byte_boundaries() {
    let lexed = lex_text("0xFF 0b1010 1_000 0x_1 1e3 b'a' b'ab' b'\\xFF'");
    assert_eq!(
        lexed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.kind)
            .collect::<Vec<_>>(),
        vec![
            LexDiagnosticKind::InvalidIntegerLiteral,
            LexDiagnosticKind::UnsupportedFloatLiteral,
            LexDiagnosticKind::InvalidByteLength,
        ]
    );
}

#[test]
fn validates_multiline_indentation() {
    let valid = lex_text("\"\"\"\n    first\n    second\n    \"\"\"");
    assert!(valid.diagnostics.is_empty());

    let invalid = lex_text("\"\"\"\n    first\n  second\n    \"\"\"");
    assert_eq!(
        invalid
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.kind)
            .collect::<Vec<_>>(),
        vec![LexDiagnosticKind::MultilineStringIndentation]
    );
}

#[test]
fn escaped_interpolation_remains_string_text() {
    assert_eq!(
        kinds("\"literal \\${value}\""),
        vec![
            TokenKind::StringStart(StringDelimiter::SingleLine),
            TokenKind::StringText,
            TokenKind::StringEnd(StringDelimiter::SingleLine),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn numeric_member_spelling_is_lexed_as_projection_only_after_a_value() {
    let projection = lex_text("value.1.0");
    assert!(projection.diagnostics.is_empty());
    assert_eq!(
        projection
            .tokens
            .iter()
            .map(|token| token.kind())
            .collect::<Vec<_>>(),
        [
            TokenKind::Identifier,
            TokenKind::Punctuation(Punctuation::Dot),
            TokenKind::IntegerLiteral,
            TokenKind::Punctuation(Punctuation::Dot),
            TokenKind::IntegerLiteral,
            TokenKind::Eof,
        ]
    );

    assert_eq!(
        lex_text("1.0").diagnostics[0].kind,
        LexDiagnosticKind::UnsupportedFloatLiteral
    );
    assert_eq!(
        lex_text(".5").diagnostics[0].kind,
        LexDiagnosticKind::UnsupportedFloatLiteral
    );
}

#[test]
fn reports_closed_lexical_error_classes_without_duplicate_eof() {
    let cases = [
        ("/*", LexDiagnosticKind::UnterminatedBlockComment),
        ("\"text", LexDiagnosticKind::UnterminatedString),
        ("\"line\n", LexDiagnosticKind::SingleLineStringNewline),
        (
            "\"\"\"same line",
            LexDiagnosticKind::MultilineStringOpeningNewline,
        ),
        ("b'ab", LexDiagnosticKind::UnterminatedByteLiteral),
        ("b'a\n", LexDiagnosticKind::ByteLiteralNewline),
        ("'", LexDiagnosticKind::PlainSingleQuote),
        ("@", LexDiagnosticKind::UnexpectedCharacter),
        (".5", LexDiagnosticKind::UnsupportedFloatLiteral),
    ];

    for (source, expected) in cases {
        let lexed = lex_text(source);
        assert_eq!(
            lexed.diagnostics.first().map(|diagnostic| diagnostic.kind),
            Some(expected)
        );
        assert_eq!(
            lexed
                .tokens
                .iter()
                .filter(|token| token.kind() == TokenKind::Eof)
                .count(),
            1
        );
    }
}

#[test]
fn reports_unterminated_interpolation_at_normalized_eof() {
    let lexed = lex_text("\"value: ${call()\"");

    assert!(
        lexed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == LexDiagnosticKind::UnterminatedInterpolation)
    );
    let eof = lexed.tokens.last().unwrap();
    assert_eq!(eof.kind(), TokenKind::Eof);
    assert!(eof.span().range().is_empty());
}

use super::*;
use crate::source::SourceMap;

#[test]
fn lexes_keywords_newlines_and_eof() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, "func main(): i32 {\n    return 0\n}\n");
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.tokens[0].kind, TokenKind::Keyword(Keyword::Func));
    assert!(
        output
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Newline)
    );
    assert_eq!(output.tokens.last().unwrap().kind, TokenKind::Eof);
}

#[test]
fn lexes_program_as_identifier() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, "func program(): i32 {}");
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.tokens[1].kind, TokenKind::Identifier);
}

#[test]
fn lexes_drop_as_identifier() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, "drop file");
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.tokens[0].kind, TokenKind::Identifier);
    assert_eq!(output.tokens[1].kind, TokenKind::Identifier);
}

#[test]
fn lexes_copy_as_identifier() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, "String.copy");
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.tokens[0].kind, TokenKind::Identifier);
    assert_eq!(output.tokens[2].kind, TokenKind::Identifier);
}

#[test]
fn lexes_trait_as_identifier() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, "func trait(): void {}");
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.tokens[1].kind, TokenKind::Identifier);
}

#[test]
fn keyword_lexemes_match_lexer_keywords() {
    for keyword_text in KEYWORD_LEXEMES {
        let mut sources = SourceMap::new();
        let id = sources.add_source("app.nct", None, (*keyword_text).to_string());
        let output = lex(&sources, id);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert!(
            matches!(output.tokens[0].kind, TokenKind::Keyword(_)),
            "`{keyword_text}` should lex as a keyword"
        );
    }

    assert!(!KEYWORD_LEXEMES.contains(&"drop"));
    assert!(!KEYWORD_LEXEMES.contains(&"copy"));
    assert!(!KEYWORD_LEXEMES.contains(&"trait"));
    assert!(!KEYWORD_LEXEMES.contains(&"from"));
    assert!(!KEYWORD_LEXEMES.contains(&"import"));
    assert!(KEYWORD_LEXEMES.contains(&"interface"));
    assert!(KEYWORD_LEXEMES.contains(&"test"));
}

#[test]
fn impl_is_an_identifier_after_the_declaration_split() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, "impl instance conform".to_string());
    let output = lex(&sources, id);

    assert!(matches!(output.tokens[0].kind, TokenKind::Identifier));
    assert_eq!(output.tokens[1].kind, TokenKind::Keyword(Keyword::Instance));
    assert_eq!(output.tokens[2].kind, TokenKind::Keyword(Keyword::Conform));
}

#[test]
fn validates_identifier_names() {
    assert!(is_valid_identifier_name("main"));
    assert!(is_valid_identifier_name("_entry2"));
    assert!(is_valid_identifier_name("program"));
    assert!(is_valid_identifier_name("copy"));
    assert!(is_valid_identifier_name("drop"));
    assert!(is_valid_identifier_name("trait"));
    assert!(!is_valid_identifier_name("interface"));
    assert!(!is_valid_identifier_name("test"));
    assert!(!is_valid_identifier_name(""));
    assert!(!is_valid_identifier_name("2main"));
    assert!(!is_valid_identifier_name("main-entry"));
    assert!(!is_valid_identifier_name("func"));
}

#[test]
fn skips_comments() {
    let mut sources = SourceMap::new();
    let id = sources.add_source(
        "app.nct",
        None,
        "let a = 1 // comment\n/* block */\nlet b = 2",
    );
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        !output
            .tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Punctuation("/*" | "*/" | "//")))
    );
}

#[test]
fn lexes_half_open_range_punctuation() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, "for i in 0..<4 {}");
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Punctuation("..<"))
    );
}

#[test]
fn lexes_match_keyword_and_switch_identifier() {
    let mut sources = SourceMap::new();
    let id = sources.add_source(
        "app.nct",
        None,
        "match value {}\nlet switch = 1\nlet try = 2",
    );
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.tokens[0].kind, TokenKind::Keyword(Keyword::Match));
    assert!(output.tokens.iter().any(|token| {
        token.kind == TokenKind::Identifier
            && sources
                .get(token.span.source)
                .and_then(|file| file.text().get(token.span.start..token.span.end))
                == Some("switch")
    }));
    assert!(output.tokens.iter().any(|token| {
        token.kind == TokenKind::Identifier
            && sources
                .get(token.span.source)
                .and_then(|file| file.text().get(token.span.start..token.span.end))
                == Some("try")
    }));
}

#[test]
fn diagnoses_float_literals() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, "let value = 1.0");
    let output = lex(&sources, id);

    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("float literals"));
}

#[test]
fn lexes_multi_line_string_literal_as_one_token() {
    let mut sources = SourceMap::new();
    let id = sources.add_source(
        "app.nct",
        None,
        "let text = \"\"\"\n    alpha\n    beta\n    \"\"\"\nlet done = true",
    );
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let string_tokens = output
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::StringLiteral)
        .collect::<Vec<_>>();
    assert_eq!(string_tokens.len(), 1);
    assert_eq!(
        output
            .tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Newline)
            .count(),
        1
    );
    assert_eq!(
        sources
            .get(string_tokens[0].span.source)
            .and_then(|file| file
                .text()
                .get(string_tokens[0].span.start..string_tokens[0].span.end)),
        Some("\"\"\"\n    alpha\n    beta\n    \"\"\"")
    );
}

#[test]
fn diagnoses_multi_line_string_indent_mismatch() {
    let mut sources = SourceMap::new();
    let id = sources.add_source(
        "app.nct",
        None,
        "let text = \"\"\"\n    alpha\n  beta\n    \"\"\"",
    );
    let output = lex(&sources, id);

    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("indentation"));
}

#[test]
fn allows_escaped_dollar_in_string_literal() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, r#"let text = "hello \${name}""#);
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::StringLiteral)
    );
}

#[test]
fn lexes_string_interpolation_source_form() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, r#"let text = "hello ${name}""#);
    let output = lex(&sources, id);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::StringLiteral)
    );
}

#[test]
fn diagnoses_unterminated_string_interpolation() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, r#"let text = "hello ${name""#);
    let output = lex(&sources, id);

    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("interpolation"));
}

#[test]
fn lexes_byte_span_with_original_offsets() {
    let mut sources = SourceMap::new();
    let id = sources.add_source("app.nct", None, r#"let text = "hello ${name}""#);
    let output = lex_span(&sources, ByteSpan::new(id, 20, 24));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.tokens[0].kind, TokenKind::Identifier);
    assert_eq!(output.tokens[0].span, ByteSpan::new(id, 20, 24));
    assert_eq!(
        output.tokens.last().unwrap().span,
        ByteSpan::new(id, 24, 24)
    );
}

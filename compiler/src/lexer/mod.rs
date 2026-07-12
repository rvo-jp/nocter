//! Tokenization for `.nct` source files.

use crate::diagnostics::Diagnostic;
use crate::literals::{find_interpolation_end, validate_string_literal_source};
use crate::source::{ByteSpan, JsonSpan, SourceId, SourceMap};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Keyword(Keyword),
    IntegerLiteral,
    StringLiteral,
    ByteLiteral,
    Newline,
    Punctuation(&'static str),
    Eof,
}

impl TokenKind {
    pub fn json_kind(&self) -> &'static str {
        match self {
            TokenKind::Identifier => "identifier",
            TokenKind::Keyword(_) => "keyword",
            TokenKind::IntegerLiteral => "integer_literal",
            TokenKind::StringLiteral => "string_literal",
            TokenKind::ByteLiteral => "byte_literal",
            TokenKind::Newline => "newline",
            TokenKind::Punctuation(_) => "punctuation",
            TokenKind::Eof => "eof",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    From,
    Import,
    Use,
    Func,
    Pub,
    Type,
    Copy,
    Struct,
    Enum,
    Trait,
    Impl,
    Method,
    Let,
    Var,
    Return,
    If,
    Else,
    For,
    In,
    While,
    Loop,
    Break,
    Continue,
    Match,
    Is,
    Catch,
    None,
    True,
    False,
    Move,
    As,
    Region,
    Using,
    Primitive,
    Void,
    Never,
}

pub(crate) const KEYWORD_LEXEMES: &[&str] = &[
    "from",
    "import",
    "use",
    "func",
    "pub",
    "type",
    "copy",
    "struct",
    "enum",
    "trait",
    "impl",
    "method",
    "let",
    "var",
    "return",
    "if",
    "else",
    "for",
    "in",
    "while",
    "loop",
    "break",
    "continue",
    "match",
    "is",
    "catch",
    "none",
    "true",
    "false",
    "move",
    "as",
    "region",
    "using",
    "primitive",
    "void",
    "never",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(sources: &SourceMap, source: SourceId) -> LexOutput {
    let Some(file) = sources.get(source) else {
        return LexOutput {
            tokens: Vec::new(),
            diagnostics: vec![Diagnostic::error(
                "E0100",
                format!("unknown source id {}", source.raw()),
            )],
        };
    };

    lex_range(sources, source, 0, file.text().len())
}

pub fn lex_span(sources: &SourceMap, span: ByteSpan) -> LexOutput {
    lex_range(sources, span.source, span.start, span.end)
}

fn lex_range(sources: &SourceMap, source: SourceId, start: usize, end: usize) -> LexOutput {
    let Some(file) = sources.get(source) else {
        return LexOutput {
            tokens: Vec::new(),
            diagnostics: vec![Diagnostic::error(
                "E0100",
                format!("unknown source id {}", source.raw()),
            )],
        };
    };

    let text = file.text();
    let bytes = text.as_bytes();
    if start > end || end > bytes.len() {
        return LexOutput {
            tokens: Vec::new(),
            diagnostics: vec![Diagnostic::error(
                "E0100",
                format!("invalid lexer byte range {start}..{end}"),
            )],
        };
    }

    let mut lexer = Lexer {
        sources,
        source,
        text,
        bytes,
        index: start,
        end,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    };

    lexer.run();

    LexOutput {
        tokens: lexer.tokens,
        diagnostics: lexer.diagnostics,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonToken {
    pub kind: String,
    pub lexeme: String,
    pub span: JsonSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TokensEnvelope {
    pub schema: &'static str,
    pub version: u32,
    pub ok: bool,
    pub command: &'static str,
    pub file: String,
    pub absolute_path: Option<String>,
    pub tokens: Vec<JsonToken>,
    pub diagnostics: Vec<Diagnostic>,
}

impl TokensEnvelope {
    pub fn new(
        file: impl Into<String>,
        absolute_path: Option<String>,
        tokens: Vec<JsonToken>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        let ok = diagnostics.is_empty();

        Self {
            schema: "nocter.tokens",
            version: 1,
            ok,
            command: "tokens",
            file: file.into(),
            absolute_path,
            tokens,
            diagnostics,
        }
    }
}

impl LexOutput {
    pub fn to_json_envelope(
        &self,
        sources: &SourceMap,
        source: SourceId,
    ) -> Result<TokensEnvelope, String> {
        let file = sources
            .get(source)
            .ok_or_else(|| format!("unknown source id {}", source.raw()))?;
        let mut json_tokens = Vec::with_capacity(self.tokens.len());

        for token in &self.tokens {
            let span = sources.span_to_json(token.span)?;
            let lexeme = file
                .text()
                .get(token.span.start..token.span.end)
                .unwrap_or("")
                .to_string();
            json_tokens.push(JsonToken {
                kind: token.kind.json_kind().to_string(),
                lexeme,
                span,
            });
        }

        Ok(TokensEnvelope::new(
            file.display_path().to_string(),
            file.absolute_path()
                .map(|path| path.to_string_lossy().into_owned()),
            json_tokens,
            self.diagnostics.clone(),
        ))
    }
}

struct Lexer<'a> {
    sources: &'a SourceMap,
    source: SourceId,
    text: &'a str,
    bytes: &'a [u8],
    index: usize,
    end: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn run(&mut self) {
        while !self.is_at_end() {
            let byte = self.bytes[self.index];
            match byte {
                b' ' | b'\t' => {
                    self.index += 1;
                }
                b'\n' => {
                    self.push(TokenKind::Newline, self.index, self.index + 1);
                    self.index += 1;
                }
                b'\r' => {
                    let start = self.index;
                    self.index += 1;
                    self.error(
                        start,
                        self.index,
                        "bare carriage return is invalid in source",
                    );
                }
                b'/' if self.peek(1) == Some(b'/') => self.skip_line_comment(),
                b'/' if self.peek(1) == Some(b'*') => self.skip_block_comment(),
                b'b' if self.peek(1) == Some(b'\'') => self.scan_byte_literal(),
                b'A'..=b'Z' | b'a'..=b'z' | b'_' => self.scan_identifier_or_keyword(),
                b'0'..=b'9' => self.scan_integer_literal(),
                b'"' => self.scan_string_literal(),
                b'\'' => {
                    let start = self.index;
                    self.index += 1;
                    self.error(
                        start,
                        self.index,
                        "plain single-quoted character literals are not part of v0",
                    );
                }
                b'@' => {
                    let start = self.index;
                    self.index += 1;
                    self.error(
                        start,
                        self.index,
                        "`@` is reserved and invalid in v0 source",
                    );
                }
                b'.' if self.peek(1).is_some_and(|next| next.is_ascii_digit()) => {
                    self.scan_invalid_leading_dot_float()
                }
                byte if byte.is_ascii() => {
                    if !self.scan_punctuation() {
                        let start = self.index;
                        self.index += 1;
                        self.error(start, self.index, "unexpected character in source");
                    }
                }
                _ => self.scan_invalid_non_ascii(),
            }
        }

        self.push(TokenKind::Eof, self.index, self.index);
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.end
    }

    fn peek(&self, offset: usize) -> Option<u8> {
        let index = self.index + offset;
        (index < self.end).then(|| self.bytes[index])
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: ByteSpan::new(self.source, start, end),
        });
    }

    fn error(&mut self, start: usize, end: usize, message: impl Into<String>) {
        let span = ByteSpan::new(self.source, start, end);
        let primary_span = self.sources.span_to_json(span).ok();
        let mut diagnostic = Diagnostic::error("E0100", message);
        diagnostic.primary_span = primary_span.map(Box::new);
        self.diagnostics.push(diagnostic);
    }

    fn skip_line_comment(&mut self) {
        self.index += 2;
        while !self.is_at_end() && self.bytes[self.index] != b'\n' {
            self.index += 1;
        }
    }

    fn skip_block_comment(&mut self) {
        let start = self.index;
        self.index += 2;

        while !self.is_at_end() {
            if self.bytes[self.index] == b'*' && self.peek(1) == Some(b'/') {
                self.index += 2;
                return;
            }

            if self.bytes[self.index] == b'\n' {
                self.push(TokenKind::Newline, self.index, self.index + 1);
            }

            self.index += 1;
        }

        self.error(start, self.index, "unterminated block comment");
    }

    fn scan_identifier_or_keyword(&mut self) {
        let start = self.index;
        self.index += 1;

        while !self.is_at_end() && is_identifier_continue(self.bytes[self.index]) {
            self.index += 1;
        }

        let text = &self.text[start..self.index];
        match keyword(text) {
            Some(keyword) => self.push(TokenKind::Keyword(keyword), start, self.index),
            None => self.push(TokenKind::Identifier, start, self.index),
        }
    }

    fn scan_integer_literal(&mut self) {
        let start = self.index;

        if self.bytes[self.index] == b'0' {
            match self.peek(1) {
                Some(b'x') => {
                    self.index += 2;
                    self.scan_prefixed_integer_tail(start, NumberBase::Hex);
                    return;
                }
                Some(b'b') => {
                    self.index += 2;
                    self.scan_prefixed_integer_tail(start, NumberBase::Binary);
                    return;
                }
                _ => {
                    self.index += 1;
                }
            }
        } else {
            self.index += 1;
        }

        while !self.is_at_end()
            && (self.bytes[self.index].is_ascii_digit() || self.bytes[self.index] == b'_')
        {
            self.index += 1;
        }

        if self.index < self.end
            && self.bytes[self.index] == b'.'
            && self.peek(1).is_some_and(|next| next.is_ascii_digit())
        {
            self.index += 1;
            while !self.is_at_end() && is_number_body_byte(self.bytes[self.index]) {
                self.index += 1;
            }
            self.error(start, self.index, "float literals are not part of v0");
            return;
        }

        if matches!(self.peek(0), Some(b'e' | b'E')) {
            let next = self.peek(1);
            if next.is_some_and(|byte| byte.is_ascii_digit() || byte == b'+' || byte == b'-') {
                self.index += 1;
                while !self.is_at_end() && is_number_body_byte(self.bytes[self.index]) {
                    self.index += 1;
                }
                self.error(start, self.index, "float literals are not part of v0");
                return;
            }
        }

        if self
            .peek(0)
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        {
            while !self.is_at_end() && is_number_body_byte(self.bytes[self.index]) {
                self.index += 1;
            }
            self.error(
                start,
                self.index,
                "integer type suffixes are not part of v0",
            );
            return;
        }

        if let Err(message) =
            validate_integer_literal(&self.text[start..self.index], NumberBase::Decimal)
        {
            self.error(start, self.index, message);
        }

        self.push(TokenKind::IntegerLiteral, start, self.index);
    }

    fn scan_prefixed_integer_tail(&mut self, start: usize, base: NumberBase) {
        while !self.is_at_end() && is_number_body_byte(self.bytes[self.index]) {
            self.index += 1;
        }

        if let Err(message) = validate_integer_literal(&self.text[start..self.index], base) {
            self.error(start, self.index, message);
        }

        self.push(TokenKind::IntegerLiteral, start, self.index);
    }

    fn scan_invalid_leading_dot_float(&mut self) {
        let start = self.index;
        self.index += 1;
        while !self.is_at_end() && is_number_body_byte(self.bytes[self.index]) {
            self.index += 1;
        }
        self.error(start, self.index, "float literals are not part of v0");
    }

    fn scan_string_literal(&mut self) {
        if self.bytes[self.index..self.end].starts_with(b"\"\"\"") {
            self.scan_multi_line_string_literal();
        } else {
            self.scan_single_line_string_literal();
        }
    }

    fn scan_single_line_string_literal(&mut self) {
        let start = self.index;
        self.index += 1;

        while !self.is_at_end() {
            match self.bytes[self.index] {
                b'"' => {
                    self.index += 1;
                    if self.validate_string_literal(start, self.index) {
                        self.push(TokenKind::StringLiteral, start, self.index);
                    }
                    return;
                }
                b'\n' | b'\r' => {
                    self.error(
                        start,
                        self.index + 1,
                        "raw newlines are invalid in single-line string literals",
                    );
                    return;
                }
                b'\\' => {
                    if let Err(message) = self.scan_escape() {
                        self.error(start, self.index, message);
                        return;
                    }
                }
                b'$' if self.peek(1) == Some(b'{') => {
                    let interpolation_start = self.index;
                    match find_interpolation_end(self.text, interpolation_start, self.end) {
                        Ok(end) => self.index = end + 1,
                        Err(error) => {
                            self.error(error.start, error.end, error.message);
                            self.index = error.end;
                            return;
                        }
                    }
                }
                _ => {
                    self.index += current_char_len(self.text, self.index);
                }
            }
        }

        self.error(start, self.index, "unterminated string literal");
    }

    fn scan_multi_line_string_literal(&mut self) {
        let start = self.index;
        self.index += 3;

        if self.peek(0) != Some(b'\n') {
            self.error(
                start,
                self.index,
                "multi-line string opening delimiter must be followed by a newline",
            );
            return;
        }

        self.index += 1;
        let mut line_start = self.index;

        while !self.is_at_end() {
            if self.index == line_start {
                let mut indent_end = self.index;
                while indent_end < self.end
                    && matches!(self.bytes.get(indent_end), Some(b' ' | b'\t'))
                {
                    indent_end += 1;
                }

                if indent_end <= self.end && self.bytes[indent_end..self.end].starts_with(b"\"\"\"")
                {
                    self.index = indent_end + 3;
                    if self.validate_string_literal(start, self.index) {
                        self.push(TokenKind::StringLiteral, start, self.index);
                    }
                    return;
                }
            }

            match self.bytes[self.index] {
                b'\n' => {
                    self.index += 1;
                    line_start = self.index;
                }
                b'\r' => {
                    self.error(
                        start,
                        self.index + 1,
                        "bare carriage return is invalid in string literals",
                    );
                    return;
                }
                b'\\' => {
                    if let Err(message) = self.scan_escape() {
                        self.error(start, self.index, message);
                        return;
                    }
                }
                b'$' if self.peek(1) == Some(b'{') => {
                    let interpolation_start = self.index;
                    match find_interpolation_end(self.text, interpolation_start, self.end) {
                        Ok(end) => self.index = end + 1,
                        Err(error) => {
                            self.error(error.start, error.end, error.message);
                            self.index = error.end;
                            return;
                        }
                    }
                }
                _ => {
                    self.index += current_char_len(self.text, self.index);
                }
            }
        }

        self.error(start, self.index, "unterminated multi-line string literal");
    }

    fn validate_string_literal(&mut self, start: usize, end: usize) -> bool {
        match validate_string_literal_source(&self.text[start..end]) {
            Ok(_) => true,
            Err(error) => {
                self.error(start + error.start, start + error.end, error.message);
                false
            }
        }
    }

    fn scan_byte_literal(&mut self) {
        let start = self.index;
        self.index += 2;
        let mut decoded_bytes = 0usize;

        while !self.is_at_end() {
            match self.bytes[self.index] {
                b'\'' => {
                    self.index += 1;
                    if decoded_bytes == 1 {
                        self.push(TokenKind::ByteLiteral, start, self.index);
                    } else {
                        self.error(
                            start,
                            self.index,
                            "byte literal must decode to exactly one byte",
                        );
                    }
                    return;
                }
                b'\n' | b'\r' => {
                    self.error(
                        start,
                        self.index + 1,
                        "raw newlines are invalid in byte literals",
                    );
                    return;
                }
                b'\\' => {
                    if let Err(message) = self.scan_escape() {
                        self.error(start, self.index, message);
                        return;
                    }
                    decoded_bytes += 1;
                }
                byte if byte.is_ascii() => {
                    self.index += 1;
                    decoded_bytes += 1;
                }
                _ => {
                    let char_len = current_char_len(self.text, self.index);
                    self.index += char_len;
                    decoded_bytes += char_len;
                }
            }
        }

        self.error(start, self.index, "unterminated byte literal");
    }

    fn scan_escape(&mut self) -> Result<(), &'static str> {
        self.index += 1;
        if self.is_at_end() {
            return Err("unterminated escape sequence");
        }

        match self.bytes[self.index] {
            b'n' | b'r' | b't' | b'0' | b'\\' | b'"' | b'\'' | b'$' => {
                self.index += 1;
                Ok(())
            }
            b'x' => {
                if self.peek(1).is_some_and(is_hex_digit) && self.peek(2).is_some_and(is_hex_digit)
                {
                    self.index += 3;
                    Ok(())
                } else {
                    Err("`\\x` escape must be followed by two hexadecimal digits")
                }
            }
            _ => Err("invalid escape sequence"),
        }
    }

    fn scan_punctuation(&mut self) -> bool {
        let start = self.index;
        const MULTI: &[(&[u8], &str)] = &[
            (b"..<", "..<"),
            (b"&+", "&+"),
            (b"??", "??"),
            (b"==", "=="),
            (b"!=", "!="),
            (b"<=", "<="),
            (b">=", ">="),
            (b"&&", "&&"),
            (b"||", "||"),
            (b"<<", "<<"),
            (b">>", ">>"),
            (b"+=", "+="),
            (b"-=", "-="),
            (b"*=", "*="),
            (b"/=", "/="),
            (b"%=", "%="),
        ];

        for &(bytes, spelling) in MULTI {
            if self.bytes[start..self.end].starts_with(bytes) {
                self.index += bytes.len();
                self.push(TokenKind::Punctuation(spelling), start, self.index);
                return true;
            }
        }

        let spelling = match self.bytes[start] {
            b'(' => "(",
            b')' => ")",
            b'{' => "{",
            b'}' => "}",
            b'[' => "[",
            b']' => "]",
            b',' => ",",
            b':' => ":",
            b';' => ";",
            b'.' => ".",
            b'+' => "+",
            b'-' => "-",
            b'*' => "*",
            b'/' => "/",
            b'%' => "%",
            b'=' => "=",
            b'!' => "!",
            b'<' => "<",
            b'>' => ">",
            b'&' => "&",
            b'|' => "|",
            b'?' => "?",
            _ => return false,
        };

        self.index += 1;
        self.push(TokenKind::Punctuation(spelling), start, self.index);
        true
    }

    fn scan_invalid_non_ascii(&mut self) {
        let start = self.index;
        let char_len = current_char_len(self.text, self.index);
        self.index += char_len;
        self.error(
            start,
            self.index,
            "non-ASCII characters are invalid outside string literals, byte literals, and comments",
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberBase {
    Decimal,
    Hex,
    Binary,
}

fn current_char_len(text: &str, offset: usize) -> usize {
    text[offset..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

fn is_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_number_body_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_hex_digit(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

fn validate_integer_literal(text: &str, base: NumberBase) -> Result<(), &'static str> {
    let digits = match base {
        NumberBase::Decimal => text,
        NumberBase::Hex | NumberBase::Binary => {
            if text.len() == 2 {
                return Err("integer literal prefix must be followed by digits");
            }
            &text[2..]
        }
    };

    let mut previous_underscore = false;
    let mut previous_digit = false;

    for byte in digits.bytes() {
        if byte == b'_' {
            if !previous_digit || previous_underscore {
                return Err("invalid digit separator placement in integer literal");
            }
            previous_underscore = true;
            previous_digit = false;
            continue;
        }

        if !digit_matches_base(byte, base) {
            return Err("integer literal contains a digit that is invalid for its base");
        }

        previous_underscore = false;
        previous_digit = true;
    }

    if previous_underscore || !previous_digit {
        return Err("invalid digit separator placement in integer literal");
    }

    Ok(())
}

fn digit_matches_base(byte: u8, base: NumberBase) -> bool {
    match base {
        NumberBase::Decimal => byte.is_ascii_digit(),
        NumberBase::Hex => byte.is_ascii_hexdigit(),
        NumberBase::Binary => matches!(byte, b'0' | b'1'),
    }
}

fn keyword(text: &str) -> Option<Keyword> {
    Some(match text {
        "from" => Keyword::From,
        "import" => Keyword::Import,
        "use" => Keyword::Use,
        "func" => Keyword::Func,
        "pub" => Keyword::Pub,
        "type" => Keyword::Type,
        "copy" => Keyword::Copy,
        "struct" => Keyword::Struct,
        "enum" => Keyword::Enum,
        "trait" => Keyword::Trait,
        "impl" => Keyword::Impl,
        "method" => Keyword::Method,
        "let" => Keyword::Let,
        "var" => Keyword::Var,
        "return" => Keyword::Return,
        "if" => Keyword::If,
        "else" => Keyword::Else,
        "for" => Keyword::For,
        "in" => Keyword::In,
        "while" => Keyword::While,
        "loop" => Keyword::Loop,
        "break" => Keyword::Break,
        "continue" => Keyword::Continue,
        "match" => Keyword::Match,
        "is" => Keyword::Is,
        "catch" => Keyword::Catch,
        "none" => Keyword::None,
        "true" => Keyword::True,
        "false" => Keyword::False,
        "move" => Keyword::Move,
        "as" => Keyword::As,
        "region" => Keyword::Region,
        "using" => Keyword::Using,
        "primitive" => Keyword::Primitive,
        "void" => Keyword::Void,
        "never" => Keyword::Never,
        _ => return None,
    })
}

pub(crate) fn is_valid_identifier_name(text: &str) -> bool {
    let Some(first) = text.as_bytes().first().copied() else {
        return false;
    };

    matches!(first, b'A'..=b'Z' | b'a'..=b'z' | b'_')
        && text.bytes().all(is_identifier_continue)
        && keyword(text).is_none()
}

#[cfg(test)]
mod tests {
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
    }

    #[test]
    fn validates_identifier_names() {
        assert!(is_valid_identifier_name("main"));
        assert!(is_valid_identifier_name("_entry2"));
        assert!(is_valid_identifier_name("program"));
        assert!(is_valid_identifier_name("drop"));
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
}

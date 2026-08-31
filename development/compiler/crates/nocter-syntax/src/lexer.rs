use nocter_source::{ByteOffset, SourceFile, SourceId, Span, TextRange};

use crate::literal::{decode_escape, valid_integer};
use crate::{Keyword, Punctuation, StringDelimiter, Token, TokenKind};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommentKind {
    Line,
    Block,
    ItemDocumentation,
    FileDocumentation,
}

impl CommentKind {
    /// Stable category spelling used by compiler-owned tooling protocols.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Block => "block",
            Self::ItemDocumentation => "item_documentation",
            Self::FileDocumentation => "file_documentation",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Comment {
    kind: CommentKind,
    span: Span,
}

impl Comment {
    #[must_use]
    pub const fn kind(self) -> CommentKind {
        self.kind
    }

    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }

    const fn with_source(self, source: SourceId) -> Self {
        Self {
            kind: self.kind,
            span: Span::new(source, self.span.range()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LexDiagnosticKind {
    UnexpectedCharacter,
    UnterminatedBlockComment,
    InvalidIntegerLiteral,
    UnsupportedFloatLiteral,
    UnterminatedString,
    SingleLineStringNewline,
    MultilineStringOpeningNewline,
    InvalidEscape,
    InvalidStringUtf8,
    MultilineStringIndentation,
    UnterminatedByteLiteral,
    ByteLiteralNewline,
    InvalidByteLength,
    PlainSingleQuote,
    UnterminatedInterpolation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LexDiagnostic {
    kind: LexDiagnosticKind,
    span: Span,
}

impl LexDiagnostic {
    #[must_use]
    pub const fn kind(self) -> LexDiagnosticKind {
        self.kind
    }

    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }

    const fn with_source(self, source: SourceId) -> Self {
        Self {
            kind: self.kind,
            span: Span::new(source, self.span.range()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LexedFile {
    source: SourceId,
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    diagnostics: Vec<LexDiagnostic>,
}

impl LexedFile {
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    #[must_use]
    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[LexDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn rebind_source(&mut self, source: SourceId) {
        self.source = source;
        for token in &mut self.tokens {
            *token = token.with_source(source);
        }
        for comment in &mut self.comments {
            *comment = comment.with_source(source);
        }
        for diagnostic in &mut self.diagnostics {
            *diagnostic = diagnostic.with_source(source);
        }
    }
}

#[must_use]
pub fn lex(source: &SourceFile) -> LexedFile {
    Lexer::new(source).run()
}

struct Lexer<'source> {
    source: &'source SourceFile,
    text: &'source str,
    bytes: &'source [u8],
    cursor: usize,
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    diagnostics: Vec<LexDiagnostic>,
}

impl<'source> Lexer<'source> {
    fn new(source: &'source SourceFile) -> Self {
        Self {
            source,
            text: source.text(),
            bytes: source.text().as_bytes(),
            cursor: 0,
            tokens: Vec::new(),
            comments: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self) -> LexedFile {
        self.lex_code(false);
        let end = self.cursor.min(self.bytes.len());
        self.push_token(TokenKind::Eof, end, end);
        self.attach_joint_facts();

        LexedFile {
            source: self.source.id(),
            tokens: self.tokens,
            comments: self.comments,
            diagnostics: self.diagnostics,
        }
    }

    fn lex_code(&mut self, interpolation: bool) -> bool {
        let mut brace_depth = 0_u32;

        while self.cursor < self.bytes.len() {
            let start = self.cursor;
            let byte = self.bytes[start];

            if interpolation && byte == b'}' {
                self.cursor += 1;
                if brace_depth == 0 {
                    self.push_token(TokenKind::InterpolationEnd, start, self.cursor);
                    return true;
                }
                brace_depth -= 1;
                self.push_token(
                    TokenKind::Punctuation(Punctuation::RightBrace),
                    start,
                    self.cursor,
                );
                continue;
            }

            match byte {
                b' ' | b'\t' => self.cursor += 1,
                b'\n' => {
                    self.cursor += 1;
                    self.push_token(TokenKind::Newline, start, self.cursor);
                }
                b'/' if self.starts_with("//") => self.lex_line_comment(),
                b'/' if self.starts_with("/*") => self.lex_block_comment(),
                b'b' if self.starts_with("b'") => self.lex_byte_literal(),
                b'"' => self.lex_string(),
                b'\'' => {
                    self.cursor += 1;
                    self.diagnostic(LexDiagnosticKind::PlainSingleQuote, start, self.cursor);
                }
                b'0'..=b'9' => self.lex_number(),
                b'.' if self.bytes.get(start + 1).is_some_and(u8::is_ascii_digit) => {
                    self.lex_leading_dot_float();
                }
                byte if is_identifier_start(byte) => self.lex_identifier(),
                b'{' => {
                    self.cursor += 1;
                    if interpolation {
                        brace_depth += 1;
                    }
                    self.push_token(
                        TokenKind::Punctuation(Punctuation::LeftBrace),
                        start,
                        self.cursor,
                    );
                }
                _ => self.lex_punctuation_or_error(),
            }
        }

        if interpolation {
            self.diagnostic(
                LexDiagnosticKind::UnterminatedInterpolation,
                self.cursor,
                self.cursor,
            );
            false
        } else {
            true
        }
    }

    fn lex_identifier(&mut self) {
        let start = self.cursor;
        self.cursor += 1;
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| is_identifier_continue(*byte))
        {
            self.cursor += 1;
        }

        let text = &self.text[start..self.cursor];
        let kind = Keyword::from_spelling(text).map_or(TokenKind::Identifier, TokenKind::Keyword);
        self.push_token(kind, start, self.cursor);
    }

    fn lex_number(&mut self) {
        let start = self.cursor;
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(u8::is_ascii_alphanumeric)
            || self.bytes.get(self.cursor) == Some(&b'_')
        {
            self.cursor += 1;
        }

        if self.bytes.get(self.cursor) == Some(&b'.')
            && self
                .bytes
                .get(self.cursor + 1)
                .is_some_and(u8::is_ascii_digit)
        {
            self.cursor += 1;
            while self
                .bytes
                .get(self.cursor)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'.')
            {
                self.cursor += 1;
            }
            self.push_token(TokenKind::IntegerLiteral, start, self.cursor);
            self.diagnostic(
                LexDiagnosticKind::UnsupportedFloatLiteral,
                start,
                self.cursor,
            );
            return;
        }

        self.push_token(TokenKind::IntegerLiteral, start, self.cursor);
        let candidate = &self.text[start..self.cursor];
        if !valid_integer(candidate) {
            let kind = if !candidate.starts_with("0x")
                && !candidate.starts_with("0b")
                && candidate.contains(['e', 'E'])
            {
                LexDiagnosticKind::UnsupportedFloatLiteral
            } else {
                LexDiagnosticKind::InvalidIntegerLiteral
            };
            self.diagnostic(kind, start, self.cursor);
        }
    }

    fn lex_leading_dot_float(&mut self) {
        let start = self.cursor;
        self.cursor += 1;
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            self.cursor += 1;
        }
        self.push_token(TokenKind::IntegerLiteral, start, self.cursor);
        self.diagnostic(
            LexDiagnosticKind::UnsupportedFloatLiteral,
            start,
            self.cursor,
        );
    }

    fn lex_line_comment(&mut self) {
        let start = self.cursor;
        let kind = if self.starts_with("//!") {
            CommentKind::FileDocumentation
        } else if self.starts_with("///") && !self.starts_with("////") {
            CommentKind::ItemDocumentation
        } else {
            CommentKind::Line
        };

        self.cursor += 2;
        while self.cursor < self.bytes.len() && self.bytes[self.cursor] != b'\n' {
            self.cursor += self.next_char_len();
        }
        self.comments.push(Comment {
            kind,
            span: self.span(start, self.cursor),
        });
    }

    fn lex_block_comment(&mut self) {
        let start = self.cursor;
        let kind = if self.starts_with("/*!") {
            CommentKind::FileDocumentation
        } else if self.starts_with("/**") && !self.starts_with("/**/") && !self.starts_with("/***")
        {
            CommentKind::ItemDocumentation
        } else {
            CommentKind::Block
        };

        self.cursor += 2;
        while self.cursor < self.bytes.len() {
            if self.starts_with("*/") {
                self.cursor += 2;
                self.comments.push(Comment {
                    kind,
                    span: self.span(start, self.cursor),
                });
                return;
            }
            if self.bytes[self.cursor] == b'\n' {
                let newline = self.cursor;
                self.cursor += 1;
                self.push_token(TokenKind::Newline, newline, self.cursor);
            } else {
                self.cursor += self.next_char_len();
            }
        }

        self.comments.push(Comment {
            kind,
            span: self.span(start, self.cursor),
        });
        self.diagnostic(
            LexDiagnosticKind::UnterminatedBlockComment,
            start,
            self.cursor,
        );
    }

    fn lex_string(&mut self) {
        let delimiter = if self.starts_with("\"\"\"") {
            StringDelimiter::MultiLine
        } else {
            StringDelimiter::SingleLine
        };
        let start = self.cursor;
        let delimiter_len = match delimiter {
            StringDelimiter::SingleLine => 1,
            StringDelimiter::MultiLine => 3,
        };
        self.cursor += delimiter_len;
        self.push_token(TokenKind::StringStart(delimiter), start, self.cursor);

        if delimiter == StringDelimiter::MultiLine && self.bytes.get(self.cursor) != Some(&b'\n') {
            self.diagnostic(
                LexDiagnosticKind::MultilineStringOpeningNewline,
                start,
                self.cursor,
            );
        }

        let mut text_start = self.cursor;
        while self.cursor < self.bytes.len() {
            if self.is_string_end(delimiter) {
                if delimiter == StringDelimiter::MultiLine {
                    self.validate_multiline_indentation(start, self.cursor);
                }
                self.emit_string_text(text_start, self.cursor);
                let end_start = self.cursor;
                self.cursor += delimiter_len;
                self.push_token(TokenKind::StringEnd(delimiter), end_start, self.cursor);
                return;
            }

            if self.starts_with("${") {
                self.emit_string_text(text_start, self.cursor);
                let interpolation_start = self.cursor;
                self.cursor += 2;
                self.push_token(
                    TokenKind::InterpolationStart,
                    interpolation_start,
                    self.cursor,
                );
                if !self.lex_code(true) {
                    return;
                }
                text_start = self.cursor;
                continue;
            }

            if self.bytes[self.cursor] == b'\\' {
                self.cursor += self.escape_source_len();
                continue;
            }

            if self.bytes[self.cursor] == b'\n' && delimiter == StringDelimiter::SingleLine {
                self.emit_string_text(text_start, self.cursor);
                self.diagnostic(
                    LexDiagnosticKind::SingleLineStringNewline,
                    self.cursor,
                    self.cursor + 1,
                );
                return;
            }

            self.cursor += self.next_char_len();
        }

        self.emit_string_text(text_start, self.cursor);
        self.diagnostic(LexDiagnosticKind::UnterminatedString, start, self.cursor);
    }

    fn is_string_end(&self, delimiter: StringDelimiter) -> bool {
        match delimiter {
            StringDelimiter::SingleLine => self.bytes.get(self.cursor) == Some(&b'"'),
            StringDelimiter::MultiLine => {
                if !self.starts_with("\"\"\"") {
                    return false;
                }
                let line_start = self.text[..self.cursor]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
                self.bytes[line_start..self.cursor]
                    .iter()
                    .all(|byte| matches!(byte, b' ' | b'\t'))
            }
        }
    }

    fn emit_string_text(&mut self, start: usize, end: usize) {
        if start == end {
            return;
        }
        self.push_token(TokenKind::StringText, start, end);
        self.validate_string_text(start, end);
    }

    fn validate_string_text(&mut self, start: usize, end: usize) {
        let mut decoded = Vec::with_capacity(end - start);
        let mut cursor = start;
        let mut valid = true;
        while cursor < end {
            if self.bytes[cursor] != b'\\' {
                let length = char_len_at(self.text, cursor);
                decoded.extend_from_slice(&self.bytes[cursor..cursor + length]);
                cursor += length;
                continue;
            }

            match decode_escape(self.text, cursor, end) {
                Ok((byte, length)) => {
                    decoded.push(byte);
                    cursor += length;
                }
                Err(length) => {
                    valid = false;
                    let diagnostic_end = (cursor + length).min(end);
                    self.diagnostic(LexDiagnosticKind::InvalidEscape, cursor, diagnostic_end);
                    cursor = diagnostic_end.max(cursor + 1);
                }
            }
        }

        if valid && std::str::from_utf8(&decoded).is_err() {
            self.diagnostic(LexDiagnosticKind::InvalidStringUtf8, start, end);
        }
    }

    fn validate_multiline_indentation(&mut self, literal_start: usize, delimiter_start: usize) {
        let content_start = literal_start + 3;
        if self.bytes.get(content_start) != Some(&b'\n') {
            return;
        }

        let closing_line_start = self.text[..delimiter_start]
            .rfind('\n')
            .map_or(content_start, |index| index + 1);
        let indentation = &self.bytes[closing_line_start..delimiter_start];
        let mut line_start = content_start + 1;

        while line_start < closing_line_start {
            let line_end = self.bytes[line_start..closing_line_start]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(closing_line_start, |offset| line_start + offset);
            if line_end > line_start && !self.bytes[line_start..line_end].starts_with(indentation) {
                self.diagnostic(
                    LexDiagnosticKind::MultilineStringIndentation,
                    line_start,
                    line_end,
                );
            }
            line_start = line_end.saturating_add(1);
        }
    }

    fn escape_source_len(&self) -> usize {
        match decode_escape(self.text, self.cursor, self.bytes.len()) {
            Ok((_, length)) | Err(length) => length,
        }
    }

    fn lex_byte_literal(&mut self) {
        let start = self.cursor;
        self.cursor += 2;
        let content_start = self.cursor;

        while self.cursor < self.bytes.len() {
            match self.bytes[self.cursor] {
                b'\'' => {
                    let content_end = self.cursor;
                    self.cursor += 1;
                    self.push_token(TokenKind::ByteLiteral, start, self.cursor);
                    self.validate_byte_content(content_start, content_end);
                    return;
                }
                b'\n' => {
                    self.diagnostic(
                        LexDiagnosticKind::ByteLiteralNewline,
                        self.cursor,
                        self.cursor + 1,
                    );
                    return;
                }
                b'\\' => self.cursor += self.escape_source_len(),
                _ => self.cursor += self.next_char_len(),
            }
        }

        self.diagnostic(
            LexDiagnosticKind::UnterminatedByteLiteral,
            start,
            self.cursor,
        );
    }

    fn validate_byte_content(&mut self, start: usize, end: usize) {
        let mut count = 0_usize;
        let mut cursor = start;
        let mut valid = true;
        while cursor < end {
            if self.bytes[cursor] == b'\\' {
                match decode_escape(self.text, cursor, end) {
                    Ok((_, length)) => {
                        count += 1;
                        cursor += length;
                    }
                    Err(length) => {
                        valid = false;
                        let diagnostic_end = (cursor + length).min(end);
                        self.diagnostic(LexDiagnosticKind::InvalidEscape, cursor, diagnostic_end);
                        cursor = diagnostic_end.max(cursor + 1);
                    }
                }
            } else {
                let length = self.next_char_len_at(cursor);
                count += length;
                cursor += length;
            }
        }

        if valid && count != 1 {
            self.diagnostic(LexDiagnosticKind::InvalidByteLength, start, end);
        }
    }

    fn lex_punctuation_or_error(&mut self) {
        let start = self.cursor;
        if let Some(punctuation) = Punctuation::longest_prefix(&self.text[start..]) {
            self.cursor += punctuation.as_str().len();
            self.push_token(TokenKind::Punctuation(punctuation), start, self.cursor);
        } else {
            self.cursor += self.next_char_len();
            self.diagnostic(LexDiagnosticKind::UnexpectedCharacter, start, self.cursor);
        }
    }

    fn attach_joint_facts(&mut self) {
        for index in 0..self.tokens.len().saturating_sub(1) {
            let left_end = self.tokens[index].span().range().end();
            let right_start = self.tokens[index + 1].span().range().start();
            self.tokens[index].set_joint_to_next(left_end == right_start);
        }
    }

    fn starts_with(&self, text: &str) -> bool {
        self.text[self.cursor..].starts_with(text)
    }

    fn next_char_len(&self) -> usize {
        self.next_char_len_at(self.cursor)
    }

    fn next_char_len_at(&self, offset: usize) -> usize {
        char_len_at(self.text, offset)
    }

    fn push_token(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token::new(kind, self.span(start, end)));
    }

    fn diagnostic(&mut self, kind: LexDiagnosticKind, start: usize, end: usize) {
        self.diagnostics.push(LexDiagnostic {
            kind,
            span: self.span(start, end),
        });
    }

    fn span(&self, start: usize, end: usize) -> Span {
        self.source.span(TextRange::new(offset(start), offset(end)))
    }
}

fn char_len_at(text: &str, offset: usize) -> usize {
    text[offset..].chars().next().map_or(1, char::len_utf8)
}

fn offset(value: usize) -> ByteOffset {
    ByteOffset::new(u32::try_from(value).expect("source length was validated"))
}

const fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod tests;

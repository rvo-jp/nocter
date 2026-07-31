use super::identifiers::{is_identifier_continue, keyword};
use super::numbers::{NumberBase, is_hex_digit, is_number_body_byte, validate_integer_literal};
use super::*;

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
            b'#' => "#",
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

fn current_char_len(text: &str, offset: usize) -> usize {
    text[offset..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

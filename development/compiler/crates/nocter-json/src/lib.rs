//! Bounded JSON syntax and exact rendering shared by compiler protocol boundaries.

use std::fmt;
use std::fmt::Write;

const MAX_NESTING: usize = 128;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Box<str>),
    String(Box<str>),
    Array(Vec<Self>),
    Object(Vec<Member>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub name: Box<str>,
    pub value: Value,
}

/// Parses one complete JSON value while retaining object member order and duplicates.
///
/// # Errors
///
/// Returns a byte-positioned syntax error or a bounded-nesting failure.
pub fn parse(input: &str) -> Result<Value, JsonError> {
    let mut parser = Parser {
        input,
        bytes: input.as_bytes(),
        cursor: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.cursor != parser.bytes.len() {
        return Err(parser.error(JsonErrorKind::TrailingData));
    }
    Ok(value)
}

/// Appends one exactly escaped JSON string.
pub fn write_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("writing to String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

struct Parser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    cursor: usize,
}

impl Parser<'_> {
    fn parse_value(&mut self, depth: usize) -> Result<Value, JsonError> {
        match self.peek() {
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(Value::Null)
            }
            Some(b't') => {
                self.literal(b"true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(Value::Bool(false))
            }
            Some(b'"') => self.parse_string().map(Value::String),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_object(depth),
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(Value::Number),
            Some(_) => Err(self.error(JsonErrorKind::ExpectedValue)),
            None => Err(self.error(JsonErrorKind::UnexpectedEnd)),
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.enter_container(depth)?;
        self.cursor += 1;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume(b']') {
            return Ok(Value::Array(values));
        }
        loop {
            values.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            if self.consume(b']') {
                return Ok(Value::Array(values));
            }
            self.expect(b',', JsonErrorKind::ExpectedArraySeparator)?;
            self.skip_whitespace();
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, JsonError> {
        self.enter_container(depth)?;
        self.cursor += 1;
        self.skip_whitespace();
        let mut members = Vec::new();
        if self.consume(b'}') {
            return Ok(Value::Object(members));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(self.error(JsonErrorKind::ExpectedObjectName));
            }
            let name = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':', JsonErrorKind::ExpectedNameSeparator)?;
            self.skip_whitespace();
            let value = self.parse_value(depth + 1)?;
            members.push(Member { name, value });
            self.skip_whitespace();
            if self.consume(b'}') {
                return Ok(Value::Object(members));
            }
            self.expect(b',', JsonErrorKind::ExpectedObjectSeparator)?;
            self.skip_whitespace();
        }
    }

    fn enter_container(&self, depth: usize) -> Result<(), JsonError> {
        if depth >= MAX_NESTING {
            Err(self.error(JsonErrorKind::NestingLimit))
        } else {
            Ok(())
        }
    }

    fn parse_string(&mut self) -> Result<Box<str>, JsonError> {
        self.cursor += 1;
        let mut result = String::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.cursor += 1;
                    return Ok(result.into_boxed_str());
                }
                Some(b'\\') => {
                    self.cursor += 1;
                    self.parse_escape(&mut result)?;
                }
                Some(0x00..=0x1f) => {
                    return Err(self.error(JsonErrorKind::UnescapedControlCharacter));
                }
                Some(_) => {
                    let character = self.input[self.cursor..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.error(JsonErrorKind::UnexpectedEnd))?;
                    self.cursor += character.len_utf8();
                    result.push(character);
                }
                None => return Err(self.error(JsonErrorKind::UnexpectedEnd)),
            }
        }
    }

    fn parse_escape(&mut self, output: &mut String) -> Result<(), JsonError> {
        let character = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{0008}',
            Some(b'f') => '\u{000c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => {
                self.cursor += 1;
                let first = self.parse_hex_quad()?;
                if (0xd800..=0xdbff).contains(&first) {
                    if !self.consume(b'\\') || !self.consume(b'u') {
                        return Err(self.error(JsonErrorKind::InvalidUnicodeEscape));
                    }
                    let second = self.parse_hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(self.error(JsonErrorKind::InvalidUnicodeEscape));
                    }
                    let scalar = 0x1_0000
                        + ((u32::from(first) - 0xd800) << 10)
                        + (u32::from(second) - 0xdc00);
                    return char::from_u32(scalar)
                        .map(|character| output.push(character))
                        .ok_or_else(|| self.error(JsonErrorKind::InvalidUnicodeEscape));
                }
                if (0xdc00..=0xdfff).contains(&first) {
                    return Err(self.error(JsonErrorKind::InvalidUnicodeEscape));
                }
                return char::from_u32(u32::from(first))
                    .map(|character| output.push(character))
                    .ok_or_else(|| self.error(JsonErrorKind::InvalidUnicodeEscape));
            }
            Some(_) => return Err(self.error(JsonErrorKind::InvalidStringEscape)),
            None => return Err(self.error(JsonErrorKind::UnexpectedEnd)),
        };
        self.cursor += 1;
        output.push(character);
        Ok(())
    }

    fn parse_hex_quad(&mut self) -> Result<u16, JsonError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let digit = match self.peek() {
                Some(b'0'..=b'9') => u16::from(self.bytes[self.cursor] - b'0'),
                Some(b'a'..=b'f') => u16::from(self.bytes[self.cursor] - b'a' + 10),
                Some(b'A'..=b'F') => u16::from(self.bytes[self.cursor] - b'A' + 10),
                Some(_) => return Err(self.error(JsonErrorKind::InvalidUnicodeEscape)),
                None => return Err(self.error(JsonErrorKind::UnexpectedEnd)),
            };
            self.cursor += 1;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Box<str>, JsonError> {
        let start = self.cursor;
        self.consume(b'-');
        match self.peek() {
            Some(b'0') => {
                self.cursor += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error(JsonErrorKind::InvalidNumber));
                }
            }
            Some(b'1'..=b'9') => self.consume_digits(),
            _ => return Err(self.error(JsonErrorKind::InvalidNumber)),
        }
        if self.consume(b'.') {
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(JsonErrorKind::InvalidNumber));
            }
            self.consume_digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.cursor += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(JsonErrorKind::InvalidNumber));
            }
            self.consume_digits();
        }
        Ok(self.input[start..self.cursor].into())
    }

    fn consume_digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.cursor += 1;
        }
    }

    fn literal(&mut self, literal: &[u8]) -> Result<(), JsonError> {
        if self.bytes.get(self.cursor..self.cursor + literal.len()) == Some(literal) {
            self.cursor += literal.len();
            Ok(())
        } else {
            Err(self.error(JsonErrorKind::ExpectedValue))
        }
    }

    fn expect(&mut self, byte: u8, kind: JsonErrorKind) -> Result<(), JsonError> {
        if self.consume(byte) {
            Ok(())
        } else {
            Err(self.error(kind))
        }
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.cursor).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.cursor += 1;
        }
    }

    fn error(&self, kind: JsonErrorKind) -> JsonError {
        JsonError {
            offset: self.cursor,
            kind,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonErrorKind {
    UnexpectedEnd,
    ExpectedValue,
    ExpectedArraySeparator,
    ExpectedObjectName,
    ExpectedNameSeparator,
    ExpectedObjectSeparator,
    InvalidNumber,
    InvalidStringEscape,
    InvalidUnicodeEscape,
    UnescapedControlCharacter,
    NestingLimit,
    TrailingData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsonError {
    offset: usize,
    kind: JsonErrorKind,
}

impl JsonError {
    #[must_use]
    pub const fn offset(self) -> usize {
        self.offset
    }

    #[must_use]
    pub const fn kind(self) -> JsonErrorKind {
        self.kind
    }
}

impl fmt::Display for JsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            JsonErrorKind::UnexpectedEnd => "unexpected end of input",
            JsonErrorKind::ExpectedValue => "expected a JSON value",
            JsonErrorKind::ExpectedArraySeparator => "expected `,` or `]`",
            JsonErrorKind::ExpectedObjectName => "expected an object member name",
            JsonErrorKind::ExpectedNameSeparator => "expected `:` after an object member name",
            JsonErrorKind::ExpectedObjectSeparator => "expected `,` or `}`",
            JsonErrorKind::InvalidNumber => "invalid JSON number",
            JsonErrorKind::InvalidStringEscape => "invalid JSON string escape",
            JsonErrorKind::InvalidUnicodeEscape => "invalid JSON Unicode escape",
            JsonErrorKind::UnescapedControlCharacter => "unescaped control character in string",
            JsonErrorKind::NestingLimit => "JSON nesting limit exceeded",
            JsonErrorKind::TrailingData => "trailing data after the JSON value",
        };
        write!(formatter, "{message} at byte {}", self.offset)
    }
}

impl std::error::Error for JsonError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_json_without_collapsing_object_members() {
        let value = parse(
            r#"{"same": 1, "same": -2.5e+3, "text": "A\u00df\ud834\udd1e", "all": [true, false, null]}"#,
        )
        .unwrap();
        let Value::Object(members) = value else {
            panic!("expected object");
        };
        assert_eq!(members.len(), 4);
        assert_eq!(members[0].name.as_ref(), "same");
        assert_eq!(members[1].name.as_ref(), "same");
        assert_eq!(members[2].value, Value::String("Aß𝄞".into()));
    }

    #[test]
    fn rejects_non_json_numbers_strings_and_trailing_data() {
        for invalid in ["01", "1.", "[1,]", r#""\ud800""#, "true false"] {
            assert!(parse(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn rejects_unbounded_recursive_input() {
        let source = format!(
            "{}0{}",
            "[".repeat(MAX_NESTING + 1),
            "]".repeat(MAX_NESTING + 1)
        );
        assert_eq!(
            parse(&source).unwrap_err().kind(),
            JsonErrorKind::NestingLimit
        );
    }

    #[test]
    fn renders_every_json_string_escape_exactly() {
        let mut output = String::new();
        write_string(&mut output, "\"\\\u{08}\u{0c}\n\r\t\u{01}β");
        assert_eq!(output, r#""\"\\\b\f\n\r\t\u0001β""#);
    }
}

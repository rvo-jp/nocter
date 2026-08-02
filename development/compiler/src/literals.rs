//! Literal decoding shared by compiler phases.

use std::str;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StringLiteralPartSpan {
    Text {
        start: usize,
        end: usize,
    },
    Interpolation {
        start: usize,
        expression_start: usize,
        expression_end: usize,
        end: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StringLiteralError {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) message: &'static str,
}

pub(crate) fn decode_string_literal_bytes(text: &str) -> Result<Vec<u8>, &'static str> {
    if string_literal_parts(text)
        .map_err(|error| error.message)?
        .iter()
        .any(|part| matches!(part, StringLiteralPartSpan::Interpolation { .. }))
    {
        return Err("interpolated string is not a string literal");
    }

    let bytes = text.as_bytes();
    if bytes.starts_with(b"\"\"\"") {
        decode_multi_line_string_literal(text)
    } else {
        decode_single_line_string_literal(text)
    }
}

pub(crate) fn decode_interpolated_text_part(
    text: &str,
    start: usize,
    end: usize,
) -> Result<String, &'static str> {
    let bounds = string_literal_bounds(text).map_err(|error| error.message)?;
    validate_multi_line_indentation(text, &bounds).map_err(|error| error.message)?;
    let raw = text
        .get(start..end)
        .ok_or("interpolated string text span is invalid")?;

    let decoded = if let Some(indent) = bounds.closing_indent {
        let mut dedented = String::with_capacity(raw.len());
        let mut index = start;
        while index < end {
            let at_line_start = index == bounds.content_start
                || text.as_bytes().get(index.wrapping_sub(1)) == Some(&b'\n');
            if at_line_start && text[index..end].starts_with(indent) {
                index += indent.len();
                continue;
            }

            let character = text[index..end]
                .chars()
                .next()
                .ok_or("invalid interpolated string text")?;
            dedented.push(character);
            index += character.len_utf8();
        }
        decode_escaped_text(&dedented, true)?
    } else {
        decode_escaped_text(raw, false)?
    };

    String::from_utf8(decoded).map_err(|_| "string literal escapes must decode to valid UTF-8")
}

pub(crate) fn decode_byte_literal(text: &str) -> Result<u8, &'static str> {
    let Some(content) = text
        .strip_prefix("b'")
        .and_then(|text| text.strip_suffix('\''))
    else {
        return Err("expected byte literal");
    };

    let bytes = decode_escaped_bytes(content, false, false)?;
    match bytes.as_slice() {
        [byte] => Ok(*byte),
        [] => Err("byte literal must decode exactly one byte"),
        _ => Err("byte literal must decode exactly one byte"),
    }
}

pub(crate) fn decode_integer_literal_value(text: &str) -> Option<u128> {
    let (base, digits) = if let Some(rest) = text.strip_prefix("0x") {
        (16, rest)
    } else if let Some(rest) = text.strip_prefix("0b") {
        (2, rest)
    } else {
        (10, text)
    };
    let digits = digits.replace('_', "");
    u128::from_str_radix(&digits, base).ok()
}

pub(crate) fn validate_string_literal_source(text: &str) -> Result<(), StringLiteralError> {
    let parts = string_literal_parts(text)?;
    if parts
        .iter()
        .all(|part| matches!(part, StringLiteralPartSpan::Text { .. }))
    {
        return decode_string_literal_bytes(text)
            .map(|_| ())
            .map_err(|message| StringLiteralError {
                start: 0,
                end: text.len(),
                message,
            });
    }

    let allow_raw_newlines = text.as_bytes().starts_with(b"\"\"\"");
    for part in parts {
        let StringLiteralPartSpan::Text { start, end } = part else {
            continue;
        };
        if start == end {
            continue;
        }

        decode_escaped_text(&text[start..end], allow_raw_newlines).map_err(|message| {
            StringLiteralError {
                start,
                end,
                message,
            }
        })?;
    }

    Ok(())
}

pub(crate) fn string_literal_parts(
    text: &str,
) -> Result<Vec<StringLiteralPartSpan>, StringLiteralError> {
    let bounds = string_literal_bounds(text)?;
    validate_multi_line_indentation(text, &bounds)?;

    let mut parts = Vec::new();
    let mut segment_start = bounds.content_start;
    let mut index = bounds.content_start;
    let bytes = text.as_bytes();

    while index < bounds.content_end {
        match bytes[index] {
            b'\\' => {
                index = (index + 2).min(bounds.content_end);
            }
            b'$' if bytes.get(index + 1) == Some(&b'{') => {
                if segment_start < index {
                    parts.push(StringLiteralPartSpan::Text {
                        start: segment_start,
                        end: index,
                    });
                }

                let closing_brace = find_interpolation_end(text, index, bounds.content_end)?;
                parts.push(StringLiteralPartSpan::Interpolation {
                    start: index,
                    expression_start: index + 2,
                    expression_end: closing_brace,
                    end: closing_brace + 1,
                });
                index = closing_brace + 1;
                segment_start = index;
            }
            _ => {
                index += current_char_len(text, index);
            }
        }
    }

    if segment_start < bounds.content_end || parts.is_empty() {
        parts.push(StringLiteralPartSpan::Text {
            start: segment_start,
            end: bounds.content_end,
        });
    }

    Ok(parts)
}

pub(crate) fn find_interpolation_end(
    text: &str,
    interpolation_start: usize,
    search_end: usize,
) -> Result<usize, StringLiteralError> {
    let bytes = text.as_bytes();
    let mut index = interpolation_start + 2;
    let mut depth = 1usize;

    while index < search_end {
        match bytes[index] {
            b'"' if bytes[index..search_end].starts_with(b"\"\"\"") => {
                index = skip_triple_quoted_source(text, index, search_end);
            }
            b'"' => {
                index = skip_quoted_source(text, index, search_end, b'"');
            }
            b'b' if bytes.get(index + 1) == Some(&b'\'') => {
                index = skip_quoted_source(text, index + 1, search_end, b'\'');
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while index < search_end && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < search_end && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(search_end);
            }
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(index);
                }
                index += 1;
            }
            _ => {
                index += current_char_len(text, index);
            }
        }
    }

    Err(StringLiteralError {
        start: interpolation_start,
        end: search_end,
        message: "unterminated string interpolation expression",
    })
}

fn decode_single_line_string_literal(text: &str) -> Result<Vec<u8>, &'static str> {
    let Some(content) = text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
    else {
        return Err("expected quoted string literal");
    };

    decode_escaped_text(content, false)
}

fn decode_multi_line_string_literal(text: &str) -> Result<Vec<u8>, &'static str> {
    let Some(content_with_opening_newline) = text
        .strip_prefix("\"\"\"")
        .and_then(|text| text.strip_suffix("\"\"\""))
    else {
        return Err("expected triple-quoted string literal");
    };

    let Some(content_with_closing_indent) = content_with_opening_newline.strip_prefix('\n') else {
        return Err("multi-line string opening delimiter must be followed by a newline");
    };

    let closing_line_start = content_with_closing_indent
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);

    let closing_indent = &content_with_closing_indent[closing_line_start..];
    if !closing_indent
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'))
    {
        return Err("multi-line string closing delimiter indentation is invalid");
    }

    let content = &content_with_closing_indent[..closing_line_start.saturating_sub(1)];
    let mut dedented = String::with_capacity(content.len());

    if !content.is_empty() {
        for (index, line) in content.split('\n').enumerate() {
            if index > 0 {
                dedented.push('\n');
            }

            if line.is_empty() {
                continue;
            }

            let Some(dedented_line) = line.strip_prefix(closing_indent) else {
                return Err(
                    "multi-line string content line must start with the closing delimiter indentation",
                );
            };
            dedented.push_str(dedented_line);
        }
    }

    decode_escaped_text(&dedented, true)
}

#[derive(Debug, Clone, Copy)]
struct StringLiteralBounds<'a> {
    content_start: usize,
    content_end: usize,
    closing_indent: Option<&'a str>,
}

fn string_literal_bounds(text: &str) -> Result<StringLiteralBounds<'_>, StringLiteralError> {
    if text.as_bytes().starts_with(b"\"\"\"") {
        return multi_line_string_literal_bounds(text);
    }

    if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
        return Ok(StringLiteralBounds {
            content_start: 1,
            content_end: text.len() - 1,
            closing_indent: None,
        });
    }

    Err(StringLiteralError {
        start: 0,
        end: text.len(),
        message: "expected quoted string literal",
    })
}

fn multi_line_string_literal_bounds(
    text: &str,
) -> Result<StringLiteralBounds<'_>, StringLiteralError> {
    if !text.starts_with("\"\"\"\n") {
        return Err(StringLiteralError {
            start: 0,
            end: text.len().min(3),
            message: "multi-line string opening delimiter must be followed by a newline",
        });
    }

    if !text.ends_with("\"\"\"") || text.len() < 6 {
        return Err(StringLiteralError {
            start: 0,
            end: text.len(),
            message: "expected triple-quoted string literal",
        });
    }

    let closing_delimiter_start = text.len() - 3;
    let closing_line_start = text[..closing_delimiter_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(4);
    let closing_indent = &text[closing_line_start..closing_delimiter_start];
    if !closing_indent
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'))
    {
        return Err(StringLiteralError {
            start: closing_line_start,
            end: closing_delimiter_start,
            message: "multi-line string closing delimiter indentation is invalid",
        });
    }

    let content_start = 4;
    let content_end = if closing_line_start == content_start {
        content_start
    } else {
        closing_line_start.saturating_sub(1)
    };

    Ok(StringLiteralBounds {
        content_start,
        content_end,
        closing_indent: Some(closing_indent),
    })
}

fn validate_multi_line_indentation(
    text: &str,
    bounds: &StringLiteralBounds<'_>,
) -> Result<(), StringLiteralError> {
    let Some(indent) = bounds.closing_indent else {
        return Ok(());
    };

    let mut line_start = bounds.content_start;
    while line_start < bounds.content_end {
        let line_end = text[line_start..bounds.content_end]
            .find('\n')
            .map(|offset| line_start + offset)
            .unwrap_or(bounds.content_end);
        let line = &text[line_start..line_end];
        if !line.is_empty() && !line.starts_with(indent) {
            return Err(StringLiteralError {
                start: line_start,
                end: line_end,
                message: "multi-line string content line must start with the closing delimiter indentation",
            });
        }

        if line_end == bounds.content_end {
            break;
        }
        line_start = line_end + 1;
    }

    Ok(())
}

fn decode_escaped_text(text: &str, allow_raw_newlines: bool) -> Result<Vec<u8>, &'static str> {
    let output = decode_escaped_bytes(text, allow_raw_newlines, true)?;
    str::from_utf8(&output).map_err(|_| "string literal escapes must decode to valid UTF-8")?;
    Ok(output)
}

fn decode_escaped_bytes(
    text: &str,
    allow_raw_newlines: bool,
    reject_interpolation: bool,
) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(text.len());
    let mut index = 0usize;

    while index < text.len() {
        let byte = text.as_bytes()[index];
        match byte {
            b'\\' => {
                index += 1;
                let escape = *text
                    .as_bytes()
                    .get(index)
                    .ok_or("unterminated escape sequence")?;
                match escape {
                    b'n' => output.push(b'\n'),
                    b'r' => output.push(b'\r'),
                    b't' => output.push(b'\t'),
                    b'0' => output.push(0),
                    b'\\' => output.push(b'\\'),
                    b'"' => output.push(b'"'),
                    b'\'' => output.push(b'\''),
                    b'$' => output.push(b'$'),
                    b'x' => {
                        let hi = *text
                            .as_bytes()
                            .get(index + 1)
                            .ok_or("incomplete hex escape")?;
                        let lo = *text
                            .as_bytes()
                            .get(index + 2)
                            .ok_or("incomplete hex escape")?;
                        output.push(decode_hex_byte(hi, lo).ok_or("invalid hex escape")?);
                        index += 2;
                    }
                    _ => return Err("invalid escape sequence"),
                }
                index += 1;
            }
            b'$' if reject_interpolation && text.as_bytes().get(index + 1) == Some(&b'{') => {
                return Err("string interpolation is not implemented yet");
            }
            b'\n' if allow_raw_newlines => {
                output.push(b'\n');
                index += 1;
            }
            b'\n' => return Err("raw newlines are invalid in single-line string literals"),
            b'\r' => return Err("bare carriage return is invalid in string literals"),
            _ => {
                let character = text[index..]
                    .chars()
                    .next()
                    .ok_or("invalid string literal")?;
                let mut buffer = [0; 4];
                output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                index += character.len_utf8();
            }
        }
    }

    Ok(output)
}

fn skip_quoted_source(text: &str, start_quote: usize, end: usize, quote: u8) -> usize {
    let bytes = text.as_bytes();
    let mut index = start_quote + 1;
    while index < end {
        if bytes[index] == b'\\' {
            index = (index + 2).min(end);
            continue;
        }

        if bytes[index] == quote {
            return index + 1;
        }

        index += current_char_len(text, index);
    }

    end
}

fn skip_triple_quoted_source(text: &str, start_quote: usize, end: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start_quote + 3;
    if bytes.get(index) != Some(&b'\n') {
        return end;
    }

    index += 1;
    let mut line_start = index;
    while index < end {
        if index == line_start {
            let mut indent_end = index;
            while indent_end < end && matches!(bytes.get(indent_end), Some(b' ' | b'\t')) {
                indent_end += 1;
            }

            if bytes[indent_end..end].starts_with(b"\"\"\"") {
                return indent_end + 3;
            }
        }

        if bytes[index] == b'\\' {
            index = (index + 2).min(end);
            continue;
        }

        if bytes[index] == b'\n' {
            index += 1;
            line_start = index;
            continue;
        }

        index += current_char_len(text, index);
    }

    end
}

fn current_char_len(text: &str, offset: usize) -> usize {
    text[offset..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

fn decode_hex_byte(hi: u8, lo: u8) -> Option<u8> {
    Some(hex_digit(hi)? << 4 | hex_digit(lo)?)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StringLiteralPartSpan, decode_byte_literal, decode_interpolated_text_part,
        decode_string_literal_bytes, string_literal_parts,
    };

    #[test]
    fn decodes_single_line_string_literal() {
        assert_eq!(
            decode_string_literal_bytes("\"a\\n\\$b\"").unwrap(),
            b"a\n$b"
        );
    }

    #[test]
    fn decodes_multi_line_string_literal_with_closing_indent() {
        assert_eq!(
            decode_string_literal_bytes("\"\"\"\n    alpha\n    beta\n    \"\"\"").unwrap(),
            b"alpha\nbeta"
        );
    }

    #[test]
    fn decodes_empty_multi_line_string_literal() {
        assert_eq!(decode_string_literal_bytes("\"\"\"\n\"\"\"").unwrap(), b"");
        assert_eq!(
            decode_string_literal_bytes("\"\"\"\n    \"\"\"").unwrap(),
            b""
        );
    }

    #[test]
    fn keeps_empty_multi_line_string_lines() {
        assert_eq!(
            decode_string_literal_bytes("\"\"\"\n    alpha\n\n    beta\n    \"\"\"").unwrap(),
            b"alpha\n\nbeta"
        );
    }

    #[test]
    fn rejects_multi_line_string_indent_mismatch() {
        let error =
            decode_string_literal_bytes("\"\"\"\n    alpha\n  beta\n    \"\"\"").unwrap_err();

        assert!(error.contains("indentation"));
    }

    #[test]
    fn rejects_unescaped_interpolation_start() {
        let error = decode_string_literal_bytes("\"hello ${name}\"").unwrap_err();

        assert!(error.contains("not a string literal"));
    }

    #[test]
    fn splits_interpolated_string_parts() {
        assert_eq!(
            string_literal_parts("\"hello ${name}!\"").unwrap(),
            vec![
                StringLiteralPartSpan::Text { start: 1, end: 7 },
                StringLiteralPartSpan::Interpolation {
                    start: 7,
                    expression_start: 9,
                    expression_end: 13,
                    end: 14,
                },
                StringLiteralPartSpan::Text { start: 14, end: 15 },
            ]
        );
    }

    #[test]
    fn splits_multi_line_interpolated_string_parts() {
        let parts = string_literal_parts("\"\"\"\n    hello ${name}\n    \"\"\"").unwrap();

        assert!(matches!(
            parts[1],
            StringLiteralPartSpan::Interpolation {
                expression_start: 16,
                expression_end: 20,
                ..
            }
        ));
    }

    #[test]
    fn decodes_interpolated_text_escapes() {
        let source = "\"line\\n${value}\\$\"";
        assert_eq!(decode_interpolated_text_part(source, 1, 7).unwrap(), "line\n");
        assert_eq!(
            decode_interpolated_text_part(source, 15, 17).unwrap(),
            "$"
        );
    }

    #[test]
    fn dedents_each_multi_line_interpolated_text_part() {
        let source = "\"\"\"\n    ${first}\n    middle ${second}\n    tail\n    \"\"\"";
        let parts = string_literal_parts(source).unwrap();
        let decoded = parts
            .into_iter()
            .filter_map(|part| match part {
                StringLiteralPartSpan::Text { start, end } => {
                    Some(decode_interpolated_text_part(source, start, end).unwrap())
                }
                StringLiteralPartSpan::Interpolation { .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(decoded, vec!["", "\nmiddle ", "\ntail"]);
    }

    #[test]
    fn rejects_invalid_utf8_escape_sequence() {
        let error = decode_string_literal_bytes("\"\\xFF\"").unwrap_err();

        assert!(error.contains("valid UTF-8"));
    }

    #[test]
    fn decodes_byte_literal() {
        assert_eq!(decode_byte_literal("b'a'").unwrap(), b'a');
        assert_eq!(decode_byte_literal("b'\\n'").unwrap(), b'\n');
        assert_eq!(decode_byte_literal("b'\\xFF'").unwrap(), 0xFF);
    }

    #[test]
    fn rejects_byte_literal_with_zero_or_multiple_bytes() {
        assert!(decode_byte_literal("b''").unwrap_err().contains("one byte"));
        assert!(
            decode_byte_literal("b'ab'")
                .unwrap_err()
                .contains("one byte")
        );
        assert!(
            decode_byte_literal("b'é'")
                .unwrap_err()
                .contains("one byte")
        );
    }
}

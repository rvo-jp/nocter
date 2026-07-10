//! Literal decoding shared by compiler phases.

use std::str;

pub(crate) fn decode_string_literal_bytes(text: &str) -> Result<Vec<u8>, &'static str> {
    let bytes = text.as_bytes();
    if bytes.starts_with(b"\"\"\"") {
        decode_multi_line_string_literal(text)
    } else {
        decode_single_line_string_literal(text)
    }
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

fn decode_escaped_text(text: &str, allow_raw_newlines: bool) -> Result<Vec<u8>, &'static str> {
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
            b'$' if text.as_bytes().get(index + 1) == Some(&b'{') => {
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

    str::from_utf8(&output).map_err(|_| "string literal escapes must decode to valid UTF-8")?;
    Ok(output)
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
    use super::decode_string_literal_bytes;

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

        assert!(error.contains("interpolation"));
    }

    #[test]
    fn rejects_invalid_utf8_escape_sequence() {
        let error = decode_string_literal_bytes("\"\\xFF\"").unwrap_err();

        assert!(error.contains("valid UTF-8"));
    }
}

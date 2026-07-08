use crate::ast::{CallExpr, Expr};
use crate::diagnostics::Diagnostic;

pub(super) fn lower_make_error_message(expression: &Expr) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let Expr::Call(call) = expression else {
        return Err(unsupported_fail_payload_diagnostic());
    };

    if !is_make_error_call(call) || call.arguments.len() != 2 {
        return Err(unsupported_fail_payload_diagnostic());
    }

    let Expr::StringLiteral(message) = &call.arguments[1] else {
        return Err(unsupported_fail_payload_diagnostic());
    };

    decode_string_literal(&message.value).map_err(|message| {
        vec![Diagnostic::error(
            "E8005",
            format!("IR v0 cannot decode failure message literal: {message}"),
        )]
    })
}

pub(super) fn with_trailing_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}

fn is_make_error_call(call: &CallExpr) -> bool {
    matches!(call.callee.as_ref(), Expr::Identifier(identifier) if identifier.name == "make_error")
}

fn unsupported_fail_payload_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8004",
        "IR v0 can only lower `return make_error(<string code>, <string message>)` as fallible failure",
    )]
}

fn decode_string_literal(text: &str) -> Result<Vec<u8>, &'static str> {
    let Some(content) = text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
    else {
        return Err("expected quoted string literal");
    };

    let mut bytes = Vec::new();
    let mut index = 0;

    while index < content.len() {
        let byte = content.as_bytes()[index];
        if byte != b'\\' {
            let character = content[index..]
                .chars()
                .next()
                .ok_or("invalid string literal")?;
            let mut buffer = [0; 4];
            bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            index += character.len_utf8();
            continue;
        }

        index += 1;
        let escape = *content
            .as_bytes()
            .get(index)
            .ok_or("unterminated escape sequence")?;
        match escape {
            b'n' => bytes.push(b'\n'),
            b'r' => bytes.push(b'\r'),
            b't' => bytes.push(b'\t'),
            b'0' => bytes.push(0),
            b'\\' => bytes.push(b'\\'),
            b'"' => bytes.push(b'"'),
            b'\'' => bytes.push(b'\''),
            b'x' => {
                let hi = *content
                    .as_bytes()
                    .get(index + 1)
                    .ok_or("incomplete hex escape")?;
                let lo = *content
                    .as_bytes()
                    .get(index + 2)
                    .ok_or("incomplete hex escape")?;
                bytes.push(decode_hex_byte(hi, lo).ok_or("invalid hex escape")?);
                index += 2;
            }
            _ => return Err("invalid escape sequence"),
        }
        index += 1;
    }

    Ok(bytes)
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

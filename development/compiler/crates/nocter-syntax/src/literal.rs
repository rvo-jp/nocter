pub(super) fn valid_integer(text: &str) -> bool {
    let (digits, valid_digit): (&str, fn(char) -> bool) =
        if let Some(digits) = text.strip_prefix("0x") {
            (digits, |character| character.is_ascii_hexdigit())
        } else if let Some(digits) = text.strip_prefix("0b") {
            (digits, |character| matches!(character, '0' | '1'))
        } else {
            (text, |character| character.is_ascii_digit())
        };

    if digits.is_empty() {
        return false;
    }

    let characters: Vec<_> = digits.chars().collect();
    characters.iter().enumerate().all(|(index, character)| {
        if valid_digit(*character) {
            true
        } else if *character == '_' {
            index > 0
                && index + 1 < characters.len()
                && valid_digit(characters[index - 1])
                && valid_digit(characters[index + 1])
        } else {
            false
        }
    })
}

pub(super) fn decode_escape(
    bytes: &[u8],
    start: usize,
    limit: usize,
) -> Result<(u8, usize), usize> {
    let Some(next) = bytes.get(start + 1).copied().filter(|_| start + 1 < limit) else {
        return Err(1);
    };
    let simple = match next {
        b'n' => Some(b'\n'),
        b'r' => Some(b'\r'),
        b't' => Some(b'\t'),
        b'0' => Some(b'\0'),
        b'\\' => Some(b'\\'),
        b'"' => Some(b'"'),
        b'\'' => Some(b'\''),
        b'$' => Some(b'$'),
        _ => None,
    };
    if let Some(byte) = simple {
        return Ok((byte, 2));
    }

    if next == b'x' {
        if start + 4 <= limit {
            let high = hex_value(bytes[start + 2]);
            let low = hex_value(bytes[start + 3]);
            if let (Some(high), Some(low)) = (high, low) {
                return Ok(((high << 4) | low, 4));
            }
        }
        return Err((limit - start).min(4));
    }

    Err((limit - start).min(2))
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

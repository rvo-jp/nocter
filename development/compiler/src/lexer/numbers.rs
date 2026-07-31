#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::lexer) enum NumberBase {
    Decimal,
    Hex,
    Binary,
}

pub(in crate::lexer) fn is_number_body_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

pub(in crate::lexer) fn is_hex_digit(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

pub(in crate::lexer) fn validate_integer_literal(
    text: &str,
    base: NumberBase,
) -> Result<(), &'static str> {
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

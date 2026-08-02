//! Completion-only source recovery for open documents that do not parse yet.

const COMPLETION_PLACEHOLDER_IDENT: &str = "__nocter_completion_placeholder";

pub(crate) fn completion_recovery_text(text: &str, offset: usize) -> Option<String> {
    completion_recovery_overlay(text, offset).map(|(text, _)| text)
}

pub(crate) fn completion_recovery_overlay(text: &str, offset: usize) -> Option<(String, usize)> {
    super::interpolation_completion_recovery_overlay(text, offset).or_else(|| {
        incomplete_member_completion_text(text, offset)
            .or_else(|| incomplete_struct_literal_field_completion_text(text, offset))
            .or_else(|| incomplete_import_symbol_completion_text(text, offset))
            .map(|text| (text, offset))
            .or_else(|| super::region_recovery::region_recovery_overlay(text, offset))
    })
}

fn incomplete_import_symbol_completion_text(text: &str, offset: usize) -> Option<String> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = text[line_start..offset].trim_start();
    let use_body = prefix
        .strip_prefix("use ")
        .or_else(|| prefix.strip_prefix("pub use "))
        .or_else(|| prefix.strip_prefix("nocter use "))?;
    let dot = use_body.rfind('.')?;
    let path = &use_body[..dot];
    if path.is_empty() || path.ends_with('/') || path == "." || path.ends_with("..") {
        return None;
    }
    let selector = &use_body[dot + 1..];
    if !selector
        .bytes()
        .all(|byte| byte == b'{' || byte == b',' || byte.is_ascii_whitespace())
    {
        return None;
    }

    let mut insertion = COMPLETION_PLACEHOLDER_IDENT.to_string();
    if selector.contains('{') {
        insertion.push('}');
    }
    let mut recovered = String::with_capacity(text.len() + insertion.len());
    recovered.push_str(&text[..offset]);
    recovered.push_str(&insertion);
    recovered.push_str(&text[offset..]);
    Some(recovered)
}

#[cfg(test)]
pub(crate) fn signature_recovery_text(text: &str, offset: usize) -> Option<String> {
    signature_recovery_texts(text, offset).into_iter().next()
}

/// Returns parseable call overlays in decreasing likelihood order.
///
/// An empty active argument is ambiguous: it can be the missing argument of a
/// non-empty callable or an unfinished zero-parameter call. Trying both keeps
/// recovery syntax-driven instead of teaching it callable names or arities.
pub(crate) fn signature_recovery_texts(text: &str, offset: usize) -> Vec<String> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return Vec::new();
    }
    let unmatched = unmatched_parentheses_before(text, offset);
    let Some(call_open) = unmatched.last().copied() else {
        return Vec::new();
    };
    if !parenthesis_follows_callable(text, call_open) {
        return Vec::new();
    }

    let needs_argument =
        previous_non_whitespace_byte(text, offset).is_some_and(|byte| matches!(byte, b'(' | b','));
    let closing = ")".repeat(unmatched.len());
    let mut insertions = Vec::new();
    if needs_argument {
        insertions.push(format!("0{closing}"));
    }
    insertions.push(closing);

    insertions
        .into_iter()
        .map(|insertion| {
            let mut recovered = String::with_capacity(text.len() + insertion.len());
            recovered.push_str(&text[..offset]);
            recovered.push_str(&insertion);
            recovered.push_str(&text[offset..]);
            recovered
        })
        .collect()
}

#[cfg(test)]
fn signature_recovery_text_without_placeholder(text: &str, offset: usize) -> Option<String> {
    let recoveries = signature_recovery_texts(text, offset);
    if recoveries.len() < 2 {
        return None;
    }
    recoveries.into_iter().nth(1)
}

fn unmatched_parentheses_before(text: &str, offset: usize) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut stack = Vec::new();
    let mut index = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment_depth = 0usize;

    while index < offset {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment_depth != 0 {
            if byte == b'/' && next == Some(b'*') {
                block_comment_depth += 1;
                index += 2;
            } else if byte == b'*' && next == Some(b'/') {
                block_comment_depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && next == Some(b'/') {
            line_comment = true;
            index += 2;
        } else if byte == b'/' && next == Some(b'*') {
            block_comment_depth = 1;
            index += 2;
        } else if byte == b'"' {
            in_string = true;
            index += 1;
        } else {
            match byte {
                b'(' => stack.push(index),
                b')' => {
                    stack.pop();
                }
                _ => {}
            }
            index += 1;
        }
    }
    stack
}

fn parenthesis_follows_callable(text: &str, open: usize) -> bool {
    let Some(prefix) = text.as_bytes().get(..open) else {
        return false;
    };
    prefix
        .iter()
        .rev()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| byte == b')' || byte == b'_' || byte.is_ascii_alphanumeric())
}

fn incomplete_member_completion_text(text: &str, offset: usize) -> Option<String> {
    if !offset_is_after_member_dot(text, offset) {
        return None;
    }

    let mut completion_text =
        String::with_capacity(text.len() + COMPLETION_PLACEHOLDER_IDENT.len());
    completion_text.push_str(&text[..offset]);
    completion_text.push_str(COMPLETION_PLACEHOLDER_IDENT);
    completion_text.push_str(&text[offset..]);
    Some(completion_text)
}

fn offset_is_after_member_dot(text: &str, offset: usize) -> bool {
    offset > 0 && text.is_char_boundary(offset) && text.as_bytes().get(offset - 1) == Some(&b'.')
}

fn incomplete_struct_literal_field_completion_text(text: &str, offset: usize) -> Option<String> {
    if !offset_is_after_struct_literal_field_boundary(text, offset) {
        return None;
    }

    let needs_closing_brace = next_non_whitespace_byte(text, offset) != Some(b'}');
    let insertion = if needs_closing_brace {
        format!("{COMPLETION_PLACEHOLDER_IDENT}: none }}")
    } else {
        format!("{COMPLETION_PLACEHOLDER_IDENT}: none")
    };
    let mut completion_text = String::with_capacity(text.len() + insertion.len());
    completion_text.push_str(&text[..offset]);
    completion_text.push_str(&insertion);
    completion_text.push_str(&text[offset..]);
    Some(completion_text)
}

fn offset_is_after_struct_literal_field_boundary(text: &str, offset: usize) -> bool {
    if !text.is_char_boundary(offset) {
        return false;
    }
    previous_non_whitespace_byte(text, offset).is_some_and(|byte| matches!(byte, b'{' | b','))
}

fn previous_non_whitespace_byte(text: &str, offset: usize) -> Option<u8> {
    text.as_bytes()
        .get(..offset)?
        .iter()
        .rev()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

fn next_non_whitespace_byte(text: &str, offset: usize) -> Option<u8> {
    text.as_bytes()
        .get(offset..)?
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_incomplete_call_after_current_argument() {
        let text = "func main(): i32 {\n    return add(20, 22\n}\n";
        let offset = text.find("22").unwrap() + 2;
        let recovered = signature_recovery_text(text, offset).expect("expected recovery");

        assert!(recovered.contains("add(20, 22)\n"), "{recovered}");
    }

    #[test]
    fn inserts_placeholder_for_empty_active_argument() {
        let text = "func main(): i32 {\n    return add(20, \n}\n";
        let offset = text.find("20, ").unwrap() + "20, ".len();
        let recovered = signature_recovery_text(text, offset).expect("expected recovery");

        assert!(recovered.contains("add(20, 0)\n"), "{recovered}");
    }

    #[test]
    fn also_closes_an_empty_zero_parameter_call_without_an_argument() {
        let text = "func main(): i32 {\n    return iterator.next(\n}\n";
        let offset = text.find("next(").unwrap() + "next(".len();
        let recovered = signature_recovery_text_without_placeholder(text, offset)
            .expect("expected zero-parameter recovery");

        assert!(recovered.contains("iterator.next()\n"), "{recovered}");
    }

    #[test]
    fn ignores_parentheses_in_strings_and_comments() {
        let text = "func main(): i32 {\n    // ignored(\n    return parse(\"(\"\n}\n";
        let offset = text.rfind("\"").unwrap() + 1;
        let recovered = signature_recovery_text(text, offset).expect("expected recovery");

        assert!(recovered.contains("parse(\"(\")\n"), "{recovered}");
    }

    #[test]
    fn inserts_placeholder_for_incomplete_import_selector() {
        let text = "use std/vec.\n";
        let offset = text.find('.').unwrap() + 1;
        let recovered = completion_recovery_text(text, offset).expect("expected recovery");

        assert_eq!(recovered, "use std/vec.__nocter_completion_placeholder\n");
    }
}

//! Completion-only source recovery for open documents that do not parse yet.

const COMPLETION_PLACEHOLDER_IDENT: &str = "__nocter_completion_placeholder";

pub(super) fn completion_recovery_text(text: &str, offset: usize) -> Option<String> {
    incomplete_member_completion_text(text, offset)
        .or_else(|| incomplete_struct_literal_field_completion_text(text, offset))
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

//! Source overlays for incomplete typed literal expressions and declaration bodies.

pub(crate) fn literal_recovery_text(text: &str, offset: usize) -> Option<String> {
    literal_recovery_overlay(text, offset).map(|(text, _)| text)
}

pub(crate) fn literal_recovery_overlay(text: &str, offset: usize) -> Option<(String, usize)> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    incomplete_sequence_expression(text, offset)
        .or_else(|| incomplete_string_expression(text, offset))
        .or_else(|| missing_literal_shape(text, offset))
        .or_else(|| incomplete_literal_body(text, offset))
        .map(|recovered| (recovered, offset))
}

fn incomplete_sequence_expression(text: &str, offset: usize) -> Option<String> {
    let scan = scan_to(text, offset);
    let open = *scan.square_brackets.last()?;
    target_precedes_delimiter(text, open)?;
    Some(insert_at(text, offset, "]"))
}

fn incomplete_string_expression(text: &str, offset: usize) -> Option<String> {
    let scan = scan_to(text, offset);
    let quote = scan.open_string?;
    target_precedes_delimiter(text, quote)?;
    Some(insert_at(text, offset, "\""))
}

fn missing_literal_shape(text: &str, offset: usize) -> Option<String> {
    let prefix = text.get(..offset)?;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let line = prefix[line_start..].trim_start();
    if line.starts_with("literal ")
        || line.starts_with("pub literal ")
        || line.starts_with("nocter literal ")
    {
        return None;
    }
    let token_end = prefix
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(index, character)| index + character.len_utf8())?;
    if token_end == offset {
        return None;
    }
    let last = text.as_bytes().get(token_end.saturating_sub(1)).copied()?;
    if !(last == b'>' || last == b'_' || last.is_ascii_alphanumeric()) {
        return None;
    }
    Some(insert_at(text, offset, "[]"))
}

fn incomplete_literal_body(text: &str, _offset: usize) -> Option<String> {
    let scan = scan_to(text, text.len());
    let open = *scan.curly_braces.last()?;
    let prefix = text.get(..open)?;
    let item_start = prefix.rfind("literal ")?;
    let boundary = prefix[..item_start]
        .chars()
        .next_back()
        .is_none_or(|character| !is_identifier_continue(character));
    if !boundary {
        return None;
    }
    if prefix[item_start..].contains('}') {
        return None;
    }
    let mut recovered = text.to_string();
    recovered.push('}');
    Some(recovered)
}

fn target_precedes_delimiter(text: &str, delimiter: usize) -> Option<()> {
    let before = text.as_bytes().get(..delimiter)?;
    let whitespace_start = before
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())?
        + 1;
    if whitespace_start == delimiter {
        return None;
    }
    let previous = before.get(whitespace_start.saturating_sub(1)).copied()?;
    (previous == b'>' || previous == b'_' || previous.is_ascii_alphanumeric()).then_some(())
}

fn insert_at(text: &str, offset: usize, insertion: &str) -> String {
    let mut recovered = String::with_capacity(text.len() + insertion.len());
    recovered.push_str(&text[..offset]);
    recovered.push_str(insertion);
    recovered.push_str(&text[offset..]);
    recovered
}

#[derive(Default)]
struct DelimiterScan {
    square_brackets: Vec<usize>,
    curly_braces: Vec<usize>,
    open_string: Option<usize>,
}

fn scan_to(text: &str, offset: usize) -> DelimiterScan {
    let bytes = text.as_bytes();
    let mut scan = DelimiterScan::default();
    let mut index = 0usize;
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
        if scan.open_string.is_some() {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                scan.open_string = None;
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
        } else {
            match byte {
                b'"' => scan.open_string = Some(index),
                b'[' => scan.square_brackets.push(index),
                b']' => {
                    scan.square_brackets.pop();
                }
                b'{' => scan.curly_braces.push(index),
                b'}' => {
                    scan.curly_braces.pop();
                }
                _ => {}
            }
            index += 1;
        }
    }
    scan
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_incomplete_sequence_expression_at_cursor() {
        let text = "func main(): i32 {\n    let values: Vec<i32> = Vec [\n}\n";
        let offset = text.find("[\n").unwrap() + 1;
        let recovered = literal_recovery_text(text, offset).expect("expected recovery");
        assert!(recovered.contains("Vec []\n"), "{recovered}");
    }

    #[test]
    fn inserts_missing_shape_after_a_target() {
        let text = "func main(): i32 {\n    let values: Vec<i32> = Vec \n}\n";
        let offset = text.find("Vec \n").unwrap() + 4;
        let recovered = literal_recovery_text(text, offset).expect("expected recovery");
        assert!(recovered.contains("Vec []\n"), "{recovered}");
    }

    #[test]
    fn closes_incomplete_string_expression() {
        let text = "func main(): i32 {\n    let text = String \"hel\n}\n";
        let offset = text.find("hel").unwrap() + 3;
        let recovered = literal_recovery_text(text, offset).expect("expected recovery");
        assert!(recovered.contains("String \"hel\"\n"), "{recovered}");
    }

    #[test]
    fn closes_incomplete_literal_declaration_body() {
        let text = "struct Text {}\nliteral Text \"\"(text: &str): Self {\n";
        let recovered = literal_recovery_text(text, text.len()).expect("expected recovery");
        assert!(recovered.ends_with("{\n}"), "{recovered}");
    }
}

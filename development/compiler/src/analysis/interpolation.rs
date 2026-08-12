//! Editor-facing interpolation facts and incomplete-source recovery.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{Expr, InterpolatedStringPart, visit_file_expressions};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterpolationEditorInfo {
    pub(crate) expression_span: ByteSpan,
    pub(crate) focus_span: ByteSpan,
    pub(crate) label: String,
    pub(crate) documentation: String,
}

pub(crate) fn interpolation_editor_info_at_offset(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<InterpolationEditorInfo> {
    let mut candidates = Vec::new();
    visit_file_expressions(&file.ast, &mut |expression| {
        let Expr::InterpolatedString(interpolated) = expression else {
            return;
        };
        if !span_contains(interpolated.span, offset) {
            return;
        }
        let Some(plan) = file.typecheck_facts.interpolation_plan(interpolated.span) else {
            return;
        };
        let mut focus = interpolated.span;
        let mut accepted = None;
        for (part, planned) in interpolated.parts.iter().zip(&plan.parts) {
            let part_span = match part {
                InterpolatedStringPart::Text(text) => text.span,
                InterpolatedStringPart::Expression(part) => part.span,
            };
            if !span_contains(part_span, offset) {
                continue;
            }
            if let InterpolatedStringPart::Expression(part) = part
                && span_contains(part.expression_span, offset)
            {
                return;
            }
            focus = part_span;
            accepted = Some(&planned.accepted_type);
            break;
        }
        let Some(result_name) = resolved_type_label(analysis, plan.string_type_definition) else {
            return;
        };
        let mut documentation = Vec::new();
        if let Some(input) = accepted {
            documentation.push(format!(
                "**Accepted interpolation input:** `{}`.",
                crate::typecheck::type_expr_presentation_label(input, &file.resolved)
            ));
            let Some(contract) = resolved_type_label(analysis, plan.format_interface_definition)
            else {
                return;
            };
            documentation.push(format!("**Formatting contract:** `{contract}`."));
        }
        candidates.push(InterpolationEditorInfo {
            expression_span: interpolated.span,
            focus_span: focus,
            label: format!("interpolated string: {result_name}"),
            documentation: documentation.join("\n\n"),
        });
    });
    candidates
        .into_iter()
        .min_by_key(|candidate| candidate.expression_span.end - candidate.expression_span.start)
}

fn resolved_type_label(
    analysis: &CompileUnitAnalysis,
    definition: crate::semantic::DefId,
) -> Option<String> {
    analysis.files.iter().find_map(|candidate_file| {
        candidate_file
            .resolved
            .symbols
            .symbols()
            .find_map(|symbol| {
                let crate::resolve::SymbolKind::Type(type_symbol) = &symbol.kind else {
                    return None;
                };
                (symbol.def_id == definition).then(|| {
                    crate::typecheck::type_symbol_presentation_label(
                        type_symbol,
                        &candidate_file.resolved,
                    )
                })
            })
    })
}

pub(crate) fn interpolation_recovery_text(text: &str, offset: usize) -> Option<String> {
    active_interpolation_expression_start(text, offset)?;
    Some(insert_at_offset(
        text,
        offset,
        &interpolation_closing_suffix(text, offset),
    ))
}

pub(crate) fn interpolation_completion_recovery_overlay(
    text: &str,
    offset: usize,
) -> Option<(String, usize)> {
    let expression_start = active_interpolation_expression_start(text, offset)?;
    let needs_placeholder = text[expression_start..offset].trim().is_empty();
    let member_placeholder = (offset > expression_start
        && text.as_bytes().get(offset - 1) == Some(&b'.'))
    .then_some("__nocter_completion_placeholder");
    let placeholder = member_placeholder
        .or(needs_placeholder.then_some("__nocter_completion_placeholder"))
        .unwrap_or("");
    let insertion = format!(
        "{placeholder}{}",
        interpolation_closing_suffix(text, offset)
    );
    Some((insert_at_offset(text, offset, &insertion), offset))
}

pub(crate) fn interpolation_signature_recovery_texts(text: &str, offset: usize) -> Vec<String> {
    let Some(expression_start) = active_interpolation_expression_start(text, offset) else {
        return Vec::new();
    };
    let expression = &text[expression_start..offset];
    let unmatched = unmatched_parentheses(expression);
    let Some(open) = unmatched.last().copied() else {
        return Vec::new();
    };
    if !parenthesis_follows_callable(expression, open) {
        return Vec::new();
    }
    let needs_argument =
        previous_non_whitespace_byte(expression).is_some_and(|byte| matches!(byte, b'(' | b','));
    let closing = ")".repeat(unmatched.len());
    let suffix = interpolation_closing_suffix(text, offset);
    let mut insertions = Vec::new();
    if needs_argument {
        insertions.push(format!("0{closing}{suffix}"));
    }
    insertions.push(format!("{closing}{suffix}"));
    insertions
        .into_iter()
        .map(|insertion| insert_at_offset(text, offset, &insertion))
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum ScanMode {
    Code,
    Quoted {
        delimiter: u8,
        width: usize,
    },
    Interpolation {
        expression_start: usize,
        depth: usize,
    },
    LineComment,
    BlockComment {
        depth: usize,
    },
}

fn active_interpolation_expression_start(text: &str, offset: usize) -> Option<usize> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    let bytes = text.as_bytes();
    let mut modes = vec![ScanMode::Code];
    let mut index = 0usize;
    while index < offset {
        match *modes.last()? {
            ScanMode::Code => {
                if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
                    modes.push(ScanMode::LineComment);
                    index += 2;
                } else if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
                    modes.push(ScanMode::BlockComment { depth: 1 });
                    index += 2;
                } else if bytes[index] == b'"' {
                    let width = usize::from(bytes[index..offset].starts_with(b"\"\"\"")) * 2 + 1;
                    modes.push(ScanMode::Quoted {
                        delimiter: b'"',
                        width,
                    });
                    index += width;
                } else {
                    index += char_len(text, index);
                }
            }
            ScanMode::Quoted { delimiter, width } => {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(offset);
                } else if delimiter == b'"'
                    && bytes[index] == b'$'
                    && bytes.get(index + 1) == Some(&b'{')
                {
                    modes.push(ScanMode::Interpolation {
                        expression_start: index + 2,
                        depth: 1,
                    });
                    index += 2;
                } else if delimiter_at(bytes, index, offset, delimiter, width) {
                    modes.pop();
                    index += width;
                } else {
                    index += char_len(text, index);
                }
            }
            ScanMode::Interpolation {
                expression_start,
                depth,
            } => match bytes[index] {
                b'"' => {
                    let width = usize::from(bytes[index..offset].starts_with(b"\"\"\"")) * 2 + 1;
                    modes.push(ScanMode::Quoted {
                        delimiter: b'"',
                        width,
                    });
                    index += width;
                }
                b'\'' => {
                    modes.push(ScanMode::Quoted {
                        delimiter: b'\'',
                        width: 1,
                    });
                    index += 1;
                }
                b'/' if bytes.get(index + 1) == Some(&b'/') => {
                    modes.push(ScanMode::LineComment);
                    index += 2;
                }
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    modes.push(ScanMode::BlockComment { depth: 1 });
                    index += 2;
                }
                b'{' => {
                    *modes.last_mut()? = ScanMode::Interpolation {
                        expression_start,
                        depth: depth + 1,
                    };
                    index += 1;
                }
                b'}' if depth == 1 => {
                    modes.pop();
                    index += 1;
                }
                b'}' => {
                    *modes.last_mut()? = ScanMode::Interpolation {
                        expression_start,
                        depth: depth - 1,
                    };
                    index += 1;
                }
                _ => index += char_len(text, index),
            },
            ScanMode::LineComment => {
                if bytes[index] == b'\n' {
                    modes.pop();
                }
                index += 1;
            }
            ScanMode::BlockComment { depth } => {
                if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
                    *modes.last_mut()? = ScanMode::BlockComment { depth: depth + 1 };
                    index += 2;
                } else if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    if depth == 1 {
                        modes.pop();
                    } else {
                        *modes.last_mut()? = ScanMode::BlockComment { depth: depth - 1 };
                    }
                    index += 2;
                } else {
                    index += char_len(text, index);
                }
            }
        }
    }
    match modes.last()? {
        ScanMode::Interpolation {
            expression_start, ..
        } => Some(*expression_start),
        ScanMode::Code
        | ScanMode::Quoted { .. }
        | ScanMode::LineComment
        | ScanMode::BlockComment { .. } => None,
    }
}

fn interpolation_closing_suffix(text: &str, offset: usize) -> String {
    let remainder = &text[offset..];
    let has_closing_quote = remainder
        .bytes()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count()
        < remainder.len()
        && remainder.bytes().find(|byte| !byte.is_ascii_whitespace()) == Some(b'"');
    if has_closing_quote {
        "}".to_string()
    } else {
        "}\"".to_string()
    }
}

fn unmatched_parentheses(text: &str) -> Vec<usize> {
    let mut stack = Vec::new();
    let mut index = 0usize;
    let bytes = text.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    while index < text.len() {
        let byte = bytes[index];
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == delimiter {
                quote = None;
            }
        } else if matches!(byte, b'"' | b'\'') {
            quote = Some(byte);
        } else if byte == b'(' {
            stack.push(index);
        } else if byte == b')' {
            stack.pop();
        }
        index += char_len(text, index);
    }
    stack
}

fn parenthesis_follows_callable(text: &str, open: usize) -> bool {
    text.as_bytes()[..open]
        .iter()
        .rev()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| byte == b')' || byte == b'_' || byte.is_ascii_alphanumeric())
}

fn previous_non_whitespace_byte(text: &str) -> Option<u8> {
    text.bytes().rev().find(|byte| !byte.is_ascii_whitespace())
}

fn insert_at_offset(text: &str, offset: usize, insertion: &str) -> String {
    let mut recovered = String::with_capacity(text.len() + insertion.len());
    recovered.push_str(&text[..offset]);
    recovered.push_str(insertion);
    recovered.push_str(&text[offset..]);
    recovered
}

fn char_len(text: &str, offset: usize) -> usize {
    text[offset..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

fn delimiter_at(bytes: &[u8], index: usize, end: usize, delimiter: u8, width: usize) -> bool {
    index + width <= end
        && bytes[index..index + width]
            .iter()
            .all(|byte| *byte == delimiter)
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_an_incomplete_interpolation_without_moving_the_cursor() {
        let text = "func main(): i32 {\n    let text = \"value ${count\n}\n";
        let offset = text.find("count").unwrap() + "count".len();
        let (recovered, recovered_offset) =
            interpolation_completion_recovery_overlay(text, offset).unwrap();

        assert_eq!(recovered_offset, offset);
        assert!(recovered.contains("\"value ${count}\"\n"), "{recovered}");
    }

    #[test]
    fn completes_an_empty_interpolation_with_a_synthetic_expression() {
        let text = "func main(): i32 {\n    let text = \"value ${\n}\n";
        let offset = text.find("${").unwrap() + 2;
        let (recovered, _) = interpolation_completion_recovery_overlay(text, offset).unwrap();

        assert!(recovered.contains("${__nocter_completion_placeholder}\""));
    }

    #[test]
    fn closes_a_nested_call_for_signature_recovery() {
        let text = "func main(): i32 {\n    let text = \"value ${format(1, \n}\n";
        let offset = text.find("1, ").unwrap() + 3;
        let recoveries = interpolation_signature_recovery_texts(text, offset);

        assert!(
            recoveries
                .iter()
                .any(|text| text.contains("format(1, 0)}\""))
        );
    }

    #[test]
    fn ignores_interpolation_spelling_inside_nested_expression_strings() {
        let text = "\"outer ${call(\"\\${not_active";
        assert_eq!(
            active_interpolation_expression_start(text, text.len()),
            None
        );
    }

    #[test]
    fn ignores_interpolation_spelling_in_comments() {
        let text = "// \"ignored ${value\nfunc main(): i32 { return 0 }";
        assert_eq!(
            active_interpolation_expression_start(text, text.find("value").unwrap()),
            None
        );
    }
}

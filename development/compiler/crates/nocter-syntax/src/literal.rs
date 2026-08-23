use nocter_source::SourceFile;

use crate::{NodeId, NodeKind, StringDelimiter, SyntaxElement, SyntaxTree, TokenKind};

/// One decoded ordinary string-expression part in exact source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedStringPart {
    Text(Box<str>),
    Expression(NodeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthoredStringUnit {
    Byte(u8),
    Expression(NodeId),
}

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

/// Decodes one parser-validated, non-interpolated string literal.
///
/// The syntax tree remains lossless; semantic consumers call this boundary rather than each
/// implementing escape and multiline-indentation rules independently.
#[must_use]
pub fn decode_string_literal(
    source: &SourceFile,
    tree: &SyntaxTree,
    node: NodeId,
) -> Option<Box<str>> {
    let mut delimiter = None;
    let mut text = "";
    for element in tree.children(node) {
        let SyntaxElement::Token(token) = element else {
            continue;
        };
        match token.kind() {
            TokenKind::StringStart(found) => delimiter = Some(found),
            TokenKind::StringText => text = source.text_at(token.range())?,
            _ => {}
        }
    }
    decode_authored_string(delimiter?, text)
}

/// Decodes a parser-validated ordinary string expression when it contains no interpolation.
///
/// Interpolated forms deliberately return `None`; their source-ordered text and expression parts
/// require the interpolation planner rather than pretending to be one static literal.
#[must_use]
pub fn decode_plain_string_expression(
    source: &SourceFile,
    tree: &SyntaxTree,
    node: NodeId,
) -> Option<Box<str>> {
    let parts = decode_string_expression(source, tree, node)?;
    let mut text = String::new();
    for part in parts {
        match part {
            DecodedStringPart::Text(part) => text.push_str(&part),
            DecodedStringPart::Expression(_) => return None,
        }
    }
    Some(text.into_boxed_str())
}

/// Decodes one parser-validated ordinary string expression into text and expression parts.
///
/// Multiline indentation is normalized over the complete authored stream before it is split back
/// into parts. An interpolation boundary therefore cannot make two consumers disagree about text
/// that appears before and after it.
#[must_use]
pub fn decode_string_expression(
    source: &SourceFile,
    tree: &SyntaxTree,
    node: NodeId,
) -> Option<Box<[DecodedStringPart]>> {
    let delimiter = tree
        .children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => match token.kind() {
                TokenKind::StringStart(delimiter) => Some(delimiter),
                _ => None,
            },
            SyntaxElement::Node(_) | SyntaxElement::Missing(_) => None,
        })?;
    let mut authored = Vec::new();
    for element in tree.children(node) {
        let SyntaxElement::Node(part) = element else {
            continue;
        };
        if tree.node(*part)?.kind() != NodeKind::StringPart {
            return None;
        }
        let mut found = false;
        for element in tree.children(*part) {
            match element {
                SyntaxElement::Token(token) if token.kind() == TokenKind::StringText => {
                    if found {
                        return None;
                    }
                    authored.extend(
                        source
                            .text_at(token.range())?
                            .bytes()
                            .map(AuthoredStringUnit::Byte),
                    );
                    found = true;
                }
                SyntaxElement::Node(expression)
                    if tree.node(*expression)?.kind() == NodeKind::Expression =>
                {
                    if found {
                        return None;
                    }
                    authored.push(AuthoredStringUnit::Expression(*expression));
                    found = true;
                }
                SyntaxElement::Node(_) => return None,
                SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {}
            }
        }
        if !found {
            return None;
        }
    }
    if delimiter == StringDelimiter::MultiLine {
        authored = normalize_multiline_units(authored)?;
    }
    decode_string_units(authored)
}

/// Decodes the authored content of one string-text segment after any delimiter-owned multiline
/// normalization has been applied.
#[must_use]
pub fn decode_string_text(text: &str) -> Option<Box<str>> {
    let bytes = text.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'\\' {
            result.push(bytes[cursor]);
            cursor += 1;
            continue;
        }
        let (decoded, width) = decode_escape(bytes, cursor, bytes.len()).ok()?;
        result.push(decoded);
        cursor += width;
    }
    String::from_utf8(result).ok().map(String::into_boxed_str)
}

fn decode_authored_string(delimiter: StringDelimiter, text: &str) -> Option<Box<str>> {
    let authored = match delimiter {
        StringDelimiter::SingleLine => text.to_owned(),
        StringDelimiter::MultiLine => normalize_multiline(text)?,
    };
    decode_string_text(&authored)
}

fn normalize_multiline_units(
    mut authored: Vec<AuthoredStringUnit>,
) -> Option<Vec<AuthoredStringUnit>> {
    if authored.first() != Some(&AuthoredStringUnit::Byte(b'\n')) {
        return None;
    }
    authored.remove(0);
    let final_newline = authored
        .iter()
        .rposition(|unit| *unit == AuthoredStringUnit::Byte(b'\n'))?;
    let indentation = authored[final_newline + 1..]
        .iter()
        .map(|unit| match unit {
            AuthoredStringUnit::Byte(byte @ (b' ' | b'\t')) => Some(*byte),
            AuthoredStringUnit::Byte(_) | AuthoredStringUnit::Expression(_) => None,
        })
        .collect::<Option<Vec<_>>>()?;
    authored.truncate(final_newline);

    let mut normalized = Vec::with_capacity(authored.len());
    let mut cursor = 0;
    let mut line_start = true;
    while cursor < authored.len() {
        if line_start {
            let line_end = authored[cursor..]
                .iter()
                .position(|unit| *unit == AuthoredStringUnit::Byte(b'\n'))
                .map_or(authored.len(), |offset| cursor + offset);
            if line_end != cursor {
                for byte in &indentation {
                    if authored.get(cursor) != Some(&AuthoredStringUnit::Byte(*byte)) {
                        return None;
                    }
                    cursor += 1;
                }
            }
            if cursor == authored.len() {
                break;
            }
        }
        let unit = authored[cursor];
        cursor += 1;
        line_start = unit == AuthoredStringUnit::Byte(b'\n');
        normalized.push(unit);
    }
    Some(normalized)
}

fn decode_string_units(authored: Vec<AuthoredStringUnit>) -> Option<Box<[DecodedStringPart]>> {
    let mut parts = Vec::new();
    let mut text = Vec::new();
    for unit in authored {
        match unit {
            AuthoredStringUnit::Byte(byte) => text.push(byte),
            AuthoredStringUnit::Expression(expression) => {
                push_decoded_text(&mut parts, &mut text)?;
                parts.push(DecodedStringPart::Expression(expression));
            }
        }
    }
    push_decoded_text(&mut parts, &mut text)?;
    Some(parts.into_boxed_slice())
}

fn push_decoded_text(parts: &mut Vec<DecodedStringPart>, text: &mut Vec<u8>) -> Option<()> {
    if text.is_empty() {
        return Some(());
    }
    let authored = std::str::from_utf8(text).ok()?;
    parts.push(DecodedStringPart::Text(decode_string_text(authored)?));
    text.clear();
    Some(())
}

fn normalize_multiline(text: &str) -> Option<String> {
    let content = text.strip_prefix('\n')?;
    let final_newline = content.rfind('\n')?;
    let (lines, indentation) = content.split_at(final_newline);
    let indentation = indentation.strip_prefix('\n')?;
    if !indentation.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
        return None;
    }
    let mut normalized = String::with_capacity(lines.len());
    for (index, line) in lines.split('\n').enumerate() {
        if index != 0 {
            normalized.push('\n');
        }
        if line.is_empty() {
            continue;
        }
        normalized.push_str(line.strip_prefix(indentation)?);
    }
    Some(normalized)
}

#[cfg(test)]
mod decode_tests {
    use nocter_source::{SourceMap, SourceName};

    use super::{
        DecodedStringPart, decode_plain_string_expression, decode_string_expression,
        decode_string_literal,
    };
    use crate::{NodeId, NodeKind, ParseGoal, SyntaxElement, parse};

    #[test]
    fn decodes_single_and_multiline_string_syntax_once() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("strings.nct"),
                b"#name: \"line\\nvalue\"\n#version: \"\"\"\n  first\n  second\n  \"\"\"\n",
            )
            .unwrap();
        let file = sources.get(source).unwrap();
        let tree = parse(file, ParseGoal::PackageFile);
        assert!(!tree.has_errors());
        let mut literals = Vec::new();
        let mut pending = vec![tree.root_id()];
        while let Some(node) = pending.pop() {
            if tree.node(node).unwrap().kind() == NodeKind::StringLiteral {
                literals.push(decode_string_literal(file, &tree, node).unwrap());
            }
            for child in tree.children(node).iter().rev() {
                if let SyntaxElement::Node(child) = child {
                    pending.push(*child);
                }
            }
        }
        assert_eq!(
            literals.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
            ["line\nvalue", "first\nsecond"]
        );
    }

    #[test]
    fn decodes_plain_expression_but_not_interpolation() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("strings.nct"),
                b"func text(): &str { \"line\\nvalue\" }\nfunc rendered(value: i32): &str { \"${value}\" }\n",
            )
            .unwrap();
        let file = sources.get(source).unwrap();
        let tree = parse(file, ParseGoal::SourceFile);
        assert!(!tree.has_errors());
        let mut expressions = Vec::new();
        let mut pending = vec![tree.root_id()];
        while let Some(node) = pending.pop() {
            if tree.node(node).unwrap().kind() == NodeKind::StringExpression {
                expressions.push(decode_plain_string_expression(file, &tree, node));
            }
            for child in tree.children(node).iter().rev() {
                if let SyntaxElement::Node(child) = child {
                    pending.push(*child);
                }
            }
        }
        assert_eq!(expressions[0].as_deref(), Some("line\nvalue"));
        assert_eq!(expressions[1], None);
    }

    #[test]
    fn interpolation_parts_share_multiline_indentation_normalization() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("strings.nct"),
                b"func rendered(first: i32, second: i32): void {\n    let text = \"\"\"\n        before ${first}\n        ${second} after\\n\n        \"\"\"\n    return\n}\n",
            )
            .unwrap();
        let file = sources.get(source).unwrap();
        let tree = parse(file, ParseGoal::SourceFile);
        assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());
        let expression = find_node(&tree, NodeKind::StringExpression);
        let parts = decode_string_expression(file, &tree, expression).unwrap();

        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0], DecodedStringPart::Text("before ".into()));
        assert!(matches!(parts[1], DecodedStringPart::Expression(_)));
        assert_eq!(parts[2], DecodedStringPart::Text("\n".into()));
        assert!(matches!(parts[3], DecodedStringPart::Expression(_)));
        assert_eq!(parts[4], DecodedStringPart::Text(" after\n".into()));
    }

    fn find_node(tree: &crate::SyntaxTree, kind: NodeKind) -> NodeId {
        let mut pending = vec![tree.root_id()];
        while let Some(node) = pending.pop() {
            if tree.node(node).is_some_and(|node| node.kind() == kind) {
                return node;
            }
            for child in tree.children(node).iter().rev() {
                if let SyntaxElement::Node(child) = child {
                    pending.push(*child);
                }
            }
        }
        panic!("missing {kind:?}")
    }
}

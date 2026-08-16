use nocter_source::SourceFile;
use nocter_syntax::{NodeId, StringDelimiter, SyntaxElement, SyntaxTree, TokenKind};

pub(crate) fn decode_string_literal(
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
    let authored = match delimiter? {
        StringDelimiter::SingleLine => text.to_owned(),
        StringDelimiter::MultiLine => normalize_multiline(text)?,
    };
    decode_escapes(authored.as_bytes())
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

fn decode_escapes(bytes: &[u8]) -> Option<Box<str>> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'\\' {
            result.push(bytes[cursor]);
            cursor += 1;
            continue;
        }
        let escaped = *bytes.get(cursor + 1)?;
        match escaped {
            b'n' => result.push(b'\n'),
            b'r' => result.push(b'\r'),
            b't' => result.push(b'\t'),
            b'0' => result.push(b'\0'),
            b'\\' => result.push(b'\\'),
            b'"' => result.push(b'"'),
            b'\'' => result.push(b'\''),
            b'$' => result.push(b'$'),
            b'x' => {
                let high = hex(*bytes.get(cursor + 2)?)?;
                let low = hex(*bytes.get(cursor + 3)?)?;
                result.push((high << 4) | low);
                cursor += 2;
            }
            _ => return None,
        }
        cursor += 2;
    }
    String::from_utf8(result).ok().map(String::into_boxed_str)
}

const fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{NodeKind, ParseGoal, SyntaxElement, parse};

    use super::decode_string_literal;

    #[test]
    fn decodes_single_and_multiline_declaration_data() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("nocter.nct"),
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
}

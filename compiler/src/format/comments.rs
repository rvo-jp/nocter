use crate::source::{ByteSpan, SourceId};

pub(super) fn first_comment_span(source: SourceId, text: &str) -> Option<ByteSpan> {
    let bytes = text.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = skip_quoted(bytes, index, b'"'),
            b'b' if bytes.get(index + 1) == Some(&b'\'') => {
                index = skip_quoted(bytes, index + 1, b'\'')
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let start = index;
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                return Some(ByteSpan::new(source, start, index));
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let start = index;
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                let end = if index + 1 < bytes.len() {
                    index + 2
                } else {
                    bytes.len()
                };
                return Some(ByteSpan::new(source, start, end));
            }
            _ => index += 1,
        }
    }

    None
}

fn skip_quoted(bytes: &[u8], start_quote: usize, quote: u8) -> usize {
    let mut index = start_quote + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }

        if bytes[index] == quote {
            return index + 1;
        }

        index += 1;
    }

    bytes.len()
}

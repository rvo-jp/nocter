use crate::source::{ByteSpan, SourceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
    DocLine,
    DocBlock,
    FileDocLine,
    FileDocBlock,
}

impl CommentKind {
    pub fn is_doc(self) -> bool {
        matches!(
            self,
            CommentKind::DocLine
                | CommentKind::DocBlock
                | CommentKind::FileDocLine
                | CommentKind::FileDocBlock
        )
    }

    pub fn is_file_doc(self) -> bool {
        matches!(self, CommentKind::FileDocLine | CommentKind::FileDocBlock)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comment {
    pub span: ByteSpan,
    pub kind: CommentKind,
}

pub fn collect_comments(source: SourceId, text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut comments = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = skip_quoted(bytes, index, b'"'),
            b'b' if bytes.get(index + 1) == Some(&b'\'') => {
                index = skip_quoted(bytes, index + 1, b'\'')
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let start = index;
                let kind = line_comment_kind(bytes, index);
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                comments.push(Comment {
                    span: ByteSpan::new(source, start, index),
                    kind,
                });
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let start = index;
                let kind = block_comment_kind(bytes, index);
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
                comments.push(Comment {
                    span: ByteSpan::new(source, start, end),
                    kind,
                });
                index = end;
            }
            _ => index += 1,
        }
    }

    comments
}

pub fn first_comment_span(source: SourceId, text: &str) -> Option<ByteSpan> {
    collect_comments(source, text)
        .first()
        .map(|comment| comment.span)
}

fn line_comment_kind(bytes: &[u8], start: usize) -> CommentKind {
    match (bytes.get(start + 2), bytes.get(start + 3)) {
        (Some(b'!'), _) => CommentKind::FileDocLine,
        (Some(b'/'), Some(b'/')) => CommentKind::Line,
        (Some(b'/'), _) => CommentKind::DocLine,
        _ => CommentKind::Line,
    }
}

fn block_comment_kind(bytes: &[u8], start: usize) -> CommentKind {
    match (bytes.get(start + 2), bytes.get(start + 3)) {
        (Some(b'!'), _) => CommentKind::FileDocBlock,
        (Some(b'*'), Some(b'*' | b'/')) => CommentKind::Block,
        (Some(b'*'), _) => CommentKind::DocBlock,
        _ => CommentKind::Block,
    }
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

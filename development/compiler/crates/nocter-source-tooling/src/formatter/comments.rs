use nocter_source::{SourceFile, TextRange};
use nocter_syntax::{Comment, SyntaxToken, TokenKind};

#[derive(Clone, Copy)]
pub(super) enum FormatElement {
    Token(SyntaxToken),
    Comment(CommentLayout),
}

#[derive(Clone, Copy)]
pub(super) struct CommentLayout {
    comment: Comment,
    preceded_on_line: bool,
    followed_on_line: bool,
    next_token: Option<SyntaxToken>,
}

impl CommentLayout {
    pub(super) const fn comment(self) -> Comment {
        self.comment
    }

    pub(super) const fn preceded_on_line(self) -> bool {
        self.preceded_on_line
    }

    pub(super) const fn followed_on_line(self) -> bool {
        self.followed_on_line
    }

    pub(super) const fn next_token(self) -> Option<SyntaxToken> {
        self.next_token
    }
}

#[derive(Clone, Copy)]
enum RawElement {
    Token(SyntaxToken),
    Comment(Comment),
}

impl RawElement {
    const fn range(self) -> TextRange {
        match self {
            Self::Token(token) => token.range(),
            Self::Comment(comment) => comment.span().range(),
        }
    }

    const fn significant_range(self) -> Option<TextRange> {
        match self {
            Self::Token(token) if matches!(token.kind(), TokenKind::Newline | TokenKind::Eof) => {
                None
            }
            _ => Some(self.range()),
        }
    }
}

pub(super) fn ordered(
    source: &SourceFile,
    tokens: &[SyntaxToken],
    comments: &[Comment],
) -> Vec<FormatElement> {
    let mut raw = Vec::with_capacity(tokens.len() + comments.len());
    let mut token_index = 0;
    let mut comment_index = 0;
    while token_index < tokens.len() || comment_index < comments.len() {
        let next_token = tokens.get(token_index).copied();
        let next_comment = comments.get(comment_index).copied();
        if next_comment.is_some_and(|comment| {
            next_token.is_none_or(|token| comment.span().range().start() <= token.range().start())
        }) {
            let comment = next_comment.expect("the comment branch requires one comment");
            let comment_end = comment.span().range().end();
            raw.push(RawElement::Comment(comment));
            comment_index += 1;
            while tokens.get(token_index).is_some_and(|token| {
                token.range().start() < comment_end && token.kind() == TokenKind::Newline
            }) {
                token_index += 1;
            }
        } else if let Some(token) = next_token {
            raw.push(RawElement::Token(token));
            token_index += 1;
        }
    }

    raw.iter()
        .copied()
        .enumerate()
        .map(|(index, element)| match element {
            RawElement::Token(token) => FormatElement::Token(token),
            RawElement::Comment(comment) => {
                let previous = raw[..index]
                    .iter()
                    .rev()
                    .find_map(|element| element.significant_range());
                let next = raw[index + 1..]
                    .iter()
                    .find_map(|element| element.significant_range());
                let next_token = raw[index + 1..].iter().find_map(|element| match element {
                    RawElement::Token(token)
                        if !matches!(token.kind(), TokenKind::Newline | TokenKind::Eof) =>
                    {
                        Some(*token)
                    }
                    RawElement::Token(_) | RawElement::Comment(_) => None,
                });
                CommentLayout {
                    comment,
                    preceded_on_line: previous.is_some_and(|range| {
                        same_line(source, range.end(), comment.span().range().start())
                    }),
                    followed_on_line: next.is_some_and(|range| {
                        same_line(source, comment.span().range().end(), range.start())
                    }),
                    next_token,
                }
                .into()
            }
        })
        .collect()
}

impl From<CommentLayout> for FormatElement {
    fn from(comment: CommentLayout) -> Self {
        Self::Comment(comment)
    }
}

fn same_line(
    source: &SourceFile,
    left: nocter_source::ByteOffset,
    right: nocter_source::ByteOffset,
) -> bool {
    let Some(left) = source.lines().line_column(left) else {
        return false;
    };
    let Some(right) = source.lines().line_column(right) else {
        return false;
    };
    left.line() == right.line()
}

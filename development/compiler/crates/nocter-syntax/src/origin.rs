use crate::{NodeId, SyntaxToken};

/// One exact authored syntax identity without a display span or semantic meaning.
///
/// Syntax-producing boundaries use this value when a later phase must refer to the same node or
/// token. Source projection remains a separate concern owned by `nocter-source-index`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyntaxOrigin {
    Node(NodeId),
    Token(SyntaxToken),
}

impl SyntaxOrigin {
    /// Returns a stable ordering key within the origin's source.
    #[must_use]
    pub const fn sort_key(self) -> (u8, usize) {
        match self {
            Self::Node(node) => (0, node.index()),
            Self::Token(token) => (1, token.lexical().index()),
        }
    }
}

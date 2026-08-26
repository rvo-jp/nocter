use std::fmt;

use nocter_source::{SourceId, Span};
use nocter_syntax::{NodeId, SyntaxOrigin, SyntaxToken, SyntaxTree, TokenId};

/// One exact syntax node and normalized source span.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceOrigin {
    source: SourceId,
    syntax: SyntaxOrigin,
    span: Span,
}

impl SourceOrigin {
    /// Projects a syntax node without retaining the syntax tree itself.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownNodeId`] when `node` is not part of `tree`.
    pub fn from_node(tree: &SyntaxTree, node: NodeId) -> Result<Self, UnknownNodeId> {
        let syntax = tree.node(node).ok_or(UnknownNodeId(node))?;
        let source = tree.source();
        Ok(Self {
            source,
            syntax: SyntaxOrigin::Node(node),
            span: Span::new(source, syntax.range()),
        })
    }

    /// Projects one exact syntax-token view without retaining the syntax tree.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownTokenId`] when the token's lexical identity is not part of `tree` or the
    /// syntax view does not lie within that lexical token's normalized range.
    pub fn from_token(tree: &SyntaxTree, token: SyntaxToken) -> Result<Self, UnknownTokenId> {
        if token.source() != tree.source() {
            return Err(UnknownTokenId(token.lexical()));
        }
        let lexical = tree
            .token(token.lexical())
            .ok_or(UnknownTokenId(token.lexical()))?;
        let lexical_range = lexical.span().range();
        if token.range().start() < lexical_range.start()
            || token.range().end() > lexical_range.end()
        {
            return Err(UnknownTokenId(token.lexical()));
        }
        let source = tree.source();
        Ok(Self {
            source,
            syntax: SyntaxOrigin::Token(token),
            span: Span::new(source, token.range()),
        })
    }

    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn syntax(self) -> SyntaxOrigin {
        self.syntax
    }

    #[must_use]
    pub const fn node(self) -> Option<NodeId> {
        match self.syntax {
            SyntaxOrigin::Node(node) => Some(node),
            SyntaxOrigin::Token(_) => None,
        }
    }

    #[must_use]
    pub const fn token(self) -> Option<SyntaxToken> {
        match self.syntax {
            SyntaxOrigin::Node(_) => None,
            SyntaxOrigin::Token(token) => Some(token),
        }
    }

    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct UnknownNodeId(NodeId);

impl UnknownNodeId {
    #[must_use]
    pub const fn id(self) -> NodeId {
        self.0
    }
}

impl fmt::Debug for UnknownNodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("UnknownNodeId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for UnknownNodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "syntax node {:?} is not part of this tree",
            self.0
        )
    }
}

impl std::error::Error for UnknownNodeId {}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct UnknownTokenId(TokenId);

impl UnknownTokenId {
    #[must_use]
    pub const fn id(self) -> TokenId {
        self.0
    }
}

impl fmt::Debug for UnknownTokenId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("UnknownTokenId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for UnknownTokenId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "syntax token {:?} is not part of this tree",
            self.0
        )
    }
}

impl std::error::Error for UnknownTokenId {}

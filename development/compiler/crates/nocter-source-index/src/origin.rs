use std::fmt;

use nocter_source::{SourceId, Span};
use nocter_syntax::{NodeId, SyntaxTree};

/// One exact syntax node and normalized source span.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceOrigin {
    source: SourceId,
    node: NodeId,
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
            node,
            span: Span::new(source, syntax.range()),
        })
    }

    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn node(self) -> NodeId {
        self.node
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

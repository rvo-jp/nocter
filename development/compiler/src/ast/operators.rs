//! Source-owned operator declarations.

use super::{CallableDecl, TypeExpr};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorDecl {
    Comparison(ComparisonOperatorDecl),
    Index(IndexOperatorDecl),
    Expansion(ExpansionOperatorDecl),
}

impl OperatorDecl {
    pub fn callable(&self) -> &CallableDecl {
        match self {
            Self::Comparison(operator) => operator.callable(),
            Self::Index(operator) => operator.callable(),
            Self::Expansion(operator) => operator.callable(),
        }
    }

    pub fn callable_mut(&mut self) -> &mut CallableDecl {
        match self {
            Self::Comparison(operator) => operator.callable_mut(),
            Self::Index(operator) => operator.callable_mut(),
            Self::Expansion(operator) => operator.callable_mut(),
        }
    }

    pub fn anchor_span(&self) -> ByteSpan {
        match self {
            Self::Comparison(operator) => operator.operator_span,
            Self::Index(operator) => operator.open_bracket_span,
            Self::Expansion(operator) => operator.operator_span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionOperatorDecl {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    callable: CallableDecl,
}

impl ExpansionOperatorDecl {
    pub fn new(span: ByteSpan, operator_span: ByteSpan, callable: CallableDecl) -> Self {
        Self {
            span,
            operator_span,
            callable,
        }
    }

    pub fn callable(&self) -> &CallableDecl {
        &self.callable
    }

    pub fn callable_mut(&mut self) -> &mut CallableDecl {
        &mut self.callable
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonOperatorDecl {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    pub kind: ComparisonOperatorKind,
    callable: CallableDecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperatorKind {
    Equality,
    StrictOrder,
}

impl ComparisonOperatorKind {
    pub fn source_token(self) -> &'static str {
        match self {
            Self::Equality => "==",
            Self::StrictOrder => "<",
        }
    }
}

impl ComparisonOperatorDecl {
    /// Adapts the fixed operator shape to the ordinary static method body pipeline. The synthetic
    /// name is an internal identity and is never presented as source syntax.
    pub fn new(
        span: ByteSpan,
        operator_span: ByteSpan,
        kind: ComparisonOperatorKind,
        callable: CallableDecl,
    ) -> Self {
        Self {
            span,
            operator_span,
            kind,
            callable,
        }
    }

    pub fn callable(&self) -> &CallableDecl {
        &self.callable
    }

    pub fn callable_mut(&mut self) -> &mut CallableDecl {
        &mut self.callable
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexOperatorDecl {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    pub open_bracket_span: ByteSpan,
    pub close_bracket_span: ByteSpan,
    callable: CallableDecl,
}

impl IndexOperatorDecl {
    pub fn new(
        span: ByteSpan,
        operator_span: ByteSpan,
        open_bracket_span: ByteSpan,
        close_bracket_span: ByteSpan,
        callable: CallableDecl,
    ) -> Self {
        Self {
            span,
            operator_span,
            open_bracket_span,
            close_bracket_span,
            callable,
        }
    }

    pub fn callable(&self) -> &CallableDecl {
        &self.callable
    }

    pub fn callable_mut(&mut self) -> &mut CallableDecl {
        &mut self.callable
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorRequirementPredicate {
    pub span: ByteSpan,
    pub open_paren_span: ByteSpan,
    pub close_paren_span: ByteSpan,
    pub colon_span: ByteSpan,
    pub shape: OperatorRequirementShape,
    pub result: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorRequirementShape {
    Comparison {
        kind: ComparisonOperatorKind,
        operator_span: ByteSpan,
        left: TypeExpr,
        right: TypeExpr,
    },
    Index {
        open_bracket_span: ByteSpan,
        close_bracket_span: ByteSpan,
        target: TypeExpr,
        index: TypeExpr,
    },
    Expansion {
        operator_span: ByteSpan,
        source: TypeExpr,
    },
}

impl OperatorRequirementPredicate {
    pub fn equality(&self) -> Option<(&TypeExpr, ByteSpan, &TypeExpr)> {
        match &self.shape {
            OperatorRequirementShape::Comparison {
                kind: ComparisonOperatorKind::Equality,
                operator_span,
                left,
                right,
            } => Some((left, *operator_span, right)),
            OperatorRequirementShape::Comparison { .. }
            | OperatorRequirementShape::Index { .. }
            | OperatorRequirementShape::Expansion { .. } => None,
        }
    }

    pub fn strict_order(&self) -> Option<(&TypeExpr, ByteSpan, &TypeExpr)> {
        match &self.shape {
            OperatorRequirementShape::Comparison {
                kind: ComparisonOperatorKind::StrictOrder,
                operator_span,
                left,
                right,
            } => Some((left, *operator_span, right)),
            OperatorRequirementShape::Comparison { .. }
            | OperatorRequirementShape::Index { .. }
            | OperatorRequirementShape::Expansion { .. } => None,
        }
    }
}

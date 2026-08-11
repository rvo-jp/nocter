//! Source-owned equality operator declarations.

use super::{MethodDecl, TypeExpr};
use crate::source::ByteSpan;

pub(crate) const EQUALITY_OPERATOR_METHOD_NAME: &str = "__nocter$operator$equal";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EqualityOperatorDecl {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    callable: MethodDecl,
}

impl EqualityOperatorDecl {
    /// Adapts the fixed operator shape to the ordinary static method body pipeline. The synthetic
    /// name is an internal identity and is never presented as source syntax.
    pub fn new(span: ByteSpan, operator_span: ByteSpan, callable: MethodDecl) -> Self {
        Self {
            span,
            operator_span,
            callable,
        }
    }

    pub fn callable_method(&self) -> &MethodDecl {
        &self.callable
    }

    pub fn callable_method_mut(&mut self) -> &mut MethodDecl {
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
    Equality {
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
}

impl OperatorRequirementPredicate {
    pub fn equality(&self) -> Option<(&TypeExpr, ByteSpan, &TypeExpr)> {
        match &self.shape {
            OperatorRequirementShape::Equality {
                operator_span,
                left,
                right,
            } => Some((left, *operator_span, right)),
            OperatorRequirementShape::Index { .. } => None,
        }
    }
}

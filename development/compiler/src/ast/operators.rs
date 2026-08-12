//! Source-owned operator declarations.

use super::{MethodDecl, TypeExpr};
use crate::source::ByteSpan;

pub(crate) const EQUALITY_OPERATOR_METHOD_NAME: &str = "__nocter$operator$equal";
pub(crate) const READONLY_INDEX_OPERATOR_METHOD_NAME: &str = "__nocter$operator$index";
pub(crate) const READWRITE_INDEX_OPERATOR_METHOD_NAME: &str = "__nocter$operator$index_readwrite";
pub(crate) const READONLY_EXPANSION_OPERATOR_METHOD_NAME: &str =
    "__nocter$operator$expand_readonly";
pub(crate) const READWRITE_EXPANSION_OPERATOR_METHOD_NAME: &str =
    "__nocter$operator$expand_readwrite";
pub(crate) const OWNED_EXPANSION_OPERATOR_METHOD_NAME: &str = "__nocter$operator$expand_owned";

pub(crate) fn is_operator_method_name(name: &str) -> bool {
    matches!(
        name,
        EQUALITY_OPERATOR_METHOD_NAME
            | READONLY_INDEX_OPERATOR_METHOD_NAME
            | READWRITE_INDEX_OPERATOR_METHOD_NAME
            | READONLY_EXPANSION_OPERATOR_METHOD_NAME
            | READWRITE_EXPANSION_OPERATOR_METHOD_NAME
            | OWNED_EXPANSION_OPERATOR_METHOD_NAME
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorDecl {
    Equality(EqualityOperatorDecl),
    Index(IndexOperatorDecl),
    Expansion(ExpansionOperatorDecl),
}

impl OperatorDecl {
    pub fn callable_method(&self) -> &MethodDecl {
        match self {
            Self::Equality(operator) => operator.callable_method(),
            Self::Index(operator) => operator.callable_method(),
            Self::Expansion(operator) => operator.callable_method(),
        }
    }

    pub fn callable_method_mut(&mut self) -> &mut MethodDecl {
        match self {
            Self::Equality(operator) => operator.callable_method_mut(),
            Self::Index(operator) => operator.callable_method_mut(),
            Self::Expansion(operator) => operator.callable_method_mut(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionOperatorDecl {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    callable: MethodDecl,
}

impl ExpansionOperatorDecl {
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
pub struct IndexOperatorDecl {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    pub open_bracket_span: ByteSpan,
    pub close_bracket_span: ByteSpan,
    callable: MethodDecl,
}

impl IndexOperatorDecl {
    pub fn new(
        span: ByteSpan,
        operator_span: ByteSpan,
        open_bracket_span: ByteSpan,
        close_bracket_span: ByteSpan,
        callable: MethodDecl,
    ) -> Self {
        Self {
            span,
            operator_span,
            open_bracket_span,
            close_bracket_span,
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
    Expansion {
        operator_span: ByteSpan,
        source: TypeExpr,
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
            OperatorRequirementShape::Index { .. } | OperatorRequirementShape::Expansion { .. } => {
                None
            }
        }
    }
}

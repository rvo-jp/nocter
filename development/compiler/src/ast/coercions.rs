//! Instance-owned borrowed-view coercion declarations and structural requirements.

use super::{CallableDecl, TypeExpr};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionEntry {
    pub span: ByteSpan,
    pub keyword_span: ByteSpan,
    pub as_span: ByteSpan,
    callable: CallableDecl,
}

impl CoercionEntry {
    pub fn new(
        span: ByteSpan,
        keyword_span: ByteSpan,
        as_span: ByteSpan,
        callable: CallableDecl,
    ) -> Self {
        Self {
            span,
            keyword_span,
            as_span,
            callable,
        }
    }

    pub fn callable(&self) -> &CallableDecl {
        &self.callable
    }

    pub fn callable_mut(&mut self) -> &mut CallableDecl {
        &mut self.callable
    }

    pub fn target(&self) -> &TypeExpr {
        &self.callable.return_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionRequirementPredicate {
    pub span: ByteSpan,
    pub source: TypeExpr,
    pub as_span: ByteSpan,
    pub target: TypeExpr,
}

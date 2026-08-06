//! Native source-level test declarations.

use super::{Block, FallibleType, TypeExpr, TypeReference};
use crate::source::ByteSpan;

/// A compiler-owned test entry. It shares callable-body machinery with functions without entering
/// the source callable namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestDecl {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub body: Block,
}

impl TestDecl {
    /// Returns the fixed `void!` result contract used below the declaration boundary.
    pub(crate) fn return_type(&self) -> TypeExpr {
        let contract_span = self.name_span;
        TypeExpr::Fallible(FallibleType {
            span: contract_span,
            success: Box::new(TypeExpr::Reference(TypeReference {
                span: contract_span,
                name: "void".to_string(),
            })),
            error: Box::new(TypeExpr::Reference(TypeReference {
                span: contract_span,
                name: "error".to_string(),
            })),
        })
    }
}

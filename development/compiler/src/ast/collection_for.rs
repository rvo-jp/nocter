use super::{Block, Expr};
use crate::source::ByteSpan;

/// A protocol-driven loop over an iterator or explicitly borrowed/moved collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionForStmt {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub source: Expr,
    pub body: Block,
}

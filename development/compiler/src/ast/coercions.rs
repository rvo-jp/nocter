use super::{Block, MethodReceiver, ResultProvenanceClause, TypeExpr, Visibility};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoerceDecl {
    pub span: ByteSpan,
    pub keyword_span: ByteSpan,
    pub target: TypeExpr,
    pub entries: Vec<CoercionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionEntry {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub receiver: MethodReceiver,
    pub as_span: ByteSpan,
    pub target: TypeExpr,
    pub result_provenance: Option<ResultProvenanceClause>,
    pub body: Block,
}

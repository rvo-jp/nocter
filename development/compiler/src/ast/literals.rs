use super::{
    Block, Expr, LiteralExpr, ParameterList, ResultProvenanceClause, TypeExpr, Visibility,
    WhereClause,
};
use crate::source::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiteralShape {
    Sequence,
    String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub keyword_span: ByteSpan,
    pub target: TypeExpr,
    pub shape: LiteralShape,
    pub shape_span: ByteSpan,
    pub parameters: ParameterList,
    pub capture: Option<LiteralCapture>,
    pub return_type: TypeExpr,
    pub result_provenance: Option<ResultProvenanceClause>,
    pub requirements: Option<WhereClause>,
    pub body: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralCapture {
    pub span: ByteSpan,
    pub ellipsis_span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub element_type: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralPackForStmt {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub pack_name: String,
    pub pack_span: ByteSpan,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedSequenceLiteralExpr {
    pub span: ByteSpan,
    pub target: TypeExpr,
    pub elements_span: ByteSpan,
    pub elements: Vec<Expr>,
    pub using: Option<LiteralContextOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStringLiteralExpr {
    pub span: ByteSpan,
    pub target: TypeExpr,
    pub text: LiteralExpr,
    pub using: Option<LiteralContextOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralContextOverride {
    pub span: ByteSpan,
    pub using_span: ByteSpan,
    pub allocator: Box<Expr>,
}

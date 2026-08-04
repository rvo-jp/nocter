//! Source-level closure syntax and explicit capture declarations.

use super::{Block, TypeExpr};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureExpr {
    pub span: ByteSpan,
    pub parameters_span: ByteSpan,
    pub captures: Vec<ClosureCapture>,
    pub capture_separator_span: Option<ByteSpan>,
    pub parameters: Vec<ClosureParameter>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureCapture {
    pub span: ByteSpan,
    pub mode: ClosureCaptureMode,
    pub operator_span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureCaptureMode {
    ReadonlyBorrow,
    ReadwriteBorrow,
    Move,
}

impl ClosureCaptureMode {
    pub fn source_prefix(self) -> &'static str {
        match self {
            Self::ReadonlyBorrow => "&",
            Self::ReadwriteBorrow => "&+",
            Self::Move => "move ",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureParameter {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub ty: Option<TypeExpr>,
}

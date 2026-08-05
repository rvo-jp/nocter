//! Source-level closure syntax and explicit capture declarations.

use super::{Block, CallableCapability, TypeExpr};
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

/// Compiler-owned concrete type materialized after contextual closure
/// inference. This variant never appears in parsed source type syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureTypeExpr {
    pub span: ByteSpan,
    pub captures: Vec<ClosureCaptureType>,
    pub parameters: Vec<TypeExpr>,
    pub return_type: Box<TypeExpr>,
    pub capability: CallableCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureCaptureType {
    pub name: String,
    pub mode: ClosureCaptureMode,
    pub ty: TypeExpr,
}

impl ClosureTypeExpr {
    pub fn identity_name(&self) -> String {
        format!("<closure@{}:{}>", self.span.source.raw(), self.span.start)
    }
}

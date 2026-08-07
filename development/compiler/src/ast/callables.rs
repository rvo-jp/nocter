//! Built-in structural callable contracts.

use super::{ResultAllocationModifier, ResultProvenanceClause, TypeExpr};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableTypeExpr {
    pub span: ByteSpan,
    pub func_span: ByteSpan,
    pub result_allocation: Option<ResultAllocationModifier>,
    pub capability: CallableCapability,
    pub parameters_span: ByteSpan,
    pub parameters: Vec<CallableTypeParameter>,
    pub return_type: Box<TypeExpr>,
    pub result_provenance: Option<ResultProvenanceClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableTypeParameter {
    pub span: ByteSpan,
    pub name: Option<String>,
    pub name_span: Option<ByteSpan>,
    pub ty: TypeExpr,
}

/// The least receiver access required to invoke a callable value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallableCapability {
    Readonly,
    Readwrite,
    Consuming,
}

impl CallableCapability {
    pub fn source_prefix(self) -> &'static str {
        match self {
            Self::Readonly => "&",
            Self::Readwrite => "&+",
            Self::Consuming => "",
        }
    }
}

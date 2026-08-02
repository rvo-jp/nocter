use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultProvenanceClause {
    pub span: ByteSpan,
    pub origins: Vec<ResultProvenanceOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultProvenanceOrigin {
    pub span: ByteSpan,
    pub kind: ResultProvenanceOriginKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultProvenanceOriginKind {
    Receiver,
    Parameter(String),
    Static,
    CurrentAllocationContext,
}

impl ResultProvenanceOriginKind {
    pub fn source_label(&self) -> &str {
        match self {
            Self::Receiver => "self",
            Self::Parameter(name) => name,
            Self::Static => "static",
            Self::CurrentAllocationContext => "current",
        }
    }
}

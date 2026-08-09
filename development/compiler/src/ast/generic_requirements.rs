use super::TypeExpr;
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParamList {
    pub span: Option<ByteSpan>,
    pub parameters: Vec<GenericParam>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParam {
    pub span: ByteSpan,
    pub copy_span: Option<ByteSpan>,
    pub name: String,
    pub name_span: ByteSpan,
    pub bounds: Vec<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableRequirementClause {
    pub span: ByteSpan,
    pub keyword_span: ByteSpan,
    pub requirements: Vec<CallableGenericRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableGenericRequirement {
    pub span: ByteSpan,
    pub copy_span: Option<ByteSpan>,
    pub name: String,
    pub name_span: ByteSpan,
    pub bounds: Vec<TypeExpr>,
}

impl GenericParamList {
    pub fn empty() -> Self {
        Self {
            span: None,
            parameters: Vec::new(),
        }
    }
}

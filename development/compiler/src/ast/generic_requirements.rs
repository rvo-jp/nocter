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
pub struct WhereClause {
    pub span: ByteSpan,
    pub keyword_span: ByteSpan,
    pub predicates: Vec<WherePredicate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericRequirementPredicate {
    pub span: ByteSpan,
    pub copy_span: Option<ByteSpan>,
    pub name: String,
    pub name_span: ByteSpan,
    pub bounds: Vec<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WherePredicate {
    Generic(GenericRequirementPredicate),
    Equality(TypeEqualityPredicate),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeEqualityPredicate {
    pub span: ByteSpan,
    pub equals_span: ByteSpan,
    pub left: TypeExpr,
    pub right: TypeExpr,
}

impl WhereClause {
    pub fn generic_requirements(&self) -> impl Iterator<Item = &GenericRequirementPredicate> {
        self.predicates
            .iter()
            .filter_map(|predicate| match predicate {
                WherePredicate::Generic(requirement) => Some(requirement),
                WherePredicate::Equality(_) => None,
            })
    }

    pub fn equalities(&self) -> impl Iterator<Item = &TypeEqualityPredicate> {
        self.predicates
            .iter()
            .filter_map(|predicate| match predicate {
                WherePredicate::Equality(equality) => Some(equality),
                WherePredicate::Generic(_) => None,
            })
    }
}

impl GenericParamList {
    pub fn empty() -> Self {
        Self {
            span: None,
            parameters: Vec::new(),
        }
    }
}

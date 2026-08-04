use super::{FunctionDecl, LiteralDecl, TypeExpr};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructDecl {
    pub span: ByteSpan,
    pub keyword_span: ByteSpan,
    pub target: TypeExpr,
    pub members: Vec<ConstructMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructMember {
    pub span: ByteSpan,
    pub default_span: Option<ByteSpan>,
    pub declaration: ConstructMemberDecl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstructMemberDecl {
    Function(FunctionDecl),
    Literal(LiteralDecl),
}

impl ConstructMember {
    pub fn is_default(&self) -> bool {
        self.default_span.is_some()
    }
}

impl ConstructDecl {
    pub fn functions(&self) -> impl Iterator<Item = (&ConstructMember, &FunctionDecl)> {
        self.members.iter().filter_map(|member| {
            let ConstructMemberDecl::Function(function) = &member.declaration else {
                return None;
            };
            Some((member, function))
        })
    }

    pub fn literals(&self) -> impl Iterator<Item = (&ConstructMember, &LiteralDecl)> {
        self.members.iter().filter_map(|member| {
            let ConstructMemberDecl::Literal(literal) = &member.declaration else {
                return None;
            };
            Some((member, literal))
        })
    }
}

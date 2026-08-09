use super::{
    Block, GenericParamList, ImplDecl, ImplMember, MethodDecl, MethodReceiver, ParameterList,
    ResultProvenanceClause, TypeExpr, Visibility,
};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoerceDecl {
    pub span: ByteSpan,
    pub keyword_span: ByteSpan,
    pub target: TypeExpr,
    pub generics: GenericParamList,
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
    pub body: Option<Block>,
}

impl CoerceDecl {
    /// Adapts unnamed coercion bodies to the ordinary inherent-method pipeline without publishing
    /// a synthetic member or making method lookup aware of coercions.
    pub(crate) fn callable_impl(&self) -> ImplDecl {
        ImplDecl {
            span: self.span,
            generics: self.generics.clone(),
            interface_ty: None,
            target_ty: self.target.clone(),
            requirements: None,
            members: self
                .entries
                .iter()
                .map(|entry| ImplMember::Method(entry.callable_method()))
                .collect(),
        }
    }
}

impl CoercionEntry {
    pub(crate) fn callable_method(&self) -> MethodDecl {
        MethodDecl {
            span: self.span,
            visibility: self.visibility,
            keyword_span: self.as_span,
            receiver: self.receiver.clone(),
            name: format!("__nocter$coerce${}", self.as_span.start),
            name_span: self.as_span,
            generics: GenericParamList::empty(),
            parameters: ParameterList {
                span: self.as_span,
                parameters: Vec::new(),
            },
            return_type: self.target.clone(),
            result_provenance: self.result_provenance.clone(),
            requirements: None,
            body: self.body.clone(),
        }
    }
}

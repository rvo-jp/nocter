use super::*;

impl TypeExpr {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            TypeExpr::Callable(ty) => {
                let mut children = Vec::new();
                children.extend(
                    ty.parameters
                        .iter()
                        .map(|parameter| parameter.ty.to_json(sources)),
                );
                children.push(ty.return_type.to_json(sources));
                if let Some(provenance) = &ty.result_provenance {
                    children.push(provenance.to_json(sources));
                }
                JsonAstNode::with_value(
                    "callable_type",
                    crate::ast::canonical_type_expr(self),
                    json_span(sources, ty.span),
                    children,
                )
            }
            TypeExpr::Closure(ty) => JsonAstNode::with_value(
                "anonymous_closure_type",
                ty.identity_name(),
                json_span(sources, ty.span),
                ty.parameters
                    .iter()
                    .chain(std::iter::once(ty.return_type.as_ref()))
                    .map(|ty| ty.to_json(sources))
                    .collect(),
            ),
            TypeExpr::Reference(ty) => JsonAstNode::with_value(
                "type_reference",
                ty.name.clone(),
                json_span(sources, ty.span),
                Vec::new(),
            ),
            TypeExpr::Generic(ty) => JsonAstNode::with_value(
                "generic_type",
                ty.name.clone(),
                json_span(sources, ty.span),
                ty.arguments
                    .iter()
                    .map(|argument| argument.to_json(sources))
                    .collect(),
            ),
            TypeExpr::Pointer(ty) => JsonAstNode::new(
                "pointer_type",
                json_span(sources, ty.span),
                vec![ty.inner.to_json(sources)],
            ),
            TypeExpr::Borrow(ty) => JsonAstNode::new(
                if ty.is_readwrite {
                    "readwrite_borrow_type"
                } else {
                    "readonly_borrow_type"
                },
                json_span(sources, ty.span),
                vec![ty.inner.to_json(sources)],
            ),
            TypeExpr::View(ty) => JsonAstNode::new(
                if ty.is_readwrite {
                    "readwrite_view_type"
                } else {
                    "readonly_view_type"
                },
                json_span(sources, ty.span),
                vec![ty.element.to_json(sources)],
            ),
            TypeExpr::Array(ty) => JsonAstNode::with_value(
                "array_type",
                ty.length.value.clone(),
                json_span(sources, ty.span),
                vec![ty.element.to_json(sources)],
            ),
            TypeExpr::Optional(ty) => JsonAstNode::new(
                "optional_type",
                json_span(sources, ty.span),
                vec![ty.inner.to_json(sources)],
            ),
            TypeExpr::Fallible(ty) => JsonAstNode::new(
                "fallible_type",
                json_span(sources, ty.span),
                vec![ty.success.to_json(sources)],
            ),
        }
    }
}

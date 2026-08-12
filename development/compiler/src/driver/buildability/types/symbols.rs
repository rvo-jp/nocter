use super::*;

pub(in crate::driver::buildability) fn binding_type_expr_with_substitutions(
    statement: &BindingStmt,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    statement
        .ty
        .clone()
        .or_else(|| typed_hir.binding_type_expr(statement.name_span).cloned())
        .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
}
pub(in crate::driver::buildability) fn local_identifier_type_expr_with_substitutions(
    identifier: &IdentifierExpr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let symbol = resolved.local_symbol_for_identifier(identifier)?;
    typed_hir
        .binding_type_expr(symbol.name_span)
        .cloned()
        .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
}
pub(in crate::driver::buildability) fn resolved_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> &'a ResolveOutput
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let source_resolved = resolver(ty.span().source);
    let Some(name) = type_expr_symbol_name(ty) else {
        return source_resolved.unwrap_or(fallback_resolved);
    };

    if let Some(resolved) = source_resolved
        && type_symbol_by_reference_name(resolved, name).is_some()
    {
        return resolved;
    }
    if type_symbol_by_reference_name(fallback_resolved, name).is_some() {
        return fallback_resolved;
    }

    source_resolved.unwrap_or(fallback_resolved)
}
pub(in crate::driver::buildability) fn type_expr_symbol_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Callable(_)
        | TypeExpr::Closure(_)
        | TypeExpr::Opaque(_)
        | TypeExpr::Projection(_) => None,
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}
pub(in crate::driver::buildability) fn type_symbol_by_reference_name<'a>(
    resolved: &'a ResolveOutput,
    name: &str,
) -> Option<&'a TypeSymbol> {
    resolved.type_symbol_by_reference_name(name).or_else(|| {
        short_qualified_type_name(name)
            .and_then(|short| resolved.type_symbol_by_reference_name(short))
    })
}
pub(in crate::driver::buildability) fn short_qualified_type_name(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_module, short)| short)
}

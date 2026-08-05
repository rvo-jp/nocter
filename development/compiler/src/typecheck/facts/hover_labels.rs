use super::*;

pub(super) fn type_label(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    type_hover_label(
        &type_expr_to_type_with_self_type(ty, resolved, self_type),
        resolved,
    )
}

pub(super) fn type_hover_label(ty: &Type, resolved: &ResolveOutput) -> String {
    if let Type::Closure(closure) = ty {
        let capability = match closure.capability {
            crate::ast::CallableCapability::Readonly => "closure",
            crate::ast::CallableCapability::Readwrite => "closure mut",
            crate::ast::CallableCapability::Consuming => "closure once",
        };
        let parameters = closure
            .parameters
            .iter()
            .map(crate::ast::canonical_type_expr)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{capability} ({parameters}): {}",
            crate::ast::canonical_type_expr(&closure.return_type)
        )
    } else {
        ty.notation_with_name(&|name| display_type_name(name, resolved).to_string())
            .render()
    }
}

pub(super) fn type_owner_hover_label<'a>(
    owner: &'a TypeSymbol,
    resolved: &'a ResolveOutput,
) -> &'a str {
    display_type_name(&owner.canonical_name, resolved)
}

pub(super) fn display_type_name<'a>(
    canonical_name: &'a str,
    resolved: &'a ResolveOutput,
) -> &'a str {
    visible_type_name(canonical_name, resolved).unwrap_or_else(|| short_type_name(canonical_name))
}

pub(super) fn short_type_name(canonical_name: &str) -> &str {
    canonical_name
        .rsplit_once('.')
        .map(|(_, name)| name)
        .unwrap_or(canonical_name)
}

pub(super) fn visible_type_name<'a>(
    canonical_name: &str,
    resolved: &'a ResolveOutput,
) -> Option<&'a str> {
    resolved
        .symbols
        .symbols()
        .filter(|symbol| crate::lexer::is_valid_identifier_name(&symbol.name))
        .filter_map(|symbol| match &symbol.kind {
            SymbolKind::Type(type_symbol)
                if type_symbol.canonical_name == canonical_name
                    && symbol.name != canonical_name =>
            {
                Some(symbol.name.as_str())
            }
            SymbolKind::Function(_)
            | SymbolKind::Primitive(_)
            | SymbolKind::Type(_)
            | SymbolKind::Imported(_) => None,
        })
        .min_by_key(|name| name.len())
}

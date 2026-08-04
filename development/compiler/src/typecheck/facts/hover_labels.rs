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
    match ty {
        Type::Callable(_) => ty.display(),
        Type::Closure(closure) => {
            let capability = match closure.capability {
                crate::ast::CallableCapability::Readonly => "closure",
                crate::ast::CallableCapability::Readwrite => "closure mut",
                crate::ast::CallableCapability::Consuming => "closure once",
            };
            let parameters = closure
                .parameters
                .iter()
                .map(crate::ast::type_expr_display_lossy)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{capability} ({parameters}): {}",
                crate::ast::type_expr_display_lossy(&closure.return_type)
            )
        }
        Type::I32 => "i32".to_string(),
        Type::Primitive(name) => name.clone(),
        Type::StrData => "str".to_string(),
        Type::Str => "&str".to_string(),
        Type::Error => "error".to_string(),
        Type::Void => "void".to_string(),
        Type::Never => "never".to_string(),
        Type::None => "none".to_string(),
        Type::ArrayData { element } => format!("[{}]", type_hover_label(element, resolved)),
        Type::View {
            is_readwrite: true,
            element,
        } => format!("&+[{}]", type_hover_label(element, resolved)),
        Type::View {
            is_readwrite: false,
            element,
        } => format!("&[{}]", type_hover_label(element, resolved)),
        Type::Array { element, length } => {
            format!("[{}; {}]", type_hover_label(element, resolved), length)
        }
        Type::Pointer(inner) => format!("*{}", type_hover_label(inner, resolved)),
        Type::Optional(inner) => format!("{}?", suffix_operand_hover_label(inner, resolved)),
        Type::Fallible { success, .. } => {
            format!("{}!", suffix_operand_hover_label(success, resolved))
        }
        Type::Named(name) => {
            if let Some(inner) = name.strip_prefix("&+") {
                format!(
                    "&+{}",
                    type_hover_label(&simple_type_from_display_name(inner), resolved)
                )
            } else if let Some(inner) = name.strip_prefix('&') {
                format!(
                    "&{}",
                    type_hover_label(&simple_type_from_display_name(inner), resolved)
                )
            } else {
                display_type_name(name, resolved).to_string()
            }
        }
        Type::Generic { name, arguments } => {
            let name = display_type_name(name, resolved);
            let arguments = arguments
                .iter()
                .map(|argument| type_hover_label(argument, resolved))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}<{arguments}>")
        }
        Type::Parameter(name) => name.clone(),
        Type::Unresolved(name) => name.clone(),
        Type::Unknown => "<unknown>".to_string(),
    }
}

fn suffix_operand_hover_label(ty: &Type, resolved: &ResolveOutput) -> String {
    let label = type_hover_label(ty, resolved);
    if matches!(ty, Type::Str | Type::View { .. })
        || matches!(ty, Type::Named(name) if name.starts_with('&'))
    {
        format!("({label})")
    } else {
        label
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

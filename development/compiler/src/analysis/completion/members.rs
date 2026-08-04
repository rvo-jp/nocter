use super::*;

pub(super) fn member_completion_items(
    ast: &AstFile,
    resolved: &ResolveOutput,
    facts: &TypecheckFacts,
    owner_name: &str,
    owner_span: ByteSpan,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    if let Some(symbol) = resolved.type_symbol_by_name(owner_name) {
        return type_member_completion_items(symbol, resolved);
    }

    let Some(owner_ty) = facts.expression_type_expr(owner_span) else {
        return Vec::new();
    };
    let can_readwrite = owner_type_is_readwrite(owner_ty)
        || (!matches!(owner_ty, TypeExpr::Borrow(_))
            && !facts.binding_is_readonly(owner_span).unwrap_or(true));
    let can_move = !matches!(owner_ty, TypeExpr::Borrow(_));
    if let Some(owner) = value_member_owner(resolved, owner_ty) {
        return value_member_completion_items(
            &owner,
            resolved,
            can_readwrite,
            can_move,
            owner_span.source,
        );
    }
    let owners = generic_bound_member_owners(ast, resolved, owner_ty, offset);
    unambiguous_capability_member_items(
        owners,
        resolved,
        can_readwrite,
        can_move,
        owner_span.source,
    )
}

fn type_member_completion_items(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
) -> Vec<CompletionItemInfo> {
    let owner = type_symbol_presentation_label(symbol, resolved);
    let mut items = Vec::new();
    if symbol.kind == TypeSymbolKind::Enum {
        items.extend(enum_variant_completion_items(symbol, resolved));
    }
    items.extend(
        symbol
            .associated_functions
            .iter()
            .filter(|function| function.is_accessible)
            .map(|function| associated_function_completion_item(function, &owner, resolved)),
    );
    items
}

fn value_member_completion_items(
    owner: &ValueMemberOwner<'_>,
    resolved: &ResolveOutput,
    can_readwrite: bool,
    can_move: bool,
    use_source: crate::source::SourceId,
) -> Vec<CompletionItemInfo> {
    let mut items = Vec::new();
    items.extend(
        owner
            .symbol
            .fields
            .iter()
            .filter(|field| field.is_accessible)
            .map(|field| {
                struct_field_completion_item(field, resolved, false, &owner.substitutions)
            }),
    );
    items.extend(
        owner
            .symbol
            .methods
            .iter()
            .filter(|method| {
                method.is_accessible
                    && method_receiver_is_available(method, can_readwrite, can_move)
            })
            .map(|method| method_completion_item(method, resolved, &owner.substitutions)),
    );
    let inherent_names = owner
        .symbol
        .methods
        .iter()
        .map(|method| method.name.as_str())
        .collect::<HashSet<_>>();
    let Some(self_ty) = owner.substitutions.get("Self") else {
        return items;
    };
    let mut defaults_by_name: HashMap<&str, Vec<CompletionItemInfo>> = HashMap::new();
    for candidate in default_method_completion_candidates(self_ty, use_source, resolved) {
        if inherent_names.contains(candidate.method.name.as_str())
            || !method_receiver_is_available(candidate.method, can_readwrite, can_move)
        {
            continue;
        }
        defaults_by_name
            .entry(candidate.method.name.as_str())
            .or_default()
            .push(method_completion_item(
                candidate.method,
                resolved,
                &candidate.substitutions,
            ));
    }
    items.extend(
        defaults_by_name
            .into_values()
            .filter_map(unambiguous_completion_candidate),
    );
    items
}

pub(super) fn struct_literal_field_completion_items(
    resolved: &ResolveOutput,
    literal: &StructLiteralExpr,
    offset: usize,
) -> Vec<CompletionItemInfo> {
    let Some(owner) = value_member_owner(resolved, &literal.ty) else {
        return Vec::new();
    };
    let used_fields = literal
        .fields
        .iter()
        .filter(|field| !span_contains(field.name_span, offset))
        .map(|field| field.name.as_str())
        .collect::<HashSet<_>>();

    owner
        .symbol
        .fields
        .iter()
        .filter(|field| field.is_accessible && !used_fields.contains(field.name.as_str()))
        .map(|field| struct_field_completion_item(field, resolved, true, &owner.substitutions))
        .collect()
}

pub(super) fn enum_variant_completion_items(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
) -> Vec<CompletionItemInfo> {
    let owner = type_symbol_presentation_label(symbol, resolved);
    symbol
        .variants
        .iter()
        .map(|variant| enum_variant_completion_item(variant, &owner, resolved))
        .collect()
}

fn enum_variant_completion_item(
    variant: &EnumVariantSignature,
    owner: &str,
    resolved: &ResolveOutput,
) -> CompletionItemInfo {
    let payload = variant
        .payload
        .iter()
        .map(|parameter| parameter_detail(parameter, resolved))
        .collect::<Vec<_>>();
    CompletionItemInfo {
        label: variant.name.clone(),
        kind: CompletionItemKind::EnumMember,
        detail: Some(enum_variant_member_label(owner, &variant.name, &payload)),
        documentation: None,
        insert_text: Some(if payload.is_empty() {
            variant.name.clone()
        } else {
            format!("{}(_)", variant.name)
        }),
        sort_text: None,
        declaration_span: Some(variant.name_span),
    }
}

fn associated_function_completion_item(
    function: &AssociatedFunctionSignature,
    owner: &str,
    resolved: &ResolveOutput,
) -> CompletionItemInfo {
    CompletionItemInfo {
        label: function.name.clone(),
        kind: CompletionItemKind::Function,
        detail: Some(callable_detail(
            "func",
            &qualified_member_name(owner, &function.name),
            &function.signature,
            resolved,
        )),
        documentation: None,
        insert_text: Some(format!("{}()", function.name)),
        sort_text: None,
        declaration_span: Some(function.name_span),
    }
}

fn struct_field_completion_item(
    field: &StructFieldSignature,
    resolved: &ResolveOutput,
    literal: bool,
    substitutions: &HashMap<String, TypeExpr>,
) -> CompletionItemInfo {
    let ty = substitute_type_expr_parameters(&field.ty, substitutions);
    let owner = substitutions
        .get("Self")
        .map(|ty| type_expr_presentation_label(ty, resolved))
        .unwrap_or_else(|| "Self".to_string());
    CompletionItemInfo {
        label: field.name.clone(),
        kind: CompletionItemKind::Field,
        detail: Some(field_member_label(
            &owner,
            &field.name,
            &type_expr_presentation_label(&ty, resolved),
        )),
        documentation: None,
        insert_text: Some(if literal {
            format!("{}: ", field.name)
        } else {
            field.name.clone()
        }),
        sort_text: None,
        declaration_span: Some(field.name_span),
    }
}

fn method_completion_item(
    method: &MethodSignature,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, TypeExpr>,
) -> CompletionItemInfo {
    let mut substitutions = substitutions.clone();
    if let Some(impl_target) = &method.impl_target_ty {
        let impl_target = substitute_type_expr_parameters(impl_target, &substitutions);
        substitutions.insert("Self".to_string(), impl_target);
    }
    let receiver_owner = substitutions
        .get("Self")
        .map(|ty| type_expr_presentation_label(ty, resolved))
        .unwrap_or_else(|| "Self".to_string());
    let receiver = format!("{}{receiver_owner}", method.receiver.mode.source_prefix());
    let return_type =
        substitute_type_expr_parameters(&method.signature.return_type, &substitutions);
    let mut detail = format!(
        "method {}.{}({}): {}",
        receiver,
        method.name,
        method
            .signature
            .parameters
            .iter()
            .map(|parameter| {
                let ty = substitute_type_expr_parameters(&parameter.ty, &substitutions);
                format!(
                    "{}: {}",
                    parameter.name,
                    type_expr_presentation_label(&ty, resolved)
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
        type_expr_presentation_label(&return_type, resolved)
    );
    if let Some(clause) = &method.signature.result_provenance {
        detail.push_str(" from ");
        detail.push_str(
            &clause
                .origins
                .iter()
                .map(|origin| origin.kind.source_label())
                .collect::<Vec<_>>()
                .join(" | "),
        );
    }
    CompletionItemInfo {
        label: method.name.clone(),
        kind: CompletionItemKind::Method,
        detail: Some(detail),
        documentation: None,
        insert_text: Some(format!("{}()", method.name)),
        sort_text: None,
        declaration_span: Some(method.name_span),
    }
}

fn generic_bound_member_owners<'a>(
    ast: &'a AstFile,
    resolved: &'a ResolveOutput,
    ty: &TypeExpr,
    offset: usize,
) -> Vec<ValueMemberOwner<'a>> {
    let Some(parameter_name) = borrowed_reference_name(ty) else {
        return Vec::new();
    };
    generic_bounds_at_offset(ast, parameter_name, offset)
        .into_iter()
        .filter_map(|bound| {
            let mut owner = value_member_owner(resolved, bound)?;
            owner.substitutions.insert(
                "Self".to_string(),
                TypeExpr::Reference(crate::ast::TypeReference {
                    span: ty.span(),
                    name: parameter_name.to_string(),
                }),
            );
            Some(owner)
        })
        .collect()
}

fn borrowed_reference_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Borrow(borrow) => borrowed_reference_name(&borrow.inner),
        _ => None,
    }
}

fn generic_bounds_at_offset<'a>(
    ast: &'a AstFile,
    parameter_name: &str,
    offset: usize,
) -> Vec<&'a TypeExpr> {
    ast.items
        .iter()
        .find_map(|item| {
            let generics = match item {
                Item::Function(function) if span_contains(function.body.span, offset) => {
                    &function.generics
                }
                Item::Impl(impl_) if span_contains(impl_.span, offset) => &impl_.generics,
                _ => return None,
            };
            generics
                .parameters
                .iter()
                .find(|parameter| parameter.name == parameter_name)
                .map(|parameter| parameter.bounds.iter().collect())
        })
        .unwrap_or_default()
}

fn unambiguous_capability_member_items(
    owners: Vec<ValueMemberOwner<'_>>,
    resolved: &ResolveOutput,
    can_readwrite: bool,
    can_move: bool,
    use_source: crate::source::SourceId,
) -> Vec<CompletionItemInfo> {
    let mut by_label: HashMap<String, Vec<CompletionItemInfo>> = HashMap::new();
    for owner in owners {
        for item in
            value_member_completion_items(&owner, resolved, can_readwrite, can_move, use_source)
        {
            by_label.entry(item.label.clone()).or_default().push(item);
        }
    }
    let mut items = by_label
        .into_values()
        .filter_map(unambiguous_completion_candidate)
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.label.cmp(&right.label));
    items
}

fn unambiguous_completion_candidate(
    candidates: Vec<CompletionItemInfo>,
) -> Option<CompletionItemInfo> {
    let identities = candidates
        .iter()
        .filter_map(|item| item.declaration_span)
        .collect::<HashSet<_>>();
    if identities.len() != 1 {
        return None;
    }
    candidates.into_iter().next()
}

pub(super) struct ValueMemberOwner<'a> {
    pub(super) symbol: &'a TypeSymbol,
    pub(super) substitutions: HashMap<String, TypeExpr>,
}

fn value_member_owner<'a>(
    resolved: &'a ResolveOutput,
    ty: &TypeExpr,
) -> Option<ValueMemberOwner<'a>> {
    match ty {
        TypeExpr::Closure(_) => None,
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            Some(ValueMemberOwner {
                symbol,
                substitutions: HashMap::from([("Self".to_string(), ty.clone())]),
            })
        }
        TypeExpr::Generic(generic) => {
            let symbol = resolved.type_symbol_by_reference_name(&generic.name)?;
            let mut substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect::<HashMap<_, _>>();
            substitutions.insert("Self".to_string(), ty.clone());
            Some(ValueMemberOwner {
                symbol,
                substitutions,
            })
        }
        TypeExpr::Borrow(borrow) => value_member_owner(resolved, &borrow.inner),
        TypeExpr::View(view) => value_member_owner(resolved, &view.element),
        TypeExpr::Pointer(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

fn owner_type_is_readwrite(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Borrow(borrow) if borrow.is_readwrite)
}

fn method_receiver_is_available(
    method: &MethodSignature,
    can_readwrite: bool,
    can_move: bool,
) -> bool {
    match method.receiver.mode {
        MethodReceiverMode::ReadwriteBorrow => can_readwrite,
        MethodReceiverMode::ReadonlyBorrow => true,
        MethodReceiverMode::Owned => can_move,
    }
}

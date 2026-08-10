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
        let mut items = value_member_completion_items(
            &owner,
            resolved,
            can_readwrite,
            can_move,
            owner_span.source,
        );
        let mut original_method_names = owner
            .symbol
            .methods
            .iter()
            .filter(|method| method.is_accessible)
            .map(|method| method.name.clone())
            .collect::<HashSet<_>>();
        if let Some(self_ty) = owner.substitutions.get("Self") {
            original_method_names.extend(
                interface_method_completion_candidates(self_ty, owner_span.source, resolved)
                    .into_iter()
                    .map(|candidate| candidate.method.name.clone()),
            );
        }
        let mut coerced_by_name: HashMap<String, Vec<CompletionItemInfo>> = HashMap::new();
        for (coerced, coerced_can_readwrite) in
            receiver_coercion_member_owners(&owner, resolved, can_readwrite, owner_span.source)
        {
            for item in value_member_completion_items(
                &coerced,
                resolved,
                coerced_can_readwrite,
                false,
                owner_span.source,
            )
            .into_iter()
            .filter(|item| item.kind == CompletionItemKind::Method)
            .filter(|item| !original_method_names.contains(&item.label))
            {
                coerced_by_name
                    .entry(item.label.clone())
                    .or_default()
                    .push(item);
            }
        }
        items.extend(coerced_by_name.into_values().filter_map(|mut candidates| {
            let mut declarations = HashSet::new();
            candidates.retain(|candidate| declarations.insert(candidate.declaration_span));
            (candidates.len() == 1).then(|| candidates.remove(0))
        }));
        return items;
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
    let mut items = Vec::new();
    if symbol.kind == TypeSymbolKind::Enum {
        items.extend(enum_variant_completion_items(symbol, resolved));
    }
    items.extend(
        symbol
            .associated_functions
            .iter()
            .filter(|function| function.is_accessible)
            .map(|function| associated_function_completion_item(function, symbol, resolved)),
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
    let mut methods_by_name: HashMap<&str, Vec<CompletionItemInfo>> = HashMap::new();
    for method in owner.symbol.methods.iter().filter(|method| {
        method.is_accessible && method_receiver_is_available(method, can_readwrite, can_move)
    }) {
        methods_by_name
            .entry(method.name.as_str())
            .or_default()
            .push(method_completion_item(
                method,
                resolved,
                &owner.substitutions,
            ));
    }
    let Some(self_ty) = owner.substitutions.get("Self") else {
        items.extend(
            methods_by_name
                .into_values()
                .filter_map(unambiguous_completion_candidate),
        );
        return items;
    };
    for candidate in interface_method_completion_candidates(self_ty, use_source, resolved) {
        if !method_receiver_is_available(candidate.method, can_readwrite, can_move) {
            continue;
        }
        methods_by_name
            .entry(candidate.method.name.as_str())
            .or_default()
            .push(method_completion_item(
                candidate.method,
                resolved,
                &candidate.substitutions,
            ));
    }
    items.extend(
        methods_by_name
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
    owner: &TypeSymbol,
    resolved: &ResolveOutput,
) -> CompletionItemInfo {
    let is_construction =
        crate::analysis::constructions::construction_owns_function(owner, &function.name);
    CompletionItemInfo {
        label: function.name.clone(),
        kind: if is_construction {
            CompletionItemKind::Constructor
        } else {
            CompletionItemKind::Function
        },
        detail: Some(
            crate::analysis::presentation::associated_function_presentation(
                owner, function, resolved,
            )
            .render(),
        ),
        documentation: None,
        insert_text: Some(format!("{}()", function.name)),
        sort_text: crate::analysis::constructions::construction_function_is_default(
            owner,
            &function.name,
        )
        .then(|| format!("0-{}", function.name)),
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
    CompletionItemInfo {
        label: method.name.clone(),
        kind: CompletionItemKind::Method,
        detail: Some(
            crate::analysis::presentation::method_presentation_with_substitutions(
                method,
                substitutions,
                resolved,
            )
            .render(),
        ),
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
            let (generics, requirements) = match item {
                Item::Function(function)
                    if function
                        .body
                        .as_ref()
                        .is_some_and(|body| span_contains(body.span, offset)) =>
                {
                    (&function.generics, function.requirements.as_ref())
                }
                Item::Impl(impl_) if span_contains(impl_.span, offset) => {
                    (&impl_.generics, impl_.requirements.as_ref())
                }
                _ => return None,
            };
            generics
                .parameters
                .iter()
                .find(|parameter| parameter.name == parameter_name)
                .map(|_| {
                    requirements
                        .into_iter()
                        .flat_map(|clause| clause.generic_requirements())
                        .filter(|requirement| requirement.name == parameter_name)
                        .flat_map(|requirement| &requirement.bounds)
                        .collect()
                })
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
        TypeExpr::Callable(_) | TypeExpr::Closure(_) => None,
        TypeExpr::Reference(reference) => {
            let symbol = if reference.name == "str" {
                resolved
                    .builtin_type_surface(crate::builtin_types::BuiltinTypeOwner::Str)
                    .map(|surface| &surface.symbol)?
            } else {
                resolved.type_symbol_by_reference_name(&reference.name)?
            };
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
        TypeExpr::Projection(_) => {
            let normalized = crate::typecheck::normalize_associated_type_expr(ty, resolved)?;
            value_member_owner(resolved, &normalized)
        }
        TypeExpr::Borrow(borrow) => value_member_owner(resolved, &borrow.inner),
        TypeExpr::View(view) => {
            let symbol = resolved
                .builtin_type_surface(crate::builtin_types::BuiltinTypeOwner::Slice)
                .map(|surface| &surface.symbol)?;
            Some(ValueMemberOwner {
                symbol,
                substitutions: HashMap::from([
                    ("T".to_string(), view.element.as_ref().clone()),
                    ("Self".to_string(), ty.clone()),
                ]),
            })
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

fn receiver_coercion_member_owners<'a>(
    source: &ValueMemberOwner<'a>,
    resolved: &'a ResolveOutput,
    can_readwrite: bool,
    _use_source: crate::source::SourceId,
) -> Vec<(ValueMemberOwner<'a>, bool)> {
    source
        .symbol
        .coercions
        .iter()
        .filter(|coercion| coercion.is_accessible)
        .filter(|coercion| {
            coercion.receiver.mode != crate::ast::MethodReceiverMode::Owned
                && (can_readwrite
                    || coercion.receiver.mode != crate::ast::MethodReceiverMode::ReadwriteBorrow)
        })
        .filter_map(|coercion| {
            let target = substitute_type_expr_parameters(&coercion.target, &source.substitutions);
            let target_is_readwrite = owner_type_is_readwrite(&target)
                || matches!(target, TypeExpr::View(ref view) if view.is_readwrite);
            value_member_owner(resolved, &target).map(|owner| (owner, target_is_readwrite))
        })
        .collect()
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

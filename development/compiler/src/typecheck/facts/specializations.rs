use super::*;

pub(super) fn expected_attempt_type(
    expression: &Expr,
    expected_success: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression_type(expression, resolved, environment) {
        Type::Fallible { error, .. } => Type::Fallible {
            success: Box::new(match expected_success {
                Type::Fallible { success, .. } => success.as_ref().clone(),
                _ => expected_success.clone(),
            }),
            error,
        },
        Type::Optional(_) => Type::Optional(Box::new(match expected_success {
            Type::Optional(inner) => inner.as_ref().clone(),
            _ => expected_success.clone(),
        })),
        _ => expected_success.clone(),
    }
}

pub(super) fn function_call_specialization(
    call: &crate::ast::CallExpr,
    declaration_span: ByteSpan,
    base_target_name: &str,
    signature: &FunctionSignature,
    expected_return_type: Option<&Type>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<FunctionCallSpecialization> {
    let checked = resolved_call_signature(resolved, call, environment)?;
    let mut substitution_types = infer_generic_substitutions(call, &checked, resolved, environment);
    if let Some(expected_return_type) = expected_return_type {
        let parameters = signature
            .generic_parameters
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        infer_type_expr_substitutions(
            &signature.return_type,
            expected_return_type,
            resolved,
            checked.self_type.as_ref(),
            &parameters,
            &mut substitution_types,
        );
    }
    if !signature
        .generic_parameters
        .iter()
        .all(|parameter| substitution_types.contains_key(parameter))
    {
        return None;
    }
    let type_arguments = signature
        .generic_parameters
        .iter()
        .map(|parameter| substitution_types.get(parameter).map(Type::display))
        .collect::<Option<Vec<_>>>()?;
    let mut free_type_parameters = HashSet::new();
    let substitutions = substitution_types
        .into_iter()
        .map(|(name, ty)| {
            type_to_type_expr_allowing_parameters(&ty, call.span, &mut free_type_parameters)
                .map(|ty| (name, ty))
        })
        .collect::<Option<HashMap<_, _>>>()?;

    Some(FunctionCallSpecialization {
        declaration_span,
        base_target_name: base_target_name.to_string(),
        generic_parameters: signature.generic_parameters.clone(),
        target_name: format!("{base_target_name}<{}>", type_arguments.join(", ")),
        substitutions,
        free_type_parameters,
    })
}

pub(super) fn method_call_specialization(
    call: &crate::ast::CallExpr,
    member: &MemberExpr,
    method: &MethodSignature,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<MethodCallSpecialization> {
    let self_type = resolved_method_call(resolved, call, environment)?.self_type;
    let mut free_type_parameters = HashSet::new();
    let self_ty = type_to_type_expr_allowing_parameters(
        &self_type,
        member.object.span(),
        &mut free_type_parameters,
    )?;
    let checked = resolved_call_signature(resolved, call, environment)?;
    let substitutions = infer_generic_substitutions(call, &checked, resolved, environment)
        .into_iter()
        .map(|(name, ty)| {
            type_to_type_expr_allowing_parameters(
                &ty,
                member.member_span,
                &mut free_type_parameters,
            )
            .map(|ty| (name, ty))
        })
        .collect::<Option<HashMap<_, _>>>()?;
    if !method
        .signature
        .generic_parameters
        .iter()
        .all(|parameter| substitutions.contains_key(parameter))
    {
        return None;
    }

    Some(MethodCallSpecialization {
        declaration_span: method.name_span,
        method_name: method.name.clone(),
        target_name: method_target_name_from_self_ty(&self_ty, &method.name),
        self_ty,
        generic_parameters: method.signature.generic_parameters.clone(),
        substitutions,
        free_type_parameters,
    })
}

pub(super) fn specialized_target_name(
    base_target_name: &str,
    generic_parameters: &[String],
    substitutions: &HashMap<String, TypeExpr>,
) -> Option<String> {
    let type_arguments = generic_parameters
        .iter()
        .map(|parameter| substitutions.get(parameter).map(canonical_type_expr))
        .collect::<Option<Vec<_>>>()?;
    Some(format!("{base_target_name}<{}>", type_arguments.join(", ")))
}

pub(super) fn method_target_name_from_self_ty(self_ty: &TypeExpr, method_name: &str) -> String {
    format!("{}.{}", canonical_type_expr(self_ty), method_name)
}

pub(super) fn drop_target_name_from_base_and_self_ty(
    base_target_name: &str,
    self_ty: &TypeExpr,
) -> String {
    let Some(base_type_name) = base_target_name.strip_suffix(".drop") else {
        return base_target_name.to_string();
    };
    let TypeExpr::Generic(generic) = self_ty else {
        return base_target_name.to_string();
    };
    let arguments = generic
        .arguments
        .iter()
        .map(canonical_type_expr)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{base_type_name}<{arguments}>.drop")
}

pub(crate) fn drop_type_specialization_from_self_ty(
    self_ty: &TypeExpr,
    resolved: &ResolveOutput,
    free_type_parameters: HashSet<String>,
) -> Option<DropTypeSpecialization> {
    drop_type_specialization_from_self_ty_inner(
        self_ty,
        resolved,
        free_type_parameters,
        &mut HashSet::new(),
    )
}

pub(super) fn drop_type_specialization_from_self_ty_inner(
    self_ty: &TypeExpr,
    resolved: &ResolveOutput,
    free_type_parameters: HashSet<String>,
    resolving_names: &mut HashSet<String>,
) -> Option<DropTypeSpecialization> {
    match self_ty {
        TypeExpr::Optional(optional) => {
            return drop_type_specialization_from_self_ty_inner(
                &optional.inner,
                resolved,
                free_type_parameters,
                resolving_names,
            );
        }
        TypeExpr::Fallible(fallible) => {
            return drop_type_specialization_from_self_ty_inner(
                &fallible.success,
                resolved,
                free_type_parameters,
                resolving_names,
            );
        }
        _ => {}
    }

    let (type_name, substitutions) = match self_ty {
        TypeExpr::Reference(reference) => (reference.name.as_str(), HashMap::new()),
        TypeExpr::Generic(generic) => {
            let symbol = resolved.type_symbol_by_reference_name(&generic.name)?;
            if symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            (generic.name.as_str(), substitutions)
        }
        _ => return None,
    };
    let symbol = resolved.type_symbol_by_reference_name(type_name)?;
    if symbol.kind == crate::resolve::TypeSymbolKind::Alias {
        let target = symbol.alias_target.as_ref()?;
        if !resolving_names.insert(symbol.canonical_name.clone()) {
            return None;
        }
        let target = substitute_type_expr_parameters(target, &substitutions);
        let specialization = drop_type_specialization_from_self_ty_inner(
            &target,
            resolved,
            free_type_parameters,
            resolving_names,
        );
        resolving_names.remove(&symbol.canonical_name);
        return specialization;
    }

    let destructor = symbol.destructor.as_ref()?;
    Some(DropTypeSpecialization {
        declaration_span: destructor.name_span,
        target_name: drop_target_name_from_base_and_self_ty(&destructor.target_name, self_ty),
        self_ty: self_ty.clone(),
        base_target_name: destructor.target_name.clone(),
        free_type_parameters,
    })
}

pub(super) fn payload_enum_symbol_and_substitutions_for_type_expr<'a>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
) -> Option<(&'a TypeSymbol, HashMap<String, TypeExpr>)> {
    payload_enum_symbol_and_substitutions_for_type_expr_inner(ty, resolved, &mut HashSet::new())
}

pub(super) fn payload_enum_symbol_and_substitutions_for_type_expr_inner<'a>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Option<(&'a TypeSymbol, HashMap<String, TypeExpr>)> {
    match ty {
        TypeExpr::Callable(_) | TypeExpr::Closure(_) | TypeExpr::Opaque(_) => None,
        TypeExpr::Projection(_) => {
            let normalized = super::super::normalize_associated_type_expr(ty, resolved)?;
            payload_enum_symbol_and_substitutions_for_type_expr_inner(
                &normalized,
                resolved,
                resolving_names,
            )
        }
        TypeExpr::Reference(reference) => {
            let symbol = resolved.type_symbol_by_reference_name(&reference.name)?;
            match symbol.kind {
                TypeSymbolKind::Enum if symbol.generic_arity == 0 => Some((symbol, HashMap::new())),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return None;
                    }
                    let result = payload_enum_symbol_and_substitutions_for_type_expr_inner(
                        target,
                        resolved,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Struct | TypeSymbolKind::Enum | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Generic(generic) => {
            let symbol = resolved.type_symbol_by_reference_name(&generic.name)?;
            if symbol.generic_arity != generic.arguments.len() {
                return None;
            }
            let substitutions: HashMap<String, TypeExpr> = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect();
            match symbol.kind {
                TypeSymbolKind::Enum => Some((symbol, substitutions)),
                TypeSymbolKind::Alias => {
                    let target = symbol.alias_target.as_ref()?;
                    if !resolving_names.insert(symbol.canonical_name.clone()) {
                        return None;
                    }
                    let target = substitute_type_expr_parameters(target, &substitutions);
                    let result = payload_enum_symbol_and_substitutions_for_type_expr_inner(
                        &target,
                        resolved,
                        resolving_names,
                    );
                    resolving_names.remove(&symbol.canonical_name);
                    result
                }
                TypeSymbolKind::Struct | TypeSymbolKind::Interface => None,
            }
        }
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

pub(super) fn free_type_parameters_in_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> HashSet<String> {
    let mut parameters = HashSet::new();
    collect_free_type_parameters_in_type_expr(ty, resolved, &mut parameters);
    parameters
}

pub(super) fn collect_free_type_parameters_in_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    parameters: &mut HashSet<String>,
) {
    match ty {
        TypeExpr::Callable(callable) => {
            for parameter in &callable.parameters {
                collect_free_type_parameters_in_type_expr(&parameter.ty, resolved, parameters);
            }
            collect_free_type_parameters_in_type_expr(&callable.return_type, resolved, parameters);
        }
        TypeExpr::Closure(closure) => {
            for capture in &closure.captures {
                collect_free_type_parameters_in_type_expr(&capture.ty, resolved, parameters);
            }
            for parameter in &closure.parameters {
                collect_free_type_parameters_in_type_expr(parameter, resolved, parameters);
            }
            collect_free_type_parameters_in_type_expr(&closure.return_type, resolved, parameters);
        }
        TypeExpr::Opaque(opaque) => {
            collect_free_type_parameters_in_type_expr(&opaque.interface, resolved, parameters);
            for binding in &opaque.associated_bindings {
                collect_free_type_parameters_in_type_expr(&binding.value, resolved, parameters);
            }
            if let Some(witness) = &opaque.witness {
                collect_free_type_parameters_in_type_expr(witness, resolved, parameters);
            }
        }
        TypeExpr::Reference(reference) => {
            if resolved
                .type_symbol_by_reference_name(&reference.name)
                .is_none()
                && !builtin_type_name(&reference.name)
            {
                parameters.insert(reference.name.clone());
            }
        }
        TypeExpr::Generic(generic) => {
            for argument in &generic.arguments {
                collect_free_type_parameters_in_type_expr(argument, resolved, parameters);
            }
        }
        TypeExpr::Projection(projection) => {
            collect_free_type_parameters_in_type_expr(&projection.base, resolved, parameters);
        }
        TypeExpr::Pointer(pointer) => {
            collect_free_type_parameters_in_type_expr(&pointer.inner, resolved, parameters);
        }
        TypeExpr::Borrow(borrow) => {
            collect_free_type_parameters_in_type_expr(&borrow.inner, resolved, parameters);
        }
        TypeExpr::View(view) => {
            collect_free_type_parameters_in_type_expr(&view.element, resolved, parameters);
        }
        TypeExpr::Array(array) => {
            collect_free_type_parameters_in_type_expr(&array.element, resolved, parameters);
        }
        TypeExpr::Optional(optional) => {
            collect_free_type_parameters_in_type_expr(&optional.inner, resolved, parameters);
        }
        TypeExpr::Fallible(fallible) => {
            collect_free_type_parameters_in_type_expr(&fallible.success, resolved, parameters);
            collect_free_type_parameters_in_type_expr(&fallible.error, resolved, parameters);
        }
    }
}

pub(super) fn builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "isize"
            | "error"
            | "str"
            | "void"
            | "never"
            | "Self"
    )
}

pub(super) fn type_expr_contains_free_parameters(
    ty: &TypeExpr,
    free_type_parameters: &HashSet<String>,
) -> bool {
    match ty {
        TypeExpr::Callable(callable) => {
            callable.parameters.iter().any(|parameter| {
                type_expr_contains_free_parameters(&parameter.ty, free_type_parameters)
            }) || type_expr_contains_free_parameters(&callable.return_type, free_type_parameters)
        }
        TypeExpr::Closure(closure) => {
            closure.captures.iter().any(|capture| {
                type_expr_contains_free_parameters(&capture.ty, free_type_parameters)
            }) || closure.parameters.iter().any(|parameter| {
                type_expr_contains_free_parameters(parameter, free_type_parameters)
            }) || type_expr_contains_free_parameters(&closure.return_type, free_type_parameters)
        }
        TypeExpr::Opaque(opaque) => {
            type_expr_contains_free_parameters(&opaque.interface, free_type_parameters)
                || opaque.associated_bindings.iter().any(|binding| {
                    type_expr_contains_free_parameters(&binding.value, free_type_parameters)
                })
                || opaque.witness.as_ref().is_some_and(|witness| {
                    type_expr_contains_free_parameters(witness, free_type_parameters)
                })
        }
        TypeExpr::Reference(reference) => free_type_parameters.contains(&reference.name),
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .any(|argument| type_expr_contains_free_parameters(argument, free_type_parameters)),
        TypeExpr::Projection(projection) => {
            type_expr_contains_free_parameters(&projection.base, free_type_parameters)
        }
        TypeExpr::Pointer(pointer) => {
            type_expr_contains_free_parameters(&pointer.inner, free_type_parameters)
        }
        TypeExpr::Borrow(borrow) => {
            type_expr_contains_free_parameters(&borrow.inner, free_type_parameters)
        }
        TypeExpr::View(view) => {
            type_expr_contains_free_parameters(&view.element, free_type_parameters)
        }
        TypeExpr::Array(array) => {
            type_expr_contains_free_parameters(&array.element, free_type_parameters)
        }
        TypeExpr::Optional(optional) => {
            type_expr_contains_free_parameters(&optional.inner, free_type_parameters)
        }
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_free_parameters(&fallible.success, free_type_parameters)
                || type_expr_contains_free_parameters(&fallible.error, free_type_parameters)
        }
    }
}

use super::*;

pub(super) fn callable_function_signature_issues(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    root_source: SourceId,
) -> Vec<BuildabilityIssue> {
    let mut issues = callable_parameter_issues(
        &function.parameters.parameters,
        substitutions,
        resolved,
        resolved_sources,
    );
    let return_type = substitute_type_expr_parameters(&function.return_type, substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    if !callable_return_type_is_buildable_with_resolver(&return_type, resolved, &source_resolver)
        && !function_error_return_type_is_buildable(
            function,
            &return_type,
            resolved,
            &source_resolver,
            root_source,
        )
    {
        issues.push(BuildabilityIssue {
            span: function.return_type.span(),
            construct: "function return types outside the supported runtime ABI",
            help: "return `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `void`, `never`, a supported aggregate, a supported static `error` payload helper, or a fallible form with a non-`error` success type",
        });
    }
    issues
}

pub(super) fn callable_method_signature_issues(
    method: &MethodDecl,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Vec<BuildabilityIssue> {
    let receiver = method.receiver.implicit_parameter();
    let mut issues = callable_parameter_issues(
        std::slice::from_ref(&receiver),
        substitutions,
        resolved,
        resolved_sources,
    );
    issues.extend(callable_parameter_issues(
        &method.parameters.parameters,
        substitutions,
        resolved,
        resolved_sources,
    ));
    let return_type = substitute_type_expr_parameters(&method.return_type, substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    if !callable_return_type_is_buildable_with_resolver(&return_type, resolved, &source_resolver) {
        issues.push(BuildabilityIssue {
            span: method.return_type.span(),
            construct: "method return types outside the supported runtime ABI",
            help: "return `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `void`, `never`, a supported aggregate, or a fallible form with a non-`error` success type",
        });
    }
    issues
}

pub(super) fn callable_parameter_issues(
    parameters: &[Parameter],
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Vec<BuildabilityIssue> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    parameters
        .iter()
        .filter_map(|parameter| {
            let ty = substitute_type_expr_parameters(&parameter.ty, substitutions);
            if callable_parameter_type_is_buildable_with_resolver(&ty, resolved, &source_resolver) {
                return None;
            }
            Some(BuildabilityIssue {
                span: parameter.span,
                construct: "function or method parameters outside the supported runtime ABI",
                help: "use `i32`, `u8`, `usize`, `bool`, `&str`, a slice view, `error`, scalar borrow parameters, aggregate borrow parameters, or supported aggregate value parameters",
            })
        })
        .collect()
}

pub(super) fn function_error_return_type_is_buildable<'a, F>(
    function: &FunctionDecl,
    return_type: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: &F,
    root_source: SourceId,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if !type_expr_is_error_parameter_with_resolver(return_type, resolved, resolver) {
        return false;
    }

    non_root_error_constructor_signature(function, root_source, resolved, resolver)
        || (function.parameters.parameters.is_empty()
            && function.body.as_ref().is_some_and(|body| {
                static_error_payload_body_is_buildable(body, root_source, resolved, resolver)
            }))
}

pub(super) fn non_root_error_constructor_signature<'a, F>(
    function: &FunctionDecl,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    function.name_span.source != root_source
        && error_constructor_signature_is_buildable_with_resolver(
            function
                .parameters
                .parameters
                .iter()
                .map(|parameter| &parameter.ty),
            &function.return_type,
            resolved,
            resolver,
        )
}

pub(super) fn static_error_payload_body_is_buildable<'a, F>(
    body: &Block,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let mut runtime_statements = body
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)));
    let Some(Stmt::Return(statement)) = runtime_statements.next() else {
        return false;
    };
    if runtime_statements.next().is_some() {
        return false;
    }
    let Some(expression) = statement.expression.as_ref() else {
        return false;
    };
    static_error_payload_expression_is_buildable(expression, root_source, resolved, resolver)
}

pub(super) fn static_error_payload_expression_is_buildable<'a, F>(
    expression: &Expr,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match expression {
        Expr::Group(group) => static_error_payload_expression_is_buildable(
            &group.expression,
            root_source,
            resolved,
            resolver,
        ),
        Expr::Call(call) => {
            error_constructor_call_is_buildable(call, root_source, resolved, resolver)
                && call.arguments.len() == 2
                && call
                    .arguments
                    .iter()
                    .all(static_error_payload_string_expression_is_buildable)
        }
        _ => false,
    }
}

pub(super) fn static_error_payload_string_expression_is_buildable(expression: &Expr) -> bool {
    match expression {
        Expr::StringLiteral(_) => true,
        Expr::Group(group) => {
            static_error_payload_string_expression_is_buildable(&group.expression)
        }
        _ => false,
    }
}

pub(super) fn error_constructor_call_is_buildable<'a, F>(
    call: &CallExpr,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if let Some(symbol) = resolved.symbol_for_call(call)
        && symbol.declaration_span.source != root_source
        && let SymbolKind::Function(signature) | SymbolKind::Primitive(signature) = &symbol.kind
    {
        return error_constructor_signature_is_buildable_with_resolver(
            signature.parameters.iter().map(|parameter| &parameter.ty),
            &signature.return_type,
            resolved,
            resolver,
        );
    }

    if let Some((_owner, function)) = resolved.associated_function_for_call(call)
        && function.name_span.source != root_source
    {
        return error_constructor_signature_is_buildable_with_resolver(
            function
                .signature
                .parameters
                .iter()
                .map(|parameter| &parameter.ty),
            &function.signature.return_type,
            resolved,
            resolver,
        );
    }

    false
}

pub(super) fn error_constructor_signature_is_buildable_with_resolver<'a, 't, F, I>(
    parameter_types: I,
    return_type: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
    I: IntoIterator<Item = &'t TypeExpr>,
{
    let mut parameter_types = parameter_types.into_iter();
    let Some(code_type) = parameter_types.next() else {
        return false;
    };
    let Some(message_type) = parameter_types.next() else {
        return false;
    };
    if parameter_types.next().is_some() {
        return false;
    }

    type_expr_has_str_view_abi_with_resolver(code_type, fallback_resolved, resolver)
        && type_expr_has_str_view_abi_with_resolver(message_type, fallback_resolved, resolver)
        && type_expr_is_error_parameter_with_resolver(return_type, fallback_resolved, resolver)
}

pub(super) fn method_contextual_substitutions(
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> HashMap<String, TypeExpr> {
    let concrete_self_ty = substitute_type_expr_parameters(self_ty, substitutions);
    let mut contextual = substitutions.clone();
    contextual.insert("Self".to_string(), concrete_self_ty);
    contextual
}

pub(super) fn method_specialization_context_substitutions(
    impl_: &ImplDecl,
    specialization: &MethodCallSpecialization,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> HashMap<String, TypeExpr> {
    let mut substitutions =
        impl_substitutions_for_self_ty(impl_, &specialization.self_ty).unwrap_or_default();
    substitutions.extend(specialization.substitutions.clone());
    crate::typecheck::extend_associated_type_substitutions_with_resolver(
        &mut substitutions,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    );
    substitutions
}

pub(super) fn callable_parameter_type_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    callable_parameter_type_is_buildable_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn callable_parameter_type_is_buildable_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return callable_non_alias_parameter_type_is_buildable_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            let Some(target) = &symbol.alias_target else {
                return callable_non_alias_parameter_type_is_buildable_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = callable_parameter_type_is_buildable_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => callable_non_alias_parameter_type_is_buildable_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        ),
    }
}

pub(super) fn callable_non_alias_parameter_type_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let shape = outcome_shape_with_resolver(ty, fallback_resolved, resolver);
    if !shape.layers.is_empty() && shape.is_supported_callable_shape() {
        return type_expr_is_buildable_scalar_or_view_with_resolver(
            &shape.payload,
            fallback_resolved,
            resolver,
        ) || type_expr_is_supported_aggregate_value_with_resolver(
            &shape.payload,
            fallback_resolved,
            resolver,
        );
    }
    type_expr_is_buildable_scalar_or_view_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_error_parameter_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_borrow_parameter_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_aggregate_value_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_move_only_fixed_array_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        )
}

pub(super) fn callable_return_type_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    callable_return_type_is_buildable_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn callable_return_type_is_buildable_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if matches!(reference.name.as_str(), "void" | "never") => {
            true
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return callable_non_alias_return_type_is_buildable_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            let Some(target) = &symbol.alias_target else {
                return callable_non_alias_return_type_is_buildable_with_resolver(
                    ty,
                    fallback_resolved,
                    resolver,
                );
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return false;
            }
            let result = callable_return_type_is_buildable_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Fallible(fallible) => callable_return_type_is_buildable_inner(
            &fallible.success,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        TypeExpr::Optional(optional) => callable_return_type_is_buildable_inner(
            &optional.inner,
            fallback_resolved,
            resolver,
            resolving_names,
        ),
        _ => callable_non_alias_return_type_is_buildable_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        ),
    }
}

pub(super) fn callable_non_alias_return_type_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    type_expr_is_buildable_scalar_or_view_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_borrow_parameter_with_resolver(ty, fallback_resolved, resolver)
        || type_expr_is_supported_aggregate_return_with_resolver(ty, fallback_resolved, resolver)
}

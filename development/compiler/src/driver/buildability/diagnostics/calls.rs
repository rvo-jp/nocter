use super::*;

pub(in crate::driver::buildability) fn unsupported_unloaded_imported_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
) -> Option<Diagnostic> {
    let symbol = resolved.symbol_for_call(call)?;
    let SymbolKind::Imported(imported) = &symbol.kind else {
        return None;
    };

    Some(unsupported_native_build_diagnostic(
        sources,
        call.span,
        "unloaded imported function calls",
        &format!(
            "load `{}` from the active Nocter home or use a same-module function until imported placeholder lowering is promoted",
            imported.path
        ),
    ))
}

pub(in crate::driver::buildability) fn unsupported_borrow_call_argument_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let argument = call
        .arguments
        .iter()
        .enumerate()
        .find_map(|(index, argument)| {
            let parameter_ty = call_argument_parameter_type(
                call,
                index,
                resolved,
                typed_hir,
                generic_substitutions,
            )?;
            if !type_expr_resolves_to_borrow_with_resolver(
                &parameter_ty,
                resolved,
                &source_resolver,
            ) {
                return None;
            }
            match unwrap_group_expr(argument) {
                Expr::Borrow(borrow)
                    if borrow.is_readwrite
                        && !readwrite_borrow_argument_source_is_buildable(
                            &borrow.expression,
                            resolved,
                            resolved_sources,
                            typed_hir,
                            generic_substitutions,
                        ) =>
                {
                    Some(argument)
                }
                _ => None,
            }
        })?;

    Some(unsupported_native_build_diagnostic(
        sources,
        argument.span(),
        "read-write borrow call arguments from unsupported expressions",
        "borrow a mutable local binding, mutable aggregate field rooted at a binding, or supported mutable slice element until read-write temporary borrow lowering is promoted",
    ))
}

pub(in crate::driver::buildability) fn unsupported_method_borrow_receiver_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    typed_hir.method_call_target(member.member_span)?;
    if !method_call_receiver_is_readwrite_borrow(member.member_span, typed_hir) {
        return None;
    }
    if readwrite_borrow_argument_source_is_buildable(
        &member.object,
        resolved,
        resolved_sources,
        typed_hir,
        generic_substitutions,
    ) {
        return None;
    }

    Some(unsupported_native_build_diagnostic(
        sources,
        member.object.span(),
        "read-write method borrow receivers from unsupported expressions",
        "call the method on a mutable local binding, mutable aggregate field rooted at a binding, or supported mutable slice element until read-write temporary receiver lowering is promoted",
    ))
}

pub(in crate::driver::buildability) fn unsupported_unspecialized_generic_method_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    typed_hir.generic_method_call_target(member.member_span)?;
    if concrete_method_call_specialization(member, typed_hir, generic_substitutions).is_some() {
        return None;
    }

    Some(unsupported_native_build_diagnostic(
        sources,
        call.span,
        "generic impl method calls without concrete type arguments",
        "call the method through a receiver whose generic arguments are concrete until generic method bodies can be re-specialized recursively",
    ))
}

pub(in crate::driver::buildability) fn concrete_method_call_specialization(
    member: &crate::ast::MemberExpr,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<MethodCallSpecialization> {
    typed_hir
        .method_call_specialization(member.member_span)?
        .with_context_substitutions(generic_substitutions)
}

pub(in crate::driver::buildability) fn unsupported_unspecialized_generic_function_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    typed_hir.generic_function_call_target(call.span)?;
    if concrete_function_call_specialization(call, typed_hir, generic_substitutions).is_some() {
        return None;
    }

    Some(unsupported_native_build_diagnostic(
        sources,
        call.span,
        "generic function calls without concrete type arguments",
        "make every generic parameter concrete through argument types or return context",
    ))
}

pub(in crate::driver::buildability) fn concrete_function_call_specialization(
    call: &CallExpr,
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<FunctionCallSpecialization> {
    typed_hir
        .function_call_specialization(call.span)?
        .with_context_substitutions(generic_substitutions)
}

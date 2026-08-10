use super::*;

fn check_method_return_type(
    sources: &SourceMap,
    owner: &(impl MethodOwnerDecl + ?Sized),
    method: &crate::ast::MethodDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    summaries: &CallableProvenanceSummaries,
) {
    let Some(body) = &method.body else { return };
    let mut environment = environment_for_method(method, resolved, owner);
    let mut borrow_provenance = ProvenanceEnvironment::default();
    let context = ReturnContext::new(
        CallableKind::Method(method_owner_member_name(owner, &method.name)),
        type_expr_to_type_in_environment(&method.return_type, resolved, &environment),
        method.return_type.span(),
    );
    check_fallible_success_type(sources, &context, diagnostics);
    check_block_returns(
        sources,
        body,
        &context,
        resolved,
        diagnostics,
        &mut environment,
        &mut borrow_provenance,
        summaries,
    );
}

pub(in crate::typecheck::returns) fn check_instance_member_return_types(
    sources: &SourceMap,
    instance: &InstanceDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    summaries: &CallableProvenanceSummaries,
) {
    for method in &instance.methods {
        check_method_return_type(sources, instance, method, resolved, diagnostics, summaries);
    }
}

pub(in crate::typecheck::returns) fn check_destruct_return_type(
    sources: &SourceMap,
    destruct: &DestructDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    summaries: &CallableProvenanceSummaries,
) {
    let context = ReturnContext::new(
        CallableKind::Drop(format!(
            "{}.drop",
            crate::typecheck::type_expr::canonical_type_expr(&destruct.target_ty)
        )),
        Type::Void,
        destruct.binding.ty.span(),
    );
    let mut environment = environment_for_destruct(destruct, resolved);
    let mut borrow_provenance = ProvenanceEnvironment::default();
    check_block_returns(
        sources,
        &destruct.body,
        &context,
        resolved,
        diagnostics,
        &mut environment,
        &mut borrow_provenance,
        summaries,
    );
}

pub(in crate::typecheck::returns) fn check_conformance_member_return_types(
    sources: &SourceMap,
    conformance: &ConformanceDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    summaries: &CallableProvenanceSummaries,
) {
    for member in &conformance.members {
        if let ConformanceMember::Method(method) = member {
            check_method_return_type(
                sources,
                conformance,
                method,
                resolved,
                diagnostics,
                summaries,
            );
        }
    }
}

pub(in crate::typecheck::returns) fn check_fallible_success_type(
    sources: &SourceMap,
    context: &ReturnContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Type::Fallible { success, .. } = &context.declared_type else {
        return;
    };

    if success_type_accepts_bare_error(success) {
        diagnostics.push(fallible_success_error_diagnostic(sources, context));
    }
}

pub(in crate::typecheck::returns) fn success_type_accepts_bare_error(ty: &Type) -> bool {
    match ty {
        Type::Error => true,
        Type::Optional(inner) => success_type_accepts_bare_error(inner),
        _ => false,
    }
}

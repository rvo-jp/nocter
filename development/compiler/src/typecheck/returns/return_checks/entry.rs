use super::*;

pub(in crate::typecheck::returns) fn check_impl_member_return_types(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    summaries: &CallableProvenanceSummaries,
) {
    for member in &impl_.members {
        match member {
            ImplMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut environment = environment_for_method(method, resolved, impl_);
                let mut borrow_provenance = ProvenanceEnvironment::default();
                let context = ReturnContext::new(
                    CallableKind::Method(impl_member_name(impl_, &method.name)),
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
            ImplMember::Drop(drop_) => {
                let context = ReturnContext::new(
                    CallableKind::Drop(impl_member_name(impl_, "drop")),
                    Type::Void,
                    drop_.binding.ty.span(),
                );
                let mut environment = environment_for_parameters_in_impl(
                    std::slice::from_ref(&drop_.binding),
                    resolved,
                    impl_,
                );
                let mut borrow_provenance = ProvenanceEnvironment::default();
                check_block_returns(
                    sources,
                    &drop_.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut borrow_provenance,
                    summaries,
                );
            }
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

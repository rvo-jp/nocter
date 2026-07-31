use super::*;

pub(in crate::driver::buildability) fn payload_enum_variant_payloads_are_supported<'a, F>(
    payloads: &[crate::resolve::ParameterSignature],
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match payloads {
        [] => true,
        [payload] => {
            let ty = substitute_type_expr_parameters(&payload.ty, substitutions);
            payload_enum_payload_type_is_supported(&ty, fallback_resolved, resolver, true)
        }
        payloads => payloads.iter().all(|payload| {
            let ty = substitute_type_expr_parameters(&payload.ty, substitutions);
            payload_enum_payload_type_is_supported(&ty, fallback_resolved, resolver, true)
        }),
    }
}

pub(in crate::driver::buildability) fn payload_enum_payload_type_is_supported<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    allow_active_drop: bool,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_with_resolver(ty, fallback_resolved, resolver).is_ok()
        && (type_expr_is_runtime_copy_value_with_resolver(
            ty,
            fallback_resolved,
            resolver,
            &mut HashSet::new(),
        ) || (allow_active_drop
            && type_expr_has_supported_recursive_drop_with_resolver(
                ty,
                fallback_resolved,
                resolver,
                &mut HashSet::new(),
            )))
}

pub(in crate::driver::buildability) fn payload_enum_constructor_call_is_supported(
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::Member(member) = call.callee.as_ref() else {
        return false;
    };
    if typecheck_facts
        .enum_variant_target(member.member_span)
        .is_none()
    {
        return false;
    }
    typecheck_facts
        .expression_type_expr(call.span)
        .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
        .is_some_and(|ty| {
            type_expr_is_supported_payload_enum_value_for_sources(&ty, resolved, resolved_sources)
        })
}

use super::*;

pub(in crate::driver::buildability) fn binding_initializer_may_use_value_control_expression(
    statement: &crate::ast::BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let ty = statement.ty.clone().or_else(|| {
        typecheck_facts
            .binding_type_expr(statement.name_span)
            .cloned()
    });
    let Some(ty) = ty else {
        return false;
    };
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    type_expr_is_buildable_scalar_or_view_for_sources(&ty, resolved, resolved_sources)
}

pub(in crate::driver::buildability) fn assignment_value_may_use_value_control_expression(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(&statement.target) {
        Expr::Identifier(identifier) => {
            let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
                return false;
            };
            typecheck_facts
                .binding_type_expr(symbol.name_span)
                .map(|ty| substitute_type_expr_parameters(ty, generic_substitutions))
                .is_some_and(|ty| {
                    type_expr_is_buildable_scalar_or_view_for_sources(
                        &ty,
                        resolved,
                        resolved_sources,
                    )
                })
        }
        Expr::Member(member) => typecheck_facts
            .field_scalar_view_kind(member.member_span)
            .is_some_and(field_kind_may_use_value_control_expression),
        _ => false,
    }
}

pub(in crate::driver::buildability) fn call_argument_may_use_value_control_expression(
    call: &CallExpr,
    index: usize,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )
    .is_some_and(|ty| {
        type_expr_is_buildable_scalar_or_view_for_sources(&ty, resolved, resolved_sources)
    })
}

pub(in crate::driver::buildability) fn otherwise_aggregate_argument_parameter_type(
    call: &CallExpr,
    index: usize,
    argument: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let Expr::Otherwise(_) = unwrap_group_expr(argument) else {
        return None;
    };
    let ty = call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

pub(in crate::driver::buildability) fn otherwise_aggregate_struct_field_type(
    field: &StructLiteralField,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let Expr::Otherwise(_) = unwrap_group_expr(&field.value) else {
        return None;
    };
    let ty = field_type_expr_for_span(field.name_span, resolved, typecheck_facts)?;
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

pub(in crate::driver::buildability) fn otherwise_aggregate_member_root_type(
    member: &MemberExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let otherwise = aggregate_member_root_otherwise(&member.object)?;
    let Expr::Call(call) = unwrap_group_expr(&otherwise.value) else {
        return None;
    };
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let ty = type_expr_top_level_optional_success_with_resolver(
        &return_type,
        resolved,
        &source_resolver,
    )?;
    type_expr_is_supported_aggregate_value_with_resolver(&ty, resolved, &source_resolver)
        .then_some(ty)
}

pub(in crate::driver::buildability) fn aggregate_member_root_otherwise(
    expression: &Expr,
) -> Option<&OtherwiseExpr> {
    match unwrap_group_expr(expression) {
        Expr::Otherwise(otherwise) => Some(otherwise),
        Expr::Member(member) => aggregate_member_root_otherwise(&member.object),
        _ => None,
    }
}

pub(in crate::driver::buildability) fn call_argument_parameter_type(
    call: &CallExpr,
    index: usize,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    if let Expr::Member(member) = call.callee.as_ref()
        && let Some(ty) = method_call_argument_parameter_type(
            member,
            index,
            resolved,
            typecheck_facts,
            generic_substitutions,
        )
    {
        return Some(ty);
    }

    let signature = resolved.call_signature_for_call(call)?;
    let parameter = signature.parameters.get(index)?;
    let mut ty = parameter.ty.clone();

    if let Some(specialization) =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)
    {
        ty = substitute_type_expr_parameters(&ty, &specialization.substitutions);
    }

    ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    Some(ty)
}

pub(in crate::driver::buildability) fn method_call_argument_parameter_type(
    member: &crate::ast::MemberExpr,
    index: usize,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let method_name_span = typecheck_facts.method_call_target(member.member_span)?;
    let method = resolved.method_signature_by_name_span(method_name_span)?;
    let parameter = method.signature.parameters.get(index)?;
    let mut ty = parameter.ty.clone();

    if let Some(specialization) =
        concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)
    {
        let self_substitution =
            HashMap::from([("Self".to_string(), specialization.self_ty.clone())]);
        ty = substitute_type_expr_parameters(&ty, &self_substitution);
        ty = substitute_type_expr_parameters(&ty, &specialization.substitutions);
        return Some(substitute_type_expr_parameters(&ty, generic_substitutions));
    }

    if typecheck_facts
        .generic_method_call_target(member.member_span)
        .is_some()
    {
        return None;
    }

    if let Some(self_ty) = &method.impl_target_ty {
        let self_substitution = HashMap::from([("Self".to_string(), self_ty.clone())]);
        ty = substitute_type_expr_parameters(&ty, &self_substitution);
    }
    Some(substitute_type_expr_parameters(&ty, generic_substitutions))
}

pub(in crate::driver::buildability) fn struct_literal_field_may_use_value_control_expression(
    field_name_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    typecheck_facts
        .field_scalar_view_kind(field_name_span)
        .is_some_and(field_kind_may_use_value_control_expression)
}

pub(in crate::driver::buildability) fn field_kind_may_use_value_control_expression(
    kind: TypecheckScalarViewKind,
) -> bool {
    match kind {
        TypecheckScalarViewKind::I32
        | TypecheckScalarViewKind::U8
        | TypecheckScalarViewKind::Usize
        | TypecheckScalarViewKind::Bool
        | TypecheckScalarViewKind::Str => true,
        TypecheckScalarViewKind::Slice(element) => {
            typecheck_slice_element_kind_is_buildable(element)
        }
    }
}

use super::*;

pub(super) fn fixed_array_literal_argument_has_fixed_array_parameter_type(
    call: &CallExpr,
    index: usize,
    argument: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(_) = unwrap_group_expr(argument) else {
        return false;
    };
    let Some(ty) = call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources).is_some()
}

pub(super) fn fixed_array_literal_argument_requires_partial_initialization_tracking(
    call: &CallExpr,
    index: usize,
    argument: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(literal) = unwrap_group_expr(argument) else {
        return false;
    };
    let Some(ty) = call_argument_parameter_type(
        call,
        index,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((element, length, _layout)) =
        fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
    else {
        return false;
    };
    u64::try_from(literal.elements.len()).ok() == Some(length)
        && fixed_array_literal_recursive_drop_element_type_is_buildable(
            &ty,
            &element,
            resolved,
            resolved_sources,
        )
        && literal
            .elements
            .iter()
            .any(|element| !expression_completes_without_source_control_exit(element))
}

pub(super) fn fixed_array_literal_struct_field_has_fixed_array_type(
    field: &StructLiteralField,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(_) = unwrap_group_expr(&field.value) else {
        return false;
    };
    let Some(ty) = field_type_expr_for_span(field.name_span, resolved, typecheck_facts) else {
        return false;
    };
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources).is_some()
}

pub(super) fn move_only_fixed_array_struct_field_requires_partial_initialization_tracking(
    field: &StructLiteralField,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if expression_completes_without_source_control_exit(&field.value) {
        return false;
    }
    let Some(ty) = field_type_expr_for_span(field.name_span, fallback_resolved, typecheck_facts)
    else {
        return false;
    };
    let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
    let resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_move_only_fixed_array_with_resolver(&ty, fallback_resolved, &resolver)
}

pub(super) fn fixed_array_literal_return_has_fixed_array_type(
    expression: &Expr,
    return_type: Option<&TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let Expr::ArrayLiteral(_) = unwrap_group_expr(expression) else {
        return false;
    };
    return_type
        .and_then(|ty| fixed_array_return_type_abi(ty, resolved, resolved_sources))
        .is_some()
}

pub(super) fn fixed_array_literal_for_type_has_fixed_array_type(
    expression: &Expr,
    ty: Option<&TypeExpr>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let Expr::ArrayLiteral(_) = unwrap_group_expr(expression) else {
        return false;
    };
    ty.and_then(|ty| fixed_array_type_abi_for_sources(ty, resolved, resolved_sources))
        .is_some()
}

pub(super) fn fixed_array_literal_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(literal) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    let Some(ty) =
        binding_type_expr_with_substitutions(statement, typecheck_facts, generic_substitutions)
    else {
        return false;
    };
    let Some((element, length, _layout)) =
        fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
    else {
        return false;
    };
    u64::try_from(literal.elements.len()).ok() == Some(length)
        && (fixed_array_element_abi_is_buildable(&element)
            || fixed_array_literal_recursive_drop_elements_are_buildable(
                literal,
                &ty,
                &element,
                resolved,
                resolved_sources,
            ))
}

fn fixed_array_literal_recursive_drop_elements_are_buildable(
    literal: &crate::ast::ArrayLiteralExpr,
    array_ty: &TypeExpr,
    element_abi: &AbiType,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    if !fixed_array_literal_recursive_drop_element_type_is_buildable(
        array_ty,
        element_abi,
        fallback_resolved,
        resolved_sources,
    ) {
        return false;
    }

    literal
        .elements
        .iter()
        .all(fixed_array_owned_element_initializer_is_buildable_with_tracking)
}

pub(super) fn fixed_array_literal_requires_partial_initialization_tracking(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(literal) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    let Some(ty) =
        binding_type_expr_with_substitutions(statement, typecheck_facts, generic_substitutions)
    else {
        return false;
    };
    let Some((element, length, _layout)) =
        fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
    else {
        return false;
    };
    if u64::try_from(literal.elements.len()).ok() != Some(length)
        || !fixed_array_literal_recursive_drop_element_type_is_buildable(
            &ty,
            &element,
            resolved,
            resolved_sources,
        )
    {
        return false;
    }

    literal
        .elements
        .iter()
        .any(|element| !expression_completes_without_source_control_exit(element))
}

fn fixed_array_literal_recursive_drop_element_type_is_buildable(
    array_ty: &TypeExpr,
    element_abi: &AbiType,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    if !matches!(element_abi, AbiType::Struct(_)) {
        return false;
    }
    let resolver = |source| resolved_sources.get(&source).copied();
    type_expr_is_supported_move_only_fixed_array_with_resolver(
        array_ty,
        fallback_resolved,
        &resolver,
    )
}

fn fixed_array_owned_element_initializer_is_buildable(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::StructLiteral(_) | Expr::Call(_) => {
            expression_completes_without_source_control_exit(expression)
        }
        Expr::Force(force) => {
            matches!(unwrap_group_expr(&force.expression), Expr::Call(_))
                && expression_completes_without_source_control_exit(&force.expression)
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            matches!(unwrap_group_expr(&unary.operand), Expr::Identifier(_))
        }
        _ => false,
    }
}

fn fixed_array_owned_element_initializer_is_buildable_with_tracking(expression: &Expr) -> bool {
    fixed_array_owned_element_initializer_is_buildable(expression)
        || matches!(
            unwrap_group_expr(expression),
            Expr::Propagate(propagation)
                if matches!(unwrap_group_expr(&propagation.expression), Expr::Call(_))
        )
}

fn expression_completes_without_source_control_exit(expression: &Expr) -> bool {
    match expression {
        Expr::Propagate(_)
        | Expr::Catch(_)
        | Expr::Otherwise(_)
        | Expr::If(_)
        | Expr::IfIs(_)
        | Expr::Match(_)
        | Expr::InterpolatedString(_) => false,
        Expr::ArrayLiteral(literal) => literal
            .elements
            .iter()
            .all(expression_completes_without_source_control_exit),
        Expr::StructLiteral(literal) => literal
            .fields
            .iter()
            .all(|field| expression_completes_without_source_control_exit(&field.value)),
        Expr::Force(force) => expression_completes_without_source_control_exit(&force.expression),
        Expr::Borrow(borrow) => {
            expression_completes_without_source_control_exit(&borrow.expression)
        }
        Expr::Unary(unary) => expression_completes_without_source_control_exit(&unary.operand),
        Expr::Binary(binary) => {
            expression_completes_without_source_control_exit(&binary.left)
                && expression_completes_without_source_control_exit(&binary.right)
        }
        Expr::TypeConversion(conversion) => {
            expression_completes_without_source_control_exit(&conversion.expression)
        }
        Expr::Call(call) => {
            expression_completes_without_source_control_exit(&call.callee)
                && call
                    .arguments
                    .iter()
                    .all(expression_completes_without_source_control_exit)
        }
        Expr::Member(member) => expression_completes_without_source_control_exit(&member.object),
        Expr::Index(index) => {
            expression_completes_without_source_control_exit(&index.object)
                && expression_completes_without_source_control_exit(&index.index)
        }
        Expr::Group(group) => expression_completes_without_source_control_exit(&group.expression),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => true,
    }
}

pub(super) fn fixed_array_copy_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::Identifier(identifier) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) = fixed_array_binding_type_abi(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    if !fixed_array_element_abi_is_buildable(&target_element) {
        return false;
    }

    let Some(source_ty) = local_identifier_type_expr_with_substitutions(
        identifier,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

pub(super) fn fixed_array_move_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::Unary(unary) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    if unary.operator != UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group_expr(&unary.operand) else {
        return false;
    };
    let Some(target_ty) =
        binding_type_expr_with_substitutions(statement, typecheck_facts, generic_substitutions)
    else {
        return false;
    };
    fixed_array_move_to_target_is_buildable(
        &target_ty,
        identifier,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn fixed_array_call_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(target_ty) =
        binding_type_expr_with_substitutions(statement, typecheck_facts, generic_substitutions)
    else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) =
        fixed_array_type_abi_for_sources(&target_ty, resolved, resolved_sources)
    else {
        return false;
    };

    let Some(source_ty) = fixed_array_binding_call_result_type_expr(
        &statement.initializer,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_supported_element(
        &target_ty,
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
        resolved,
        resolved_sources,
    )
}

pub(super) fn fixed_array_member_binding_is_buildable(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::Member(member) = unwrap_group_expr(&statement.initializer) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) = fixed_array_binding_type_abi(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };

    let Some(source_ty) = field_type_expr_for_member(member, resolved, typecheck_facts) else {
        return false;
    };
    let source_ty = substitute_type_expr_parameters(&source_ty, generic_substitutions);
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

pub(super) fn fixed_array_copy_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group_expr(&statement.value) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) = fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    if !fixed_array_element_abi_is_buildable(&target_element) {
        return false;
    }

    let Some(source_ty) = local_identifier_type_expr_with_substitutions(
        identifier,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

pub(super) fn fixed_array_move_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Unary(unary) = unwrap_group_expr(&statement.value) else {
        return false;
    };
    if unary.operator != UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group_expr(&unary.operand) else {
        return false;
    };
    let Some(target_ty) = fixed_array_assignment_target_type_expr(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    fixed_array_move_to_target_is_buildable(
        &target_ty,
        identifier,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

fn fixed_array_move_to_target_is_buildable(
    target_ty: &TypeExpr,
    source: &IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some((target_element, target_length, target_layout)) =
        fixed_array_type_abi_for_sources(target_ty, resolved, resolved_sources)
    else {
        return false;
    };
    if !fixed_array_literal_recursive_drop_element_type_is_buildable(
        target_ty,
        &target_element,
        resolved,
        resolved_sources,
    ) {
        return false;
    }

    let Some(source_ty) = local_identifier_type_expr_with_substitutions(
        source,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

pub(super) fn fixed_array_call_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Some(target_ty) = fixed_array_assignment_target_type_expr(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) =
        fixed_array_type_abi_for_sources(&target_ty, resolved, resolved_sources)
    else {
        return false;
    };

    let Some(source_ty) = fixed_array_call_result_type_expr(
        &statement.value,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_supported_element(
        &target_ty,
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
        resolved,
        resolved_sources,
    )
}

pub(super) fn fixed_array_otherwise_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    if !matches!(unwrap_group_expr(&statement.value), Expr::Otherwise(_)) {
        return false;
    }
    let Some(target_ty) = fixed_array_assignment_target_type_expr(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) =
        fixed_array_type_abi_for_sources(&target_ty, resolved, resolved_sources)
    else {
        return false;
    };

    let Some(source_ty) = fixed_array_binding_call_result_type_expr(
        &statement.value,
        resolved,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_supported_element(
        &target_ty,
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
        resolved,
        resolved_sources,
    )
}

pub(super) fn fixed_array_member_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Member(member) = unwrap_group_expr(&statement.value) else {
        return false;
    };
    let Some((target_element, target_length, target_layout)) = fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };

    let Some(source_ty) = field_type_expr_for_member(member, resolved, typecheck_facts) else {
        return false;
    };
    let source_ty = substitute_type_expr_parameters(&source_ty, generic_substitutions);
    let Some((source_element, source_length, source_layout)) =
        fixed_array_type_abi_for_sources(&source_ty, resolved, resolved_sources)
    else {
        return false;
    };

    fixed_array_abi_matches_buildable_element(
        &target_element,
        target_length,
        target_layout,
        &source_element,
        source_length,
        source_layout,
    )
}

pub(super) fn fixed_array_abi_matches_buildable_element(
    target_element: &AbiType,
    target_length: u64,
    target_layout: crate::abi::ValueLayout,
    source_element: &AbiType,
    source_length: u64,
    source_layout: crate::abi::ValueLayout,
) -> bool {
    fixed_array_abi_matches(
        target_element,
        target_length,
        target_layout,
        source_element,
        source_length,
        source_layout,
    ) && fixed_array_element_abi_is_buildable(source_element)
}

fn fixed_array_abi_matches(
    target_element: &AbiType,
    target_length: u64,
    target_layout: crate::abi::ValueLayout,
    source_element: &AbiType,
    source_length: u64,
    source_layout: crate::abi::ValueLayout,
) -> bool {
    target_element == source_element
        && target_length == source_length
        && target_layout == source_layout
}

fn fixed_array_abi_matches_supported_element(
    target_ty: &TypeExpr,
    target_element: &AbiType,
    target_length: u64,
    target_layout: crate::abi::ValueLayout,
    source_element: &AbiType,
    source_length: u64,
    source_layout: crate::abi::ValueLayout,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> bool {
    let resolver = |source| resolved_sources.get(&source).copied();
    fixed_array_abi_matches(
        target_element,
        target_length,
        target_layout,
        source_element,
        source_length,
        source_layout,
    ) && (fixed_array_element_abi_is_buildable(source_element)
        || type_expr_is_supported_move_only_fixed_array_with_resolver(
            target_ty,
            fallback_resolved,
            &resolver,
        ))
}

pub(super) fn fixed_array_binding_call_result_type_expr(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(expression) {
        Expr::Otherwise(otherwise) => {
            let Expr::Call(call) = unwrap_group_expr(&otherwise.value) else {
                return None;
            };
            fixed_array_inner_type_expr_from_optional_call(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        }
        _ => fixed_array_call_result_type_expr(
            expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
    }
}

pub(super) fn fixed_array_call_result_type_expr(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(expression) {
        Expr::Call(call) => call_return_type_expr_with_substitutions(
            call,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group_expr(&propagation.expression) else {
                return None;
            };
            fixed_array_success_type_expr_from_fallible_call(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group_expr(&force.expression) else {
                return None;
            };
            fixed_array_success_type_expr_from_fallible_call(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group_expr(&catch.expression) else {
                return None;
            };
            fixed_array_success_type_expr_from_fallible_call(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )
        }
        _ => None,
    }
}

pub(super) fn fixed_array_success_type_expr_from_fallible_call(
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    Some(*fallible.success)
}

pub(super) fn fixed_array_inner_type_expr_from_optional_call(
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let TypeExpr::Optional(optional) = return_type else {
        return None;
    };
    Some(*optional.inner)
}

pub(super) fn fixed_array_literal_assignment_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::ArrayLiteral(literal) = unwrap_group_expr(&statement.value) else {
        return false;
    };
    let Some((element, length, _layout)) = fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    u64::try_from(literal.elements.len()).ok() == Some(length)
        && (fixed_array_element_abi_is_buildable(&element)
            || fixed_array_literal_recursive_drop_assignment_is_buildable(
                statement,
                literal,
                &element,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ))
}

fn fixed_array_literal_recursive_drop_assignment_is_buildable(
    statement: &AssignmentStmt,
    literal: &crate::ast::ArrayLiteralExpr,
    element_abi: &AbiType,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some(target_ty) = fixed_array_assignment_target_type_expr(
        &statement.target,
        fallback_resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    fixed_array_literal_recursive_drop_element_type_is_buildable(
        &target_ty,
        element_abi,
        fallback_resolved,
        resolved_sources,
    ) && literal
        .elements
        .iter()
        .all(fixed_array_owned_element_initializer_is_buildable_with_tracking)
}

fn fixed_array_literal_assignment_requires_partial_initialization_tracking(
    statement: &AssignmentStmt,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Expr::ArrayLiteral(literal) = unwrap_group_expr(&statement.value) else {
        return false;
    };
    let Some(target_ty) = fixed_array_assignment_target_type_expr(
        &statement.target,
        fallback_resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) else {
        return false;
    };
    let Some((element, length, _layout)) =
        fixed_array_type_abi_for_sources(&target_ty, fallback_resolved, resolved_sources)
    else {
        return false;
    };
    u64::try_from(literal.elements.len()).ok() == Some(length)
        && fixed_array_literal_recursive_drop_element_type_is_buildable(
            &target_ty,
            &element,
            fallback_resolved,
            resolved_sources,
        )
        && literal
            .elements
            .iter()
            .any(|element| !expression_completes_without_source_control_exit(element))
}

pub(super) fn unsupported_fixed_array_assignment_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<Diagnostic> {
    if statement.operator != AssignmentOperator::Assign {
        return None;
    }
    fixed_array_assignment_target_abi(
        &statement.target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )?;

    if fixed_array_literal_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_copy_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_move_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_call_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_otherwise_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) || fixed_array_member_assignment_is_buildable(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return None;
    }

    if fixed_array_literal_assignment_requires_partial_initialization_tracking(
        statement,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    ) {
        return Some(unsupported_v0_build_diagnostic(
            sources,
            statement.value.span(),
            "fixed array literal assignments whose element initialization can exit early",
            "initialize every recursively dropped element without `?`, `catch`, `otherwise`, or value control flow until per-element initialization state is tracked",
        ));
    }

    Some(match unwrap_group_expr(&statement.value) {
        Expr::ArrayLiteral(_) => unsupported_v0_build_diagnostic(
            sources,
            statement.value.span(),
            "fixed array assignments outside supported literal values",
            "match the target fixed array length and use `i32`, `u8`, `usize`, `bool`, or `&str` elements until broader fixed array element storage is promoted",
        ),
        _ => unsupported_v0_build_diagnostic(
            sources,
            statement.target.span(),
            "fixed array assignments outside supported replacement values",
            "assign a matching fixed array literal, copy another matching local or aggregate-field fixed array, or assign a matching fixed array call result until broader fixed array expression lowering is promoted",
        ),
    })
}

pub(super) fn fixed_array_assignment_target_abi(
    target: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<(AbiType, u64, crate::abi::ValueLayout)> {
    let ty = fixed_array_assignment_target_type_expr(
        target,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )?;
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
}

pub(super) fn fixed_array_assignment_target_type_expr(
    target: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    let ty = assignment_target_type_expr(target, resolved, typecheck_facts, generic_substitutions)?;
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)?;
    Some(ty)
}

pub(super) fn fixed_array_binding_type_abi(
    statement: &BindingStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<(AbiType, u64, crate::abi::ValueLayout)> {
    let ty =
        binding_type_expr_with_substitutions(statement, typecheck_facts, generic_substitutions)?;
    fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)
}

pub(super) fn fixed_array_return_type_abi(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<(AbiType, u64, crate::abi::ValueLayout)> {
    match ty {
        TypeExpr::Fallible(fallible) => {
            fixed_array_return_type_abi(&fallible.success, resolved, resolved_sources)
        }
        TypeExpr::Optional(optional) => {
            fixed_array_return_type_abi(&optional.inner, resolved, resolved_sources)
        }
        _ => fixed_array_type_abi_for_sources(ty, resolved, resolved_sources),
    }
}

pub(super) fn fixed_array_type_abi_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<(AbiType, u64, crate::abi::ValueLayout)> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    let value =
        abi_value_from_type_expr_with_resolver(ty, fallback_resolved, source_resolver).ok()?;
    let layout = value.layout;
    match value.ty {
        AbiType::Array { element, length } => Some((*element, length, layout)),
        _ => None,
    }
}

pub(super) fn fixed_array_element_abi_is_buildable(element: &AbiType) -> bool {
    matches!(
        element,
        AbiType::I32 | AbiType::U8 | AbiType::Usize | AbiType::Bool | AbiType::StrView
    )
}

pub(super) fn fixed_array_index_compound_assignment_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    let Some((element, layout)) = fixed_array_index_target_abi(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    ) else {
        return false;
    };
    layout.size > 0 && matches!(element, AbiType::I32 | AbiType::U8 | AbiType::Usize)
}

pub(super) fn fixed_array_index_assignment_target_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    let (element, layout) = fixed_array_index_target_abi(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    )?;
    Some(layout.size > 0 && fixed_array_element_abi_is_buildable(&element))
}

pub(super) fn collect_fixed_array_literal_binding_diagnostics(
    statement: &BindingStmt,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_fixed_array_literal_elements_diagnostics(
        unwrap_group_expr(&statement.initializer),
        sources,
        resolved,
        typecheck_facts,
        generic_substitutions,
        root_source,
        names,
        resolved_sources,
        nocter_home,
        queue,
        diagnostics,
    );
}

pub(super) fn collect_fixed_array_literal_elements_diagnostics(
    expression: &Expr,
    sources: &SourceMap,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
    resolved_sources: &ResolvedSources<'_>,
    nocter_home: Option<&Path>,
    queue: &mut VecDeque<CallTarget>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::ArrayLiteral(literal) = expression else {
        collect_expression_diagnostics(
            expression,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
        return;
    };

    for element in &literal.elements {
        collect_value_expression_diagnostics(
            element,
            None,
            sources,
            resolved,
            typecheck_facts,
            generic_substitutions,
            root_source,
            names,
            resolved_sources,
            nocter_home,
            queue,
            diagnostics,
        );
    }
}

pub(super) fn fixed_array_index_expression_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<bool> {
    let (element, layout) = fixed_array_index_target_abi(
        expression,
        resolved,
        typecheck_facts,
        generic_substitutions,
        resolved_sources,
    )?;
    Some(layout.size > 0 && fixed_array_element_abi_is_buildable(&element))
}

pub(super) fn fixed_array_index_target_abi(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<(AbiType, crate::abi::ValueLayout)> {
    let ty = fixed_array_index_target_type_expr(
        &expression.object,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let (element, _length, layout) =
        fixed_array_type_abi_for_sources(&ty, resolved, resolved_sources)?;
    Some((element, layout))
}

pub(super) fn fixed_array_index_target_type_expr(
    expression: &Expr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            typecheck_facts
                .binding_type_expr(symbol.name_span)
                .cloned()
                .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions))
        }
        Expr::Member(member) => field_type_expr_for_member(member, resolved, typecheck_facts)
            .map(|ty| substitute_type_expr_parameters(&ty, generic_substitutions)),
        Expr::Group(group) => fixed_array_index_target_type_expr(
            &group.expression,
            resolved,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

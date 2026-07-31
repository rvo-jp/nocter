use super::*;

pub(in crate::driver::buildability) fn range_for_binding_type_is_buildable(
    statement: &ForRangeStmt,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    matches!(
        typecheck_facts.binding_scalar_view_kind(statement.name_span),
        Some(TypecheckScalarViewKind::I32 | TypecheckScalarViewKind::Usize)
    )
}

pub(in crate::driver::buildability) fn assignment_operator_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    if statement.operator == AssignmentOperator::Assign {
        return true;
    }
    match unwrap_group_expr(&statement.target) {
        Expr::Identifier(identifier) => {
            let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
                return false;
            };
            matches!(
                typecheck_facts.binding_scalar_view_kind(symbol.name_span),
                Some(
                    TypecheckScalarViewKind::I32
                        | TypecheckScalarViewKind::Usize
                        | TypecheckScalarViewKind::U8
                )
            )
        }
        Expr::Member(member) => {
            aggregate_field_compound_assignment_is_buildable(member.member_span, typecheck_facts)
        }
        Expr::Index(index) => {
            fixed_array_index_compound_assignment_is_buildable(
                index,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            ) || slice_index_compound_assignment_is_buildable(
                &index.object,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
            )
        }
        _ => false,
    }
}

pub(in crate::driver::buildability) fn slice_index_compound_assignment_is_buildable(
    object: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    matches!(
        slice_index_assignment_element_kind(
            object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions
        ),
        Some(
            TypecheckSliceElementKind::I32
                | TypecheckSliceElementKind::U8
                | TypecheckSliceElementKind::Usize,
        )
    )
}

pub(in crate::driver::buildability) fn aggregate_field_compound_assignment_is_buildable(
    member_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    matches!(
        typecheck_facts.field_scalar_view_kind(member_span),
        Some(
            TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize,
        )
    )
}

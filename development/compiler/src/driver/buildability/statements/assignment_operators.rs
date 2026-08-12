use super::*;

pub(in crate::driver::buildability) fn range_for_binding_type_is_buildable(
    statement: &ForRangeStmt,
    typed_hir: &TypedHir,
) -> bool {
    matches!(
        typed_hir.binding_scalar_view_kind(statement.name_span),
        Some(TypecheckScalarViewKind::I32 | TypecheckScalarViewKind::Usize)
    )
}

pub(in crate::driver::buildability) fn assignment_operator_is_buildable(
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typed_hir: &TypedHir,
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
                typed_hir.binding_scalar_view_kind(symbol.name_span),
                Some(
                    TypecheckScalarViewKind::I32
                        | TypecheckScalarViewKind::Usize
                        | TypecheckScalarViewKind::U8
                )
            )
        }
        Expr::Member(member) => {
            aggregate_field_compound_assignment_is_buildable(member.member_span, typed_hir)
        }
        Expr::Index(index) => {
            fixed_array_index_compound_assignment_is_buildable(
                index,
                resolved,
                resolved_sources,
                typed_hir,
                generic_substitutions,
            ) || slice_index_compound_assignment_is_buildable(
                &index.object,
                resolved,
                resolved_sources,
                typed_hir,
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
    typed_hir: &TypedHir,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    matches!(
        slice_index_assignment_element_kind(
            object,
            resolved,
            resolved_sources,
            typed_hir,
            generic_substitutions
        ),
        Some(
            TypecheckSliceElementKind::I32
                | TypecheckSliceElementKind::U8
                | TypecheckSliceElementKind::Usize
                | TypecheckSliceElementKind::Integer(_),
        )
    )
}

pub(in crate::driver::buildability) fn aggregate_field_compound_assignment_is_buildable(
    member_span: ByteSpan,
    typed_hir: &TypedHir,
) -> bool {
    matches!(
        typed_hir.field_scalar_view_kind(member_span),
        Some(
            TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize,
        )
    )
}

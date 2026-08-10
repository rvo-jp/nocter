use super::*;

fn assignment_target_root_name(expression: &Expr) -> Option<&str> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some(&identifier.name),
        Expr::Member(member) => assignment_target_root_name(&member.object),
        _ => None,
    }
}

pub(super) fn nonterminal_assignment_target_allowed(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
    local_mark: usize,
) -> bool {
    assignment_target_root_name(&statement.target)
        .is_some_and(|target_name| context.local_defined_since(target_name, local_mark))
        || assignment_targets_whole_scalar_or_view_local(statement, context)
        || compound_assignment_targets_scalar_integer_local(statement, context)
        || assignment_targets_whole_aggregate_local(statement, context)
        || assignment_targets_whole_outcome_local(statement, context)
        || assignment_targets_readwrite_aggregate_field(statement, context)
        || assignment_targets_direct_slice_index(statement, context)
}

fn assignment_targets_whole_scalar_or_view_local(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&statement.target) else {
        return false;
    };
    matches!(
        context.i32_location(&identifier.name),
        Some(I32Location::Local(_))
    ) || matches!(
        context.u8_location(&identifier.name),
        Some(U8Location::Local(_))
    ) || matches!(
        context.usize_location(&identifier.name),
        Some(UsizeLocation::Local(_))
    ) || matches!(
        context.bool_location(&identifier.name),
        Some(BoolLocation::Local(_))
    ) || matches!(
        context.str_location(&identifier.name),
        Some(StrLocation::Local(_))
    ) || matches!(
        context.slice_location(&identifier.name),
        Some(SliceLocation::Local(_))
    )
}

fn compound_assignment_targets_scalar_integer_local(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    if statement.operator == AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&statement.target) else {
        return false;
    };
    matches!(
        context.i32_location(&identifier.name),
        Some(I32Location::Local(_))
    ) || matches!(
        context.usize_location(&identifier.name),
        Some(UsizeLocation::Local(_))
    )
}

fn assignment_targets_whole_aggregate_local(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&statement.target) else {
        return false;
    };
    context.aggregate_local(&identifier.name).is_some()
}

fn assignment_targets_whole_outcome_local(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&statement.target) else {
        return false;
    };
    context.outcome_local(&identifier.name).is_some()
}

pub(super) fn outer_aggregate_assignment_before_function_exit_allowed(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
    local_mark: usize,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
) -> bool {
    if !statement_suffix_exits_function(statements, index, result, context) {
        return false;
    }
    let Some(target_name) = assignment_target_root_name(&statement.target) else {
        return false;
    };
    context.aggregate_local(target_name).is_some()
        && !context.aggregate_local_defined_since(target_name, local_mark)
}

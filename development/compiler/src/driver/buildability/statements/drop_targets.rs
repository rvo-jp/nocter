use super::*;

pub(in crate::driver::buildability) fn enqueue_member_replacement_drop_target(
    statement: &AssignmentStmt,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    queue: &mut VecDeque<CallTarget>,
) {
    if statement.operator != AssignmentOperator::Assign {
        return;
    }
    let Expr::Member(member) = unwrap_group_expr(&statement.target) else {
        return;
    };
    let Some(specialization) = typecheck_facts.field_drop_type_specialization(member.member_span)
    else {
        return;
    };
    let Some(specialization) = specialization.with_context_substitutions(generic_substitutions)
    else {
        return;
    };
    queue.push_back(call_target_for_source(
        specialization.declaration_span.source,
        root_source,
        specialization.target_name,
    ));
}

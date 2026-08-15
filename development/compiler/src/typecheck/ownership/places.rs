use super::*;

pub(super) fn whole_identifier(expression: &Expr) -> Option<&IdentifierExpr> {
    match expression {
        Expr::Identifier(identifier) => Some(identifier),
        Expr::Group(group) => whole_identifier(&group.expression),
        _ => None,
    }
}

pub(super) fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

pub(super) fn assignment_target_place(
    expression: &Expr,
    resolved: &ResolveOutput,
) -> Option<BorrowPlace> {
    expression_place(expression, resolved)
}

pub(super) fn expression_place(expression: &Expr, resolved: &ResolveOutput) -> Option<BorrowPlace> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some(BorrowPlace::whole(
            resolved.local_symbol_id_for_reference_span(identifier.span)?,
            identifier.name.clone(),
        )),
        Expr::Member(member) => member_expression_place(member, resolved),
        Expr::Index(index) => index_expression_place(index, resolved),
        _ => None,
    }
}

pub(super) fn reference_place(
    name: &str,
    span: crate::source::ByteSpan,
    resolved: &ResolveOutput,
) -> Option<BorrowPlace> {
    Some(BorrowPlace::whole(
        resolved.local_symbol_id_for_reference_span(span)?,
        name.to_string(),
    ))
}

pub(super) fn member_expression_place(
    member: &crate::ast::MemberExpr,
    resolved: &ResolveOutput,
) -> Option<BorrowPlace> {
    let mut place = expression_place(&member.object, resolved)?;
    place.push_field(member.member.clone());
    Some(place)
}

pub(super) fn index_expression_place(
    index: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
) -> Option<BorrowPlace> {
    let mut place = expression_place(&index.object, resolved)?;
    place.mark_unknown();
    Some(place)
}

pub(super) fn expression_place_has_only_named_fields(
    expression: &Expr,
    resolved: &ResolveOutput,
) -> bool {
    expression_place(expression, resolved).is_some_and(|place| place.fields.is_some())
}

pub(super) fn owned_method_receiver_identifier<'a>(
    call: &'a crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<&'a IdentifierExpr> {
    let method = method_member_for_call(call)?;
    let (_, signature) = resolved_method_for_call(resolved, call, environment)?;
    if signature.receiver.mode != MethodReceiverMode::Owned {
        return None;
    }

    let Expr::Identifier(identifier) = method.object.as_ref() else {
        return None;
    };
    let receiver_type = expression_type(&method.object, resolved, environment);
    non_copy_struct_type_name(&receiver_type, resolved)?;
    Some(identifier)
}

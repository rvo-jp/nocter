use super::*;

pub(super) fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

pub(super) fn call_callee_name_span(call: &CallExpr) -> Option<ByteSpan> {
    match call.callee.without_groups() {
        Expr::Identifier(identifier) => Some(identifier.span),
        Expr::Member(member) => Some(member.member_span),
        _ => None,
    }
}

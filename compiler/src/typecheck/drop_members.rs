use super::diagnostics::drop_binding_type_unsupported_diagnostic;
use crate::ast::{AstFile, ImplMember, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::source::SourceMap;

pub(super) fn check_drop_members(
    sources: &SourceMap,
    ast: &AstFile,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        let Item::Impl(impl_) = item else {
            continue;
        };
        for member in &impl_.members {
            let ImplMember::Drop(drop_) = member else {
                continue;
            };
            if !drop_binding_is_readwrite_self_borrow(&drop_.binding.ty) {
                diagnostics.push(drop_binding_type_unsupported_diagnostic(sources, drop_));
            }
        }
    }
}

fn drop_binding_is_readwrite_self_borrow(ty: &TypeExpr) -> bool {
    matches!(
        ty,
        TypeExpr::Borrow(borrow)
            if borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "Self")
    )
}

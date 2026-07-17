use super::diagnostics::{
    copy_struct_drop_member_diagnostic, drop_binding_type_unsupported_diagnostic,
};
use super::model::Type;
use super::type_expr::type_expr_to_type;
use crate::ast::{AstFile, ImplMember, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::source::SourceMap;

pub(super) fn check_drop_members(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        let Item::Impl(impl_) = item else {
            continue;
        };
        let impl_target_type = type_expr_to_type(&impl_.target_ty, resolved);
        let copy_struct_name = copy_struct_name(&impl_target_type, resolved);
        for member in &impl_.members {
            let ImplMember::Drop(drop_) = member else {
                continue;
            };
            if let Some(struct_name) = copy_struct_name {
                diagnostics.push(copy_struct_drop_member_diagnostic(
                    sources,
                    struct_name,
                    &impl_.target_ty,
                    drop_,
                ));
            }
            if !drop_binding_is_readwrite_self_borrow(&drop_.binding.ty) {
                diagnostics.push(drop_binding_type_unsupported_diagnostic(sources, drop_));
            }
        }
    }
}

fn copy_struct_name<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a str> {
    let Type::Named(name) = ty else {
        return None;
    };
    resolved
        .type_symbol_by_canonical_name(name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Struct && symbol.is_copy)
        .map(|symbol| symbol.canonical_name.as_str())
}

fn drop_binding_is_readwrite_self_borrow(ty: &TypeExpr) -> bool {
    matches!(
        ty,
        TypeExpr::Borrow(borrow)
            if borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "Self")
    )
}

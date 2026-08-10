use super::diagnostics::{
    conditional_drop_pattern_diagnostic, copy_struct_drop_member_diagnostic,
    drop_binding_type_unsupported_diagnostic,
};
use super::model::Type;
use super::type_expr::type_expr_to_type;
use crate::ast::{AstFile, InstanceMember, Item, TypeExpr};
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
        let Item::Instance(instance) = item else {
            continue;
        };
        let owner_target_type = type_expr_to_type(&instance.target_ty, resolved);
        let copy_struct_name = copy_struct_name(&owner_target_type, resolved);
        for member in &instance.members {
            let InstanceMember::Drop(drop_) = member else {
                continue;
            };
            if let Some(struct_name) = copy_struct_name {
                diagnostics.push(copy_struct_drop_member_diagnostic(
                    sources,
                    struct_name,
                    &instance.target_ty,
                    drop_,
                ));
            }
            if !drop_binding_is_readwrite_self_borrow(&drop_.binding.ty) {
                diagnostics.push(drop_binding_type_unsupported_diagnostic(sources, drop_));
            }
            if !drop_pattern_is_uniform(instance) {
                diagnostics.push(conditional_drop_pattern_diagnostic(
                    sources, instance, drop_,
                ));
            }
        }
    }
}

fn drop_pattern_is_uniform(instance: &crate::ast::InstanceDecl) -> bool {
    if instance
        .requirements
        .as_ref()
        .is_some_and(|clause| !clause.predicates.is_empty())
    {
        return false;
    }
    let slots: Vec<&crate::ast::TypeExpr> = match &instance.target_ty {
        crate::ast::TypeExpr::Reference(_) => Vec::new(),
        crate::ast::TypeExpr::Generic(generic) => generic.arguments.iter().collect(),
        crate::ast::TypeExpr::View(view) if !view.is_readwrite => vec![&view.element],
        _ => return false,
    };
    let mut seen = std::collections::HashSet::new();
    slots.len() == instance.generics.parameters.len()
        && slots.into_iter().all(|slot| {
            matches!(slot, crate::ast::TypeExpr::Reference(reference) if seen.insert(reference.name.as_str()))
        })
}

fn copy_struct_name<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a str> {
    let name = ty.nominal_name()?;
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

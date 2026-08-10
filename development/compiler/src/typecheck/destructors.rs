use super::diagnostics::{
    copy_struct_destruct_diagnostic, destruct_binding_type_unsupported_diagnostic,
    invalid_destruct_target_diagnostic, non_uniform_destruct_pattern_diagnostic,
};
use super::model::Type;
use super::type_expr::type_expr_to_type;
use crate::ast::{AstFile, DestructDecl, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::source::SourceMap;

pub(super) fn check_destructors(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        let Item::Destruct(destruct) = item else {
            continue;
        };
        let owner_target_type = type_expr_to_type(&destruct.target_ty, resolved);
        let invalid_target = target_is_not_struct_or_enum(destruct, &owner_target_type, resolved);
        if invalid_target {
            diagnostics.push(invalid_destruct_target_diagnostic(sources, destruct));
        }
        if let Some(struct_name) = copy_struct_name(&owner_target_type, resolved) {
            diagnostics.push(copy_struct_destruct_diagnostic(
                sources,
                struct_name,
                destruct,
            ));
        }
        if !binding_is_readwrite_self_borrow(&destruct.binding.ty) {
            diagnostics.push(destruct_binding_type_unsupported_diagnostic(
                sources, destruct,
            ));
        }
        if !invalid_target && !pattern_is_uniform(destruct) {
            diagnostics.push(non_uniform_destruct_pattern_diagnostic(sources, destruct));
        }
    }
}

fn target_is_not_struct_or_enum(
    destruct: &DestructDecl,
    ty: &Type,
    resolved: &ResolveOutput,
) -> bool {
    if !matches!(
        destruct.target_ty,
        TypeExpr::Reference(_) | TypeExpr::Generic(_)
    ) {
        return true;
    }
    if ty.is_unknown_or_unresolved() {
        return false;
    }
    let Some(name) = ty.nominal_name() else {
        return true;
    };
    resolved
        .type_symbol_by_canonical_name(name)
        .is_some_and(|symbol| !matches!(symbol.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum))
}

fn pattern_is_uniform(destruct: &DestructDecl) -> bool {
    let slots: Vec<&TypeExpr> = match &destruct.target_ty {
        TypeExpr::Reference(_) => Vec::new(),
        TypeExpr::Generic(generic) => generic.arguments.iter().collect(),
        _ => return false,
    };
    let mut seen = std::collections::HashSet::new();
    slots.len() == destruct.generics.parameters.len()
        && slots.into_iter().all(|slot| {
            matches!(slot, TypeExpr::Reference(reference) if seen.insert(reference.name.as_str()))
        })
}

fn copy_struct_name<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a str> {
    let name = ty.nominal_name()?;
    resolved
        .type_symbol_by_canonical_name(name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Struct && symbol.is_copy)
        .map(|symbol| symbol.canonical_name.as_str())
}

fn binding_is_readwrite_self_borrow(ty: &TypeExpr) -> bool {
    matches!(
        ty,
        TypeExpr::Borrow(borrow)
            if borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "Self")
    )
}

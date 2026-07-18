use super::diagnostics::{
    duplicate_interface_impl_diagnostic, interface_impl_contract_not_interface_diagnostic,
    interface_impl_target_not_nominal_diagnostic, interface_method_missing_diagnostic,
    interface_method_not_public_diagnostic, interface_method_signature_mismatch_diagnostic,
};
use super::model::Type;
use super::type_expr::type_expr_to_type_with_self_type;
use crate::ast::{AstFile, ImplDecl, Item, TypeExpr, Visibility};
use crate::diagnostics::Diagnostic;
use crate::resolve::{MethodSignature, ResolveOutput, TypeSymbol, TypeSymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_interface_impls(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashMap::<(String, String), ByteSpan>::new();

    for impl_ in ast.items.iter().filter_map(|item| match item {
        Item::Impl(impl_) if impl_.interface_ty.is_some() => Some(impl_),
        _ => None,
    }) {
        check_interface_impl(sources, impl_, resolved, diagnostics, &mut seen);
    }
}

fn check_interface_impl(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    seen: &mut HashMap<(String, String), ByteSpan>,
) {
    let Some(interface_ty) = &impl_.interface_ty else {
        return;
    };

    let Some(interface_symbol) =
        resolve_interface_symbol(interface_ty, resolved, diagnostics, sources)
    else {
        return;
    };
    let Some(target_symbol) = resolve_target_symbol(impl_, resolved, diagnostics, sources) else {
        return;
    };

    let key = (
        interface_symbol.canonical_name.clone(),
        target_symbol.canonical_name.clone(),
    );
    match seen.entry(key) {
        std::collections::hash_map::Entry::Occupied(first) => {
            diagnostics.push(duplicate_interface_impl_diagnostic(
                sources,
                impl_,
                interface_symbol,
                target_symbol,
                *first.get(),
            ));
            return;
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(impl_.span);
        }
    }

    let self_type = Type::Named(target_symbol.canonical_name.clone());
    for required in &interface_symbol.methods {
        let Some(actual) = target_symbol
            .methods
            .iter()
            .find(|method| method.name == required.name)
        else {
            diagnostics.push(interface_method_missing_diagnostic(
                sources,
                impl_,
                interface_symbol,
                target_symbol,
                required,
            ));
            continue;
        };

        if actual.visibility != Visibility::Public {
            diagnostics.push(interface_method_not_public_diagnostic(
                sources,
                interface_symbol,
                target_symbol,
                required,
                actual,
            ));
            continue;
        }

        let expected = method_shape(required, resolved, &self_type);
        let found = method_shape(actual, resolved, &self_type);
        if expected.has_unknown_or_unresolved() || found.has_unknown_or_unresolved() {
            continue;
        }
        if expected != found {
            diagnostics.push(interface_method_signature_mismatch_diagnostic(
                sources,
                interface_symbol,
                target_symbol,
                required,
                actual,
                method_shape_label(required, resolved, &self_type),
                method_shape_label(actual, resolved, &self_type),
            ));
        }
    }
}

fn resolve_interface_symbol<'a>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
) -> Option<&'a TypeSymbol> {
    let actual_type = type_expr_to_type_with_self_type(ty, resolved, None);
    if actual_type.is_unknown_or_unresolved() {
        return None;
    }
    let symbol = symbol_for_type(&actual_type, resolved);
    if symbol.is_none_or(|symbol| symbol.kind != TypeSymbolKind::Interface) {
        diagnostics.push(interface_impl_contract_not_interface_diagnostic(
            sources,
            ty,
            &actual_type,
            symbol,
        ));
        return None;
    }

    symbol
}

fn resolve_target_symbol<'a>(
    impl_: &ImplDecl,
    resolved: &'a ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
) -> Option<&'a TypeSymbol> {
    let actual_type = type_expr_to_type_with_self_type(&impl_.target_ty, resolved, None);
    if actual_type.is_unknown_or_unresolved() {
        return None;
    }
    let symbol = symbol_for_type(&actual_type, resolved);
    if symbol
        .is_none_or(|symbol| !matches!(symbol.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum))
    {
        diagnostics.push(interface_impl_target_not_nominal_diagnostic(
            sources,
            &impl_.target_ty,
            &actual_type,
            symbol,
        ));
        return None;
    }

    symbol
}

fn symbol_for_type<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a TypeSymbol> {
    let canonical_name = ty.nominal_name()?;

    resolved.type_symbol_by_canonical_name(canonical_name)
}

fn method_shape(
    method: &MethodSignature,
    resolved: &ResolveOutput,
    self_type: &Type,
) -> MethodShape {
    MethodShape {
        receiver: type_expr_to_type_with_self_type(&method.receiver.ty, resolved, Some(self_type)),
        parameters: method
            .signature
            .parameters
            .iter()
            .map(|parameter| {
                type_expr_to_type_with_self_type(&parameter.ty, resolved, Some(self_type))
            })
            .collect(),
        return_type: type_expr_to_type_with_self_type(
            &method.signature.return_type,
            resolved,
            Some(self_type),
        ),
    }
}

fn method_shape_label(
    method: &MethodSignature,
    resolved: &ResolveOutput,
    self_type: &Type,
) -> String {
    let shape = method_shape(method, resolved, self_type);
    let parameters = shape
        .parameters
        .iter()
        .map(Type::display)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "method (self: {}).{}({}): {}",
        shape.receiver.display(),
        method.name,
        parameters,
        shape.return_type.display()
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MethodShape {
    receiver: Type,
    parameters: Vec<Type>,
    return_type: Type,
}

impl MethodShape {
    fn has_unknown_or_unresolved(&self) -> bool {
        self.receiver.is_unknown_or_unresolved()
            || self.parameters.iter().any(Type::is_unknown_or_unresolved)
            || self.return_type.is_unknown_or_unresolved()
    }
}

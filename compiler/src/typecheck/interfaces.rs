use super::calls::method_applies_to_receiver;
use super::diagnostics::{
    duplicate_interface_impl_diagnostic, interface_impl_contract_not_interface_diagnostic,
    interface_impl_target_not_nominal_diagnostic, interface_method_missing_diagnostic,
    interface_method_not_public_diagnostic, interface_method_signature_mismatch_diagnostic,
};
use super::environments::generic_parameter_substitutions;
use super::model::Type;
use super::type_expr::{infer_type_expr_substitutions, type_expr_to_type_with_substitutions};
use crate::ast::{AstFile, ImplDecl, Item, TypeExpr, Visibility};
use crate::diagnostics::Diagnostic;
use crate::resolve::{MethodSignature, ResolveOutput, TypeSymbol, TypeSymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::{HashMap, HashSet};

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

    let impl_substitutions = generic_parameter_substitutions(&impl_.generics);
    let Some((interface_symbol, interface_type)) = resolve_interface_symbol(
        interface_ty,
        resolved,
        diagnostics,
        sources,
        &impl_substitutions,
    ) else {
        return;
    };
    let Some((target_symbol, target_type)) =
        resolve_target_symbol(impl_, resolved, diagnostics, sources, &impl_substitutions)
    else {
        return;
    };

    let key = conformance_key(&interface_type, &target_type);
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

    let self_type = target_type;
    let interface_substitutions =
        type_symbol_generic_substitutions(interface_symbol, &interface_type);
    for required in &interface_symbol.methods {
        let Some(actual) = target_symbol.methods.iter().find(|method| {
            method.name == required.name && method_applies_to_receiver(method, &self_type, resolved)
        }) else {
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

        let actual_substitutions = method_impl_target_substitutions(actual, &self_type, resolved);
        let expected = method_shape(required, resolved, &self_type, &interface_substitutions);
        let found = method_shape(actual, resolved, &self_type, &actual_substitutions);
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
                method_shape_label(required, resolved, &self_type, &interface_substitutions),
                method_shape_label(actual, resolved, &self_type, &actual_substitutions),
            ));
        }
    }
}

fn resolve_interface_symbol<'a>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    substitutions: &HashMap<String, Type>,
) -> Option<(&'a TypeSymbol, Type)> {
    let actual_type = type_expr_to_type_with_substitutions(ty, resolved, None, substitutions);
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

    symbol.map(|symbol| (symbol, actual_type))
}

fn resolve_target_symbol<'a>(
    impl_: &ImplDecl,
    resolved: &'a ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    substitutions: &HashMap<String, Type>,
) -> Option<(&'a TypeSymbol, Type)> {
    let actual_type =
        type_expr_to_type_with_substitutions(&impl_.target_ty, resolved, None, substitutions);
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

    symbol.map(|symbol| (symbol, actual_type))
}

fn symbol_for_type<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a TypeSymbol> {
    let canonical_name = ty.nominal_name()?;

    resolved.type_symbol_by_canonical_name(canonical_name)
}

fn conformance_key(interface_type: &Type, target_type: &Type) -> (String, String) {
    let mut parameters = HashMap::new();
    (
        conformance_key_type(interface_type, &mut parameters),
        conformance_key_type(target_type, &mut parameters),
    )
}

fn conformance_key_type(ty: &Type, parameters: &mut HashMap<String, usize>) -> String {
    match ty {
        Type::I32 => "i32".to_string(),
        Type::Primitive(name) => name.clone(),
        Type::StrData => "str".to_string(),
        Type::Str => "&str".to_string(),
        Type::Error => "error".to_string(),
        Type::Void => "void".to_string(),
        Type::Never => "never".to_string(),
        Type::None => "none".to_string(),
        Type::ArrayData { element } => {
            format!("[{}]", conformance_key_type(element, parameters))
        }
        Type::View {
            is_readwrite,
            element,
        } => format!(
            "&{}[{}]",
            if *is_readwrite { "+" } else { "" },
            conformance_key_type(element, parameters)
        ),
        Type::Array { element, length } => {
            format!("[{}; {length}]", conformance_key_type(element, parameters))
        }
        Type::Pointer(inner) => format!("*{}", conformance_key_type(inner, parameters)),
        Type::Optional(inner) => format!("{}?", conformance_key_type(inner, parameters)),
        Type::Fallible { success, error } => format!(
            "{}!{}",
            conformance_key_type(success, parameters),
            conformance_key_type(error, parameters)
        ),
        Type::Named(name) => name.clone(),
        Type::Generic { name, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| conformance_key_type(argument, parameters))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}<{arguments}>")
        }
        Type::Parameter(name) => {
            let next = parameters.len();
            let index = *parameters.entry(name.clone()).or_insert(next);
            format!("${index}")
        }
        Type::Unresolved(name) => name.clone(),
        Type::Unknown => "<unknown>".to_string(),
    }
}

fn type_symbol_generic_substitutions(symbol: &TypeSymbol, ty: &Type) -> HashMap<String, Type> {
    let Type::Generic { name, arguments } = ty else {
        return HashMap::new();
    };
    if name != &symbol.canonical_name {
        return HashMap::new();
    }

    symbol
        .generic_parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect()
}

fn method_impl_target_substitutions(
    method: &MethodSignature,
    self_type: &Type,
    resolved: &ResolveOutput,
) -> HashMap<String, Type> {
    let Some(impl_target_ty) = &method.impl_target_ty else {
        return HashMap::new();
    };

    let parameters = method
        .signature
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut substitutions = HashMap::new();
    infer_type_expr_substitutions(
        impl_target_ty,
        self_type,
        resolved,
        None,
        &parameters,
        &mut substitutions,
    );
    substitutions
}

fn method_shape(
    method: &MethodSignature,
    resolved: &ResolveOutput,
    self_type: &Type,
    substitutions: &HashMap<String, Type>,
) -> MethodShape {
    MethodShape {
        receiver: type_expr_to_type_with_substitutions(
            &method.receiver.ty,
            resolved,
            Some(self_type),
            substitutions,
        ),
        parameters: method
            .signature
            .parameters
            .iter()
            .map(|parameter| {
                type_expr_to_type_with_substitutions(
                    &parameter.ty,
                    resolved,
                    Some(self_type),
                    substitutions,
                )
            })
            .collect(),
        return_type: type_expr_to_type_with_substitutions(
            &method.signature.return_type,
            resolved,
            Some(self_type),
            substitutions,
        ),
    }
}

fn method_shape_label(
    method: &MethodSignature,
    resolved: &ResolveOutput,
    self_type: &Type,
    substitutions: &HashMap<String, Type>,
) -> String {
    let shape = method_shape(method, resolved, self_type, substitutions);
    let parameters = shape
        .parameters
        .iter()
        .map(Type::display)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "method {}.{}({}): {}",
        method_receiver_shape_label(&shape.receiver),
        method.name,
        parameters,
        shape.return_type.display()
    )
}

fn method_receiver_shape_label(receiver: &Type) -> &'static str {
    let display = receiver.display();
    if display.starts_with("&+") {
        return "&+self";
    }
    if display.starts_with('&') {
        return "&self";
    }
    "self"
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

use super::copyability::implicit_non_copy_struct_identifier_source;
use super::diagnostics::{
    argument_count_mismatch_diagnostic, argument_type_mismatch_diagnostic,
    associated_function_unknown_diagnostic, field_called_as_method_diagnostic,
    method_readwrite_receiver_requires_var_diagnostic, method_receiver_unsupported_diagnostic,
    method_unknown_diagnostic, non_copy_struct_argument_diagnostic,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::type_expr::type_expr_to_type_with_substitutions;
use crate::ast::{CallExpr, Expr, MemberExpr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::resolve::{
    FunctionSignature, MethodSignature, ResolveOutput, TypeSymbol, TypeSymbolKind,
};
use crate::source::{ByteSpan, SourceMap};
use std::collections::{HashMap, HashSet};

pub(super) fn check_known_function_call(
    sources: &SourceMap,
    call: &CallExpr,
    signature: &CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    if call.arguments.len() != signature.signature.parameters.len() {
        diagnostics.push(argument_count_mismatch_diagnostic(
            sources,
            call,
            signature,
            signature.signature.parameters.len(),
            call.arguments.len(),
        ));
        return;
    }

    let substitutions = infer_generic_substitutions(call, signature, resolved, environment);
    for (index, (argument, parameter)) in call
        .arguments
        .iter()
        .zip(signature.signature.parameters.iter())
        .enumerate()
    {
        let expected = type_expr_to_type_with_substitutions(
            &parameter.ty,
            resolved,
            signature.self_type.as_ref(),
            &substitutions,
        );
        let actual = expression_type(argument, resolved, environment);
        if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
            continue;
        }
        if expected.first_unsized_part().is_some() {
            continue;
        }

        if !is_expression_assignable(&expected, argument, resolved, environment) {
            diagnostics.push(argument_type_mismatch_diagnostic(
                sources, index, argument, parameter, &expected, &actual,
            ));
            continue;
        }

        if let Some((source_name, type_name)) =
            implicit_non_copy_struct_identifier_source(argument, resolved, environment)
        {
            diagnostics.push(non_copy_struct_argument_diagnostic(
                sources,
                index,
                argument,
                parameter,
                source_name,
                &type_name,
            ));
        }
    }
}

pub(super) fn call_return_type(
    call: &CallExpr,
    signature: &CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let substitutions = infer_generic_substitutions(call, signature, resolved, environment);
    type_expr_to_type_with_substitutions(
        &signature.signature.return_type,
        resolved,
        signature.self_type.as_ref(),
        &substitutions,
    )
}

#[derive(Debug, Clone)]
pub(super) struct CheckedCallSignature<'a> {
    pub(super) signature: &'a FunctionSignature,
    pub(super) self_type: Option<Type>,
    pub(super) name: String,
    pub(super) kind: CheckedCallKind,
    pub(super) declaration_span: Option<ByteSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CheckedCallKind {
    Function,
    AssociatedFunction,
    Method,
}

impl CheckedCallKind {
    pub(super) fn noun(self) -> &'static str {
        match self {
            CheckedCallKind::Function => "function",
            CheckedCallKind::AssociatedFunction => "associated function",
            CheckedCallKind::Method => "method",
        }
    }
}

pub(super) fn resolved_call_signature<'a>(
    resolved: &'a ResolveOutput,
    call: &CallExpr,
    environment: &TypeEnvironment,
) -> Option<CheckedCallSignature<'a>> {
    if let Some(signature) = resolved.function_signature_for_call(call) {
        return Some(CheckedCallSignature {
            signature,
            self_type: None,
            name: resolved.call_name_for_diagnostic(call),
            kind: CheckedCallKind::Function,
            declaration_span: resolved
                .symbol_for_call(call)
                .map(|symbol| symbol.declaration_span),
        });
    }

    if let Some((owner, function)) = resolved.associated_function_for_call(call) {
        return Some(CheckedCallSignature {
            signature: &function.signature,
            self_type: Some(Type::Named(owner.canonical_name.clone())),
            name: format!("{}.{}", owner.canonical_name, function.name),
            kind: CheckedCallKind::AssociatedFunction,
            declaration_span: Some(function.name_span),
        });
    }

    resolved_method_for_call(resolved, call, environment).map(|(owner, method)| {
        CheckedCallSignature {
            signature: &method.signature,
            self_type: Some(Type::Named(owner.canonical_name.clone())),
            name: format!("{}.{}", owner.canonical_name, method.name),
            kind: CheckedCallKind::Method,
            declaration_span: Some(method.name_span),
        }
    })
}

pub(super) fn resolved_method_for_call<'a>(
    resolved: &'a ResolveOutput,
    call: &CallExpr,
    environment: &TypeEnvironment,
) -> Option<(&'a TypeSymbol, &'a MethodSignature)> {
    let member = method_member_for_call(call)?;
    let receiver_type = expression_type(&member.object, resolved, environment);
    let owner = inherent_method_owner_for_type(&receiver_type, resolved)?;
    let method = owner
        .methods
        .iter()
        .find(|method| method.is_accessible && method.name == member.member)?;

    Some((owner, method))
}

fn infer_generic_substitutions(
    call: &CallExpr,
    signature: &CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> HashMap<String, Type> {
    if signature.signature.generic_parameters.is_empty() {
        return HashMap::new();
    }

    let parameters = signature
        .signature
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut substitutions = HashMap::new();
    for (argument, parameter) in call
        .arguments
        .iter()
        .zip(signature.signature.parameters.iter())
    {
        let actual = expression_type(argument, resolved, environment);
        if actual.is_unknown_or_unresolved() {
            continue;
        }
        infer_from_type_expr(
            &parameter.ty,
            &actual,
            resolved,
            signature.self_type.as_ref(),
            &parameters,
            &mut substitutions,
        );
    }
    substitutions
}

fn infer_from_type_expr(
    expected: &TypeExpr,
    actual: &Type,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    parameters: &HashSet<&str>,
    substitutions: &mut HashMap<String, Type>,
) {
    match expected {
        TypeExpr::Reference(reference) if reference.name == "Self" => {
            if self_type.is_some_and(|self_type| self_type == actual) {
                return;
            }
        }
        TypeExpr::Reference(reference) if parameters.contains(reference.name.as_str()) => {
            substitutions
                .entry(reference.name.clone())
                .or_insert_with(|| actual.clone());
        }
        TypeExpr::Pointer(pointer) => {
            if let Type::Pointer(actual_inner) = actual {
                infer_from_type_expr(
                    &pointer.inner,
                    actual_inner,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Borrow(borrow) => {
            if let Some(actual_inner) = borrowed_actual_inner_type(actual, borrow.is_readwrite) {
                infer_from_type_expr(
                    &borrow.inner,
                    &actual_inner,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::View(view) => {
            if let Type::ArrayData { element } = actual {
                infer_from_type_expr(
                    &view.element,
                    element,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Array(array) => {
            if let Type::Array { element, .. } = actual {
                infer_from_type_expr(
                    &array.element,
                    element,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Optional(optional) => {
            if let Type::Optional(actual_inner) = actual {
                infer_from_type_expr(
                    &optional.inner,
                    actual_inner,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Fallible(fallible) => {
            if let Type::Fallible { success, error } = actual {
                infer_from_type_expr(
                    &fallible.success,
                    success,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
                infer_from_type_expr(
                    &fallible.error,
                    error,
                    resolved,
                    self_type,
                    parameters,
                    substitutions,
                );
            }
        }
        TypeExpr::Generic(generic) => {
            if let Some(expected_arguments) =
                expected_generic_parts(generic, actual, resolved, self_type)
                && expected_arguments.len() == generic.arguments.len()
            {
                for (expected_argument, actual_argument) in
                    generic.arguments.iter().zip(expected_arguments.iter())
                {
                    infer_from_type_expr(
                        expected_argument,
                        actual_argument,
                        resolved,
                        self_type,
                        parameters,
                        substitutions,
                    );
                }
            }
        }
        TypeExpr::Reference(_) => {}
    }
}

fn expected_generic_parts(
    generic: &crate::ast::GenericType,
    actual: &Type,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Option<Vec<Type>> {
    let expected_name = if generic.name == "Self" {
        self_type?.nominal_name()?.to_string()
    } else {
        resolved
            .type_symbol_by_reference_name(&generic.name)
            .map(|symbol| symbol.canonical_name.clone())
            .unwrap_or_else(|| generic.name.clone())
    };

    match actual {
        Type::Generic { name, arguments } if *name == expected_name => Some(arguments.clone()),
        _ => None,
    }
}

fn borrowed_actual_inner_type(actual: &Type, is_readwrite: bool) -> Option<Type> {
    match actual {
        Type::Str if !is_readwrite => Some(Type::StrData),
        Type::View {
            is_readwrite: actual_readwrite,
            element,
        } if *actual_readwrite == is_readwrite => Some(Type::ArrayData {
            element: element.clone(),
        }),
        Type::Named(name) if is_readwrite => {
            name.strip_prefix("&+").map(simple_type_from_display_name)
        }
        Type::Named(name) if !is_readwrite => {
            name.strip_prefix('&').map(simple_type_from_display_name)
        }
        _ => None,
    }
}

fn simple_type_from_display_name(name: &str) -> Type {
    match name {
        "i32" => Type::I32,
        "bool" | "i8" | "i16" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" => {
            Type::Primitive(name.to_string())
        }
        "str" => Type::StrData,
        "&str" => Type::Str,
        "error" => Type::Error,
        "void" => Type::Void,
        "never" => Type::Never,
        name if name.starts_with('*') => {
            Type::Pointer(Box::new(simple_type_from_display_name(&name[1..])))
        }
        name => parse_generic_display_type(name).unwrap_or_else(|| Type::Named(name.to_string())),
    }
}

fn parse_generic_display_type(name: &str) -> Option<Type> {
    let open = name.find('<')?;
    let close = name.rfind('>')?;
    if close != name.len() - 1 || close <= open {
        return None;
    }
    let arguments = split_top_level_type_arguments(&name[open + 1..close])
        .into_iter()
        .map(simple_type_from_display_name)
        .collect();
    Some(Type::Generic {
        name: name[..open].trim().to_string(),
        arguments,
    })
}

fn split_top_level_type_arguments(arguments: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in arguments.char_indices() {
        match ch {
            '<' | '[' | '(' => depth += 1,
            '>' | ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                result.push(arguments[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    result.push(arguments[start..].trim());
    result
}

pub(super) fn check_method_receiver_call(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let Some((owner, method)) = resolved_method_for_call(resolved, call, environment) else {
        return;
    };
    let Some(member) = method_member_for_call(call) else {
        return;
    };

    match method_receiver_kind(method) {
        Some(MethodReceiverKind::Owned | MethodReceiverKind::ReadonlyBorrow) => {}
        Some(MethodReceiverKind::ReadwriteBorrow) => {
            if receiver_is_mutable_binding(member, environment) {
                return;
            }

            diagnostics.push(method_readwrite_receiver_requires_var_diagnostic(
                sources, member, owner, method,
            ));
        }
        None => diagnostics.push(method_receiver_unsupported_diagnostic(
            sources, member, owner, method,
        )),
    }
}

pub(super) fn check_unresolved_member_call(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let Some(member) = method_member_for_call(call) else {
        return;
    };

    if resolved_call_signature(resolved, call, environment).is_some() {
        return;
    }

    if let Some(owner) = associated_function_owner_for_member(member, resolved) {
        diagnostics.push(associated_function_unknown_diagnostic(
            sources, member, owner,
        ));
        return;
    }

    let receiver_type = expression_type(&member.object, resolved, environment);
    if receiver_type.is_unknown_or_unresolved() {
        return;
    }

    let Some(owner) = inherent_method_owner_for_type(&receiver_type, resolved) else {
        diagnostics.push(method_unknown_diagnostic(
            sources,
            member,
            &receiver_type,
            None,
        ));
        return;
    };

    if let Some(field) = owner
        .fields
        .iter()
        .find(|field| field.is_accessible && field.name == member.member)
    {
        diagnostics.push(field_called_as_method_diagnostic(
            sources, member, owner, field,
        ));
        return;
    }

    diagnostics.push(method_unknown_diagnostic(
        sources,
        member,
        &receiver_type,
        Some(owner),
    ));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MethodReceiverKind {
    Owned,
    ReadonlyBorrow,
    ReadwriteBorrow,
}

fn method_receiver_kind(method: &MethodSignature) -> Option<MethodReceiverKind> {
    match &method.receiver.ty {
        TypeExpr::Reference(reference) if reference.name == "Self" => {
            Some(MethodReceiverKind::Owned)
        }
        TypeExpr::Borrow(borrow) if type_expr_is_self_reference(&borrow.inner) => {
            if borrow.is_readwrite {
                Some(MethodReceiverKind::ReadwriteBorrow)
            } else {
                Some(MethodReceiverKind::ReadonlyBorrow)
            }
        }
        _ => None,
    }
}

fn type_expr_is_self_reference(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "Self")
}

fn receiver_is_mutable_binding(member: &MemberExpr, environment: &TypeEnvironment) -> bool {
    let Expr::Identifier(identifier) = member.object.as_ref() else {
        return false;
    };

    environment.is_mutable_binding(&identifier.name)
}

fn inherent_method_owner_for_type<'a>(
    ty: &Type,
    resolved: &'a ResolveOutput,
) -> Option<&'a TypeSymbol> {
    let canonical_name = ty.nominal_name()?;

    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| matches!(symbol.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum))
}

fn associated_function_owner_for_member<'a>(
    member: &MemberExpr,
    resolved: &'a ResolveOutput,
) -> Option<&'a TypeSymbol> {
    let Expr::Identifier(type_name) = member.object.as_ref() else {
        return None;
    };

    resolved
        .type_symbol_by_name(&type_name.name)
        .filter(|symbol| matches!(symbol.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum))
}

pub(super) fn method_member_for_call(call: &CallExpr) -> Option<&MemberExpr> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };

    Some(member)
}

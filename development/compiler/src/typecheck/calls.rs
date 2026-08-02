use super::copyability::implicit_non_copy_owned_value_source;
use super::diagnostics::{
    argument_count_mismatch_diagnostic, argument_type_mismatch_diagnostic,
    associated_function_unknown_diagnostic, field_called_as_method_diagnostic,
    method_readwrite_receiver_requires_var_diagnostic, method_unknown_diagnostic,
    non_copy_struct_argument_diagnostic,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::type_expr::{
    infer_type_expr_substitutions, simple_type_from_display_name,
    type_expr_to_type_with_substitutions,
};
use super::visibility::member_visibility_is_accessible;
use crate::ast::{CallExpr, Expr, MemberExpr, MethodReceiverMode, TypeExpr};
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

        if let Some(source) = implicit_non_copy_owned_value_source(argument, resolved, environment)
        {
            diagnostics.push(non_copy_struct_argument_diagnostic(
                sources,
                index,
                argument,
                parameter,
                &source.source_name,
                &source.type_name,
                source.kind,
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
    pub(super) impl_target_ty: Option<&'a TypeExpr>,
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
            impl_target_ty: None,
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
            impl_target_ty: None,
            name: format!("{}.{}", owner.canonical_name, function.name),
            kind: CheckedCallKind::AssociatedFunction,
            declaration_span: Some(function.name_span),
        });
    }

    resolved_method_for_call(resolved, call, environment).map(|(owner, method)| {
        let receiver_type = method_member_for_call(call)
            .map(|member| expression_type(&member.object, resolved, environment))
            .unwrap_or_else(|| Type::Named(owner.canonical_name.clone()));
        let self_type = method_self_type_for_receiver(&receiver_type);
        CheckedCallSignature {
            signature: &method.signature,
            self_type: Some(self_type),
            impl_target_ty: method.impl_target_ty.as_ref(),
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
    let self_type = method_self_type_for_receiver(&receiver_type);
    let owner = inherent_method_owner_for_type(&self_type, resolved)?;
    let method = owner.methods.iter().find(|method| {
        method_is_accessible(method, member.member_span.source, resolved)
            && method.name == member.member
            && method_applies_to_receiver(method, &self_type, resolved)
    })?;

    Some((owner, method))
}

pub(super) fn infer_generic_substitutions(
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
    if let (Some(impl_target_ty), Some(self_type)) =
        (signature.impl_target_ty, signature.self_type.as_ref())
    {
        infer_type_expr_substitutions(
            impl_target_ty,
            self_type,
            resolved,
            None,
            &parameters,
            &mut substitutions,
        );
    }

    for (argument, parameter) in call
        .arguments
        .iter()
        .zip(signature.signature.parameters.iter())
    {
        let actual = expression_type(argument, resolved, environment);
        if actual.is_unknown_or_unresolved() {
            continue;
        }
        infer_type_expr_substitutions(
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

pub(super) fn method_applies_to_receiver(
    method: &MethodSignature,
    receiver_type: &Type,
    resolved: &ResolveOutput,
) -> bool {
    let Some(impl_target_ty) = &method.impl_target_ty else {
        return true;
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
        receiver_type,
        resolved,
        None,
        &parameters,
        &mut substitutions,
    );
    let expected =
        type_expr_to_type_with_substitutions(impl_target_ty, resolved, None, &substitutions);

    !expected.is_unknown_or_unresolved() && expected == *receiver_type
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

    match method.receiver.mode {
        MethodReceiverMode::Owned | MethodReceiverMode::ReadonlyBorrow => {}
        MethodReceiverMode::ReadwriteBorrow => {
            if receiver_is_mutable_binding(member, environment) {
                return;
            }

            diagnostics.push(method_readwrite_receiver_requires_var_diagnostic(
                sources, member, owner, method,
            ));
        }
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

    let self_type = method_self_type_for_receiver(&receiver_type);
    let Some(owner) = inherent_method_owner_for_type(&self_type, resolved) else {
        diagnostics.push(method_unknown_diagnostic(
            sources,
            member,
            &receiver_type,
            None,
        ));
        return;
    };

    if let Some(field) = owner.fields.iter().find(|field| {
        field_is_accessible(field, member.member_span.source, resolved)
            && field.name == member.member
    }) {
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

fn receiver_is_mutable_binding(member: &MemberExpr, environment: &TypeEnvironment) -> bool {
    match unwrap_group(&member.object) {
        Expr::Identifier(identifier) => {
            aggregate_member_root_is_writable_place(&identifier.name, environment)
        }
        Expr::Member(_) => aggregate_member_root_name(&member.object)
            .is_some_and(|name| aggregate_member_root_is_writable_place(name, environment)),
        _ => false,
    }
}

fn aggregate_member_root_name(expression: &Expr) -> Option<&str> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some(&identifier.name),
        Expr::Member(member) => aggregate_member_root_name(&member.object),
        _ => None,
    }
}

fn aggregate_member_root_is_writable_place(name: &str, environment: &TypeEnvironment) -> bool {
    environment.is_mutable_binding(name)
        || environment
            .get(name)
            .is_some_and(|ty| matches!(ty, Type::Named(name) if name.starts_with("&+")))
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
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

fn method_is_accessible(
    method: &MethodSignature,
    use_source: crate::source::SourceId,
    resolved: &ResolveOutput,
) -> bool {
    method.is_accessible
        && member_visibility_is_accessible(
            method.visibility,
            method.name_span,
            use_source,
            resolved,
        )
}

fn field_is_accessible(
    field: &crate::resolve::StructFieldSignature,
    use_source: crate::source::SourceId,
    resolved: &ResolveOutput,
) -> bool {
    field.is_accessible
        && member_visibility_is_accessible(field.visibility, field.name_span, use_source, resolved)
}

pub(super) fn method_self_type_for_receiver(receiver_type: &Type) -> Type {
    match receiver_type {
        Type::Named(name) => name
            .strip_prefix("&+")
            .or_else(|| name.strip_prefix('&'))
            .map(simple_type_from_display_name)
            .unwrap_or_else(|| receiver_type.clone()),
        _ => receiver_type.clone(),
    }
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

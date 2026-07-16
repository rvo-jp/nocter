use super::diagnostics::{
    argument_count_mismatch_diagnostic, argument_type_mismatch_diagnostic,
    associated_function_unknown_diagnostic, field_called_as_method_diagnostic,
    method_readwrite_receiver_requires_var_diagnostic, method_receiver_unsupported_diagnostic,
    method_unknown_diagnostic,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::type_expr::type_expr_to_type_with_self_type;
use crate::ast::{CallExpr, Expr, MemberExpr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::resolve::{
    FunctionSignature, MethodSignature, ResolveOutput, TypeSymbol, TypeSymbolKind,
};
use crate::source::{ByteSpan, SourceMap};

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

    for (index, (argument, parameter)) in call
        .arguments
        .iter()
        .zip(signature.signature.parameters.iter())
        .enumerate()
    {
        let expected =
            type_expr_to_type_with_self_type(&parameter.ty, resolved, signature.self_type.as_ref());
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
        }
    }
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
    let Type::Named(canonical_name) = ty else {
        return None;
    };

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

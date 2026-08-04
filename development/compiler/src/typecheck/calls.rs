use super::copyability::implicit_non_copy_owned_value_source;
use super::diagnostics::{
    ambiguous_default_method_diagnostic, ambiguous_generic_bound_method_diagnostic,
    argument_count_mismatch_diagnostic, argument_type_mismatch_diagnostic,
    associated_function_unknown_diagnostic, closure_callable_contract_diagnostic,
    field_called_as_method_diagnostic, generic_bound_not_satisfied_diagnostic,
    method_readwrite_receiver_requires_var_diagnostic, method_unknown_diagnostic,
    non_copy_struct_argument_diagnostic,
};
use super::expressions::expression_type;
use super::interface_bounds::{
    implemented_interface_types, interface_symbol_for_bound,
    interface_symbols_for_generic_parameter, type_satisfies_interface_bound,
    type_symbol_substitutions,
};
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
    check_generic_interface_bounds(
        sources,
        call,
        signature,
        &substitutions,
        resolved,
        environment,
        diagnostics,
    );
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
        let self_type = method_self_type_for_receiver_in_environment(&receiver_type, environment);
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
    let self_type = method_self_type_for_receiver_in_environment(&receiver_type, environment);
    match &self_type {
        Type::Parameter(parameter) => {
            let mut candidates =
                bounded_method_candidates(parameter, &self_type, member, environment, resolved)
                    .into_iter();
            let candidate = candidates.next()?;
            candidates.next().is_none().then_some(candidate)
        }
        _ => {
            if let Some(owner) = inherent_method_owner_for_type(&self_type, resolved)
                && let Some(method) = owner.methods.iter().find(|method| {
                    method_is_accessible(method, member.member_span.source, resolved)
                        && method.name == member.member
                        && method_applies_to_receiver(method, &self_type, resolved)
                })
            {
                return Some((owner, method));
            }
            let mut candidates = super::default_methods::candidates(
                &self_type,
                &member.member,
                member.member_span.source,
                resolved,
            )
            .into_iter();
            let candidate = candidates.next()?;
            candidates.next().is_none().then_some(candidate)
        }
    }
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
    if signature.kind == CheckedCallKind::Method
        && let Some(member) = method_member_for_call(call)
        && let Some((owner, _)) = resolved_method_for_call(resolved, call, environment)
        && owner.kind == TypeSymbolKind::Interface
    {
        let receiver_type = method_self_type_for_receiver_in_environment(
            &expression_type(&member.object, resolved, environment),
            environment,
        );
        let interface_type = match &receiver_type {
            Type::Parameter(parameter) => {
                interface_symbols_for_generic_parameter(parameter, environment, resolved)
                    .into_iter()
                    .find(|(candidate, _)| candidate.canonical_name == owner.canonical_name)
                    .map(|(_, bound)| bound)
            }
            _ => implemented_interface_types(&receiver_type, resolved)
                .into_iter()
                .find(|implemented| {
                    implemented.nominal_name() == Some(owner.canonical_name.as_str())
                }),
        };
        if let Some(interface_type) = interface_type {
            substitutions.extend(type_symbol_substitutions(owner, &interface_type));
        }
    }
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
        let actual = if let Expr::Closure(closure) = argument.without_groups()
            && let TypeExpr::Reference(reference) = &parameter.ty
            && let Some(contract) = super::closures::expected_callable_contract_for_generic(
                &reference.name,
                signature.signature,
                &substitutions,
                resolved,
            ) {
            let expected_parameters = contract
                .parameters
                .iter()
                .all(|ty| !ty.is_unknown_or_unresolved())
                .then_some(contract.parameters.as_slice());
            let expected_return =
                (!contract.return_type.is_unknown_or_unresolved()).then_some(&contract.return_type);
            let Some(actual) = super::closures::infer_closure_type(
                closure,
                resolved,
                environment,
                expected_parameters,
                expected_return,
            ) else {
                continue;
            };
            infer_type_expr_substitutions(
                &parameter.ty,
                &actual,
                resolved,
                signature.self_type.as_ref(),
                &parameters,
                &mut substitutions,
            );
            super::closures::infer_substitutions_from_closure_contract(
                &contract,
                &actual,
                resolved,
                signature.self_type.as_ref(),
                &parameters,
                &mut substitutions,
            );
            actual
        } else {
            expression_type(argument, resolved, environment)
        };
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
    infer_substitutions_from_interface_bounds(signature, resolved, &parameters, &mut substitutions);
    substitutions
}

fn infer_substitutions_from_interface_bounds(
    signature: &CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    parameters: &HashSet<&str>,
    substitutions: &mut HashMap<String, Type>,
) {
    for _ in 0..signature.signature.generic_parameters.len() {
        let before = substitutions.len();
        for (parameter, bounds) in signature
            .signature
            .generic_parameters
            .iter()
            .zip(&signature.signature.generic_parameter_bounds)
        {
            let Some(actual) = substitutions.get(parameter).cloned() else {
                continue;
            };
            for bound in bounds {
                let expected_interface_name = match bound {
                    TypeExpr::Reference(reference) => reference.name.as_str(),
                    TypeExpr::Generic(generic) => generic.name.as_str(),
                    _ => continue,
                };
                for implemented in implemented_interface_types(&actual, resolved) {
                    let matches_interface = implemented
                        .nominal_name()
                        .and_then(|name| resolved.type_symbol_by_canonical_name(name))
                        .is_some_and(|symbol| {
                            symbol.canonical_name == expected_interface_name
                                || symbol
                                    .canonical_name
                                    .rsplit('.')
                                    .next()
                                    .is_some_and(|name| name == expected_interface_name)
                        });
                    if matches_interface {
                        infer_type_expr_substitutions(
                            bound,
                            &implemented,
                            resolved,
                            None,
                            parameters,
                            substitutions,
                        );
                        break;
                    }
                }
            }
        }
        if substitutions.len() == before {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_generic_interface_bounds(
    sources: &SourceMap,
    call: &CallExpr,
    signature: &CheckedCallSignature<'_>,
    substitutions: &HashMap<String, Type>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (parameter, bounds) in signature
        .signature
        .generic_parameters
        .iter()
        .zip(&signature.signature.generic_parameter_bounds)
    {
        let Some(actual) = substitutions.get(parameter) else {
            continue;
        };
        for bound in bounds {
            let Some((_, bound_type)) = interface_symbol_for_bound(bound, substitutions, resolved)
            else {
                continue;
            };
            if type_satisfies_bound_in_environment(actual, &bound_type, resolved, environment) {
                continue;
            }
            let span =
                generic_argument_evidence_span(call, &signature.signature.parameters, parameter)
                    .unwrap_or(call.span);
            if let Type::Closure(closure) = actual
                && let Some(expected_capability) =
                    super::closures::callable_bound_capability(&bound_type, resolved)
            {
                diagnostics.push(closure_callable_contract_diagnostic(
                    sources,
                    span,
                    closure,
                    &bound_type,
                    expected_capability,
                    bound.span(),
                ));
                continue;
            }
            diagnostics.push(generic_bound_not_satisfied_diagnostic(
                sources,
                span,
                actual,
                &bound_type,
                bound.span(),
            ));
        }
    }
}

fn type_satisfies_bound_in_environment(
    actual: &Type,
    bound: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if let Type::Closure(closure) = actual
        && super::closures::closure_satisfies_callable_bound(closure, bound, resolved)
    {
        return true;
    }
    if type_satisfies_interface_bound(actual, bound, resolved) {
        return true;
    }
    let parameter = match actual {
        Type::Parameter(parameter) => parameter,
        Type::Named(parameter) if environment.generic_bounds(parameter).is_some() => parameter,
        _ => return false,
    };
    interface_symbols_for_generic_parameter(parameter, environment, resolved)
        .into_iter()
        .any(|(_, actual_bound)| actual_bound == *bound)
}

fn generic_argument_evidence_span(
    call: &CallExpr,
    parameters: &[crate::resolve::ParameterSignature],
    generic: &str,
) -> Option<ByteSpan> {
    call.arguments
        .iter()
        .zip(parameters)
        .find_map(|(argument, parameter)| {
            type_expr_mentions_parameter(&parameter.ty, generic).then(|| argument.span())
        })
}

fn type_expr_mentions_parameter(ty: &TypeExpr, parameter: &str) -> bool {
    match ty {
        TypeExpr::Closure(closure) => {
            closure
                .captures
                .iter()
                .any(|capture| type_expr_mentions_parameter(&capture.ty, parameter))
                || closure
                    .parameters
                    .iter()
                    .any(|ty| type_expr_mentions_parameter(ty, parameter))
                || type_expr_mentions_parameter(&closure.return_type, parameter)
        }
        TypeExpr::Reference(reference) => reference.name == parameter,
        TypeExpr::Generic(generic) => {
            generic.name == parameter
                || generic
                    .arguments
                    .iter()
                    .any(|argument| type_expr_mentions_parameter(argument, parameter))
        }
        TypeExpr::Pointer(pointer) => type_expr_mentions_parameter(&pointer.inner, parameter),
        TypeExpr::Borrow(borrow) => type_expr_mentions_parameter(&borrow.inner, parameter),
        TypeExpr::View(view) => type_expr_mentions_parameter(&view.element, parameter),
        TypeExpr::Array(array) => type_expr_mentions_parameter(&array.element, parameter),
        TypeExpr::Optional(optional) => type_expr_mentions_parameter(&optional.inner, parameter),
        TypeExpr::Fallible(fallible) => {
            type_expr_mentions_parameter(&fallible.success, parameter)
                || type_expr_mentions_parameter(&fallible.error, parameter)
        }
    }
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

    let self_type = method_self_type_for_receiver_in_environment(&receiver_type, environment);
    if let Type::Parameter(parameter) = &self_type {
        let candidates =
            bounded_method_candidates(parameter, &self_type, member, environment, resolved);
        if candidates.len() > 1 {
            diagnostics.push(ambiguous_generic_bound_method_diagnostic(
                sources,
                member.member_span,
                &member.member,
                &candidates,
            ));
            return;
        }
    } else {
        let candidates = super::default_methods::candidates(
            &self_type,
            &member.member,
            member.member_span.source,
            resolved,
        );
        if candidates.len() > 1 {
            diagnostics.push(ambiguous_default_method_diagnostic(
                sources,
                member.member_span,
                &member.member,
                &candidates,
            ));
            return;
        }
    }
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

fn bounded_method_candidates<'a>(
    parameter: &str,
    self_type: &Type,
    member: &MemberExpr,
    environment: &TypeEnvironment,
    resolved: &'a ResolveOutput,
) -> Vec<(&'a TypeSymbol, &'a MethodSignature)> {
    interface_symbols_for_generic_parameter(parameter, environment, resolved)
        .into_iter()
        .filter_map(|(owner, _)| {
            owner
                .methods
                .iter()
                .find(|method| {
                    method_is_accessible(method, member.member_span.source, resolved)
                        && method.name == member.member
                        && method_applies_to_receiver(method, self_type, resolved)
                })
                .map(|method| (owner, method))
        })
        .collect()
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

pub(super) fn method_self_type_for_receiver_in_environment(
    receiver_type: &Type,
    environment: &TypeEnvironment,
) -> Type {
    let self_type = method_self_type_for_receiver(receiver_type);
    let Type::Named(name) = &self_type else {
        return self_type;
    };
    if environment.generic_bounds(name).is_some() {
        Type::Parameter(name.clone())
    } else {
        self_type
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

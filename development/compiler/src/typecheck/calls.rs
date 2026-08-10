use super::coercions::{SelectedCoercion, receiver_coercion_candidates};
use super::copyability::{implicit_non_copy_owned_value_source, type_is_copy_in_environment};
use super::diagnostics::{
    ambiguous_concrete_method_diagnostic, ambiguous_generic_bound_method_diagnostic,
    argument_count_mismatch_diagnostic, argument_type_mismatch_diagnostic,
    associated_function_unknown_diagnostic, closure_callable_contract_diagnostic,
    copy_requirement_not_satisfied_diagnostic, field_called_as_method_diagnostic,
    generic_bound_not_satisfied_diagnostic, method_readwrite_receiver_requires_var_diagnostic,
    method_unknown_diagnostic, non_copy_struct_argument_diagnostic,
    type_equality_not_satisfied_diagnostic,
};
use super::expressions::expression_type;
use super::interface_bounds::{
    conformed_interface_types, interface_symbol_for_bound, interface_symbols_for_constrained_type,
    interface_symbols_for_generic_parameter, type_satisfies_interface_bound,
    type_symbol_substitutions,
};
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::type_expr::{infer_type_expr_substitutions, type_expr_to_type_with_substitutions};
use super::visibility::member_visibility_is_accessible;
use crate::ast::{CallExpr, Expr, MemberExpr, MethodReceiverMode, TypeExpr};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
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
    let specialized_self_type = signature
        .self_type
        .as_ref()
        .map(|ty| ty.substitute_parameters(&substitutions));
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
            specialized_self_type.as_ref(),
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
    let specialized_self_type = signature
        .self_type
        .as_ref()
        .map(|ty| ty.substitute_parameters(&substitutions));
    type_expr_to_type_with_substitutions(
        &signature.signature.return_type,
        resolved,
        specialized_self_type.as_ref(),
        &substitutions,
    )
}

#[derive(Debug, Clone)]
pub(super) struct CheckedCallSignature<'a> {
    pub(super) signature: &'a FunctionSignature,
    pub(super) self_type: Option<Type>,
    pub(super) owner_target_ty: Option<&'a TypeExpr>,
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
            owner_target_ty: None,
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
            self_type: Some(type_symbol_self_type(owner)),
            owner_target_ty: None,
            name: format!("{}.{}", owner.canonical_name, function.name),
            kind: CheckedCallKind::AssociatedFunction,
            declaration_span: Some(function.name_span),
        });
    }

    resolved_method_call(resolved, call, environment).map(|selected| CheckedCallSignature {
        signature: &selected.method.signature,
        self_type: Some(selected.self_type),
        owner_target_ty: selected.method.owner_target_ty.as_ref(),
        name: format!("{}.{}", selected.owner.canonical_name, selected.method.name),
        kind: CheckedCallKind::Method,
        declaration_span: Some(selected.method.name_span),
    })
}

fn type_symbol_self_type(owner: &TypeSymbol) -> Type {
    if owner.generic_parameters.is_empty() {
        return Type::Named(owner.canonical_name.clone());
    }

    Type::Generic {
        name: owner.canonical_name.clone(),
        arguments: owner
            .generic_parameters
            .iter()
            .map(|parameter| Type::Parameter(parameter.clone()))
            .collect(),
    }
}

pub(super) fn resolved_method_for_call<'a>(
    resolved: &'a ResolveOutput,
    call: &CallExpr,
    environment: &TypeEnvironment,
) -> Option<(&'a TypeSymbol, &'a MethodSignature)> {
    resolved_method_call(resolved, call, environment)
        .map(|selected| (selected.owner, selected.method))
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedMethodCall<'a> {
    pub(super) owner: &'a TypeSymbol,
    pub(super) method: &'a MethodSignature,
    pub(super) self_type: Type,
    pub(super) receiver_coercion: Option<SelectedCoercion>,
}

pub(super) fn resolved_method_call<'a>(
    resolved: &'a ResolveOutput,
    call: &CallExpr,
    environment: &TypeEnvironment,
) -> Option<ResolvedMethodCall<'a>> {
    let member = method_member_for_call(call)?;
    let receiver_type = expression_type(&member.object, resolved, environment);
    let self_type = method_self_type_for_receiver_in_environment(&receiver_type, environment);
    match &self_type {
        Type::Parameter(_) | Type::Projection { .. } => {
            let mut candidates =
                bounded_method_candidates(&self_type, member, environment, resolved).into_iter();
            let (owner, method) = candidates.next()?;
            candidates.next().is_none().then_some(ResolvedMethodCall {
                owner,
                method,
                self_type,
                receiver_coercion: None,
            })
        }
        _ => {
            let inherent = inherent_method_owner_for_type(&self_type, resolved).and_then(|owner| {
                owner
                    .methods
                    .iter()
                    .find(|method| {
                        method_is_accessible(method, member.member_span.source, resolved)
                            && method.name == member.member
                            && method_applies_to_receiver(method, &self_type, resolved)
                    })
                    .map(|method| (owner, method))
            });
            let mut interface_candidates = super::interface_methods::candidates(
                &self_type,
                &member.member,
                member.member_span.source,
                resolved,
            )
            .into_iter();
            let interface = interface_candidates.next();
            if inherent.is_some() && interface.is_some() || interface_candidates.next().is_some() {
                return None;
            }
            if let Some((owner, method)) = inherent.or(interface) {
                return Some(ResolvedMethodCall {
                    owner,
                    method,
                    self_type,
                    receiver_coercion: None,
                });
            }

            resolve_receiver_coerced_method(
                resolved,
                member,
                &receiver_type,
                &self_type,
                environment,
            )
        }
    }
}

fn resolve_receiver_coerced_method<'a>(
    resolved: &'a ResolveOutput,
    member: &MemberExpr,
    receiver_type: &Type,
    source_self_type: &Type,
    environment: &TypeEnvironment,
) -> Option<ResolvedMethodCall<'a>> {
    let mut candidates = receiver_coerced_method_candidates(
        resolved,
        member,
        receiver_type,
        source_self_type,
        environment,
    );
    let candidate = candidates.pop()?;
    candidates.is_empty().then_some(candidate)
}

fn receiver_coerced_method_candidates<'a>(
    resolved: &'a ResolveOutput,
    member: &MemberExpr,
    receiver_type: &Type,
    source_self_type: &Type,
    environment: &TypeEnvironment,
) -> Vec<ResolvedMethodCall<'a>> {
    let source_is_readwrite = matches!(
        receiver_type,
        Type::Borrow {
            is_readwrite: true,
            ..
        }
    ) || receiver_is_mutable_binding(member, environment);
    let coercions = receiver_coercion_candidates(source_self_type, source_is_readwrite, resolved);
    let mut candidates = Vec::new();
    for coercion in coercions {
        let target_self_type =
            method_self_type_for_receiver_in_environment(&coercion.target_type, environment);
        let inherent =
            inherent_method_owner_for_type(&target_self_type, resolved).and_then(|owner| {
                owner
                    .methods
                    .iter()
                    .find(|method| {
                        method_is_accessible(method, member.member_span.source, resolved)
                            && method.name == member.member
                            && method_applies_to_receiver(method, &target_self_type, resolved)
                    })
                    .map(|method| (owner, method))
            });
        let interfaces = super::interface_methods::candidates(
            &target_self_type,
            &member.member,
            member.member_span.source,
            resolved,
        );
        let target_candidates = inherent.into_iter().chain(interfaces);
        for (owner, method) in target_candidates {
            if !method_accepts_coerced_receiver(method, &coercion.target_type) {
                continue;
            }
            candidates.push(ResolvedMethodCall {
                owner,
                method,
                self_type: target_self_type.clone(),
                receiver_coercion: Some(coercion.clone()),
            });
        }
    }
    collapse_equivalent_receiver_coercion_candidates(candidates)
}

fn method_accepts_coerced_receiver(method: &MethodSignature, target: &Type) -> bool {
    match method.receiver.mode {
        MethodReceiverMode::Owned => false,
        MethodReceiverMode::ReadonlyBorrow => true,
        MethodReceiverMode::ReadwriteBorrow => receiver_type_is_readwrite(target),
    }
}

fn receiver_type_is_readwrite(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Borrow {
            is_readwrite: true,
            ..
        } | Type::View {
            is_readwrite: true,
            ..
        }
    )
}

fn collapse_equivalent_receiver_coercion_candidates<'a>(
    candidates: Vec<ResolvedMethodCall<'a>>,
) -> Vec<ResolvedMethodCall<'a>> {
    let mut collapsed: Vec<ResolvedMethodCall<'a>> = Vec::new();
    for candidate in candidates {
        let Some(coercion) = candidate.receiver_coercion.as_ref() else {
            collapsed.push(candidate);
            continue;
        };
        let Some(existing_index) = collapsed.iter().position(|existing| {
            existing.method.name_span == candidate.method.name_span
                && existing.owner.canonical_name == candidate.owner.canonical_name
        }) else {
            collapsed.push(candidate);
            continue;
        };
        let existing = &collapsed[existing_index];
        let Some(existing_coercion) = existing.receiver_coercion.as_ref() else {
            collapsed.push(candidate);
            continue;
        };
        let existing_rank = receiver_coercion_capability_rank(existing.method, existing_coercion);
        let candidate_rank = receiver_coercion_capability_rank(candidate.method, coercion);
        if candidate_rank < existing_rank {
            collapsed[existing_index] = candidate;
        } else if candidate_rank == existing_rank {
            collapsed.push(candidate);
        }
    }
    collapsed
}

fn receiver_coercion_capability_rank(
    method: &MethodSignature,
    coercion: &SelectedCoercion,
) -> (u8, u8) {
    let target_rank = match method.receiver.mode {
        MethodReceiverMode::ReadonlyBorrow => {
            u8::from(receiver_type_is_readwrite(&coercion.target_type))
        }
        MethodReceiverMode::ReadwriteBorrow | MethodReceiverMode::Owned => 0,
    };
    let source_rank = u8::from(coercion.receiver_mode == MethodReceiverMode::ReadwriteBorrow);
    (target_rank, source_rank)
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
            _ => conformed_interface_types(&receiver_type, resolved)
                .into_iter()
                .find(|implemented| {
                    implemented.nominal_name() == Some(owner.canonical_name.as_str())
                }),
        };
        if let Some(interface_type) = interface_type {
            substitutions.extend(type_symbol_substitutions(owner, &interface_type));
        }
    }
    if let (Some(owner_target_ty), Some(self_type)) =
        (signature.owner_target_ty, signature.self_type.as_ref())
    {
        infer_type_expr_substitutions(
            owner_target_ty,
            self_type.opaque_lowering_view(),
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
                signature.self_type.as_ref(),
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
    infer_substitutions_from_type_equalities(signature, resolved, &parameters, &mut substitutions);
    substitutions
}

fn infer_substitutions_from_type_equalities(
    signature: &CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    parameters: &HashSet<&str>,
    substitutions: &mut HashMap<String, Type>,
) {
    let Some(clause) = signature.signature.where_clause.as_ref() else {
        return;
    };
    for _ in 0..signature.signature.generic_parameters.len() {
        let before = substitutions.len();
        let self_type = signature
            .self_type
            .as_ref()
            .map(|ty| ty.substitute_parameters(substitutions));
        for refinement in clause.refinements() {
            let value = type_expr_to_type_with_substitutions(
                &refinement.value,
                resolved,
                self_type.as_ref(),
                substitutions,
            );
            if !value.is_unknown_or_unresolved() {
                infer_type_expr_substitutions(
                    &TypeExpr::Reference(crate::ast::TypeReference {
                        span: refinement.name_span,
                        name: refinement.name.clone(),
                    }),
                    &value,
                    resolved,
                    self_type.as_ref(),
                    parameters,
                    substitutions,
                );
            }
        }
        for equality in clause.equalities() {
            let left = type_expr_to_type_with_substitutions(
                &equality.left,
                resolved,
                self_type.as_ref(),
                substitutions,
            );
            let right = type_expr_to_type_with_substitutions(
                &equality.right,
                resolved,
                self_type.as_ref(),
                substitutions,
            );
            if !right.is_unknown_or_unresolved() {
                infer_type_expr_substitutions(
                    &equality.left,
                    &right,
                    resolved,
                    self_type.as_ref(),
                    parameters,
                    substitutions,
                );
            }
            if !left.is_unknown_or_unresolved() {
                infer_type_expr_substitutions(
                    &equality.right,
                    &left,
                    resolved,
                    self_type.as_ref(),
                    parameters,
                    substitutions,
                );
            }
        }
        if substitutions.len() == before {
            break;
        }
    }
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
            .zip(&signature.signature.generic_parameter_requirements)
        {
            let Some(actual) = substitutions.get(parameter).cloned() else {
                continue;
            };
            for bound in bounds.type_bounds() {
                let expected_interface_name = match bound {
                    TypeExpr::Reference(reference) => reference.name.as_str(),
                    TypeExpr::Generic(generic) => generic.name.as_str(),
                    _ => continue,
                };
                for implemented in conformed_interface_types(&actual, resolved) {
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
    let specialized_self_type = signature
        .self_type
        .as_ref()
        .map(|ty| ty.substitute_parameters(substitutions));
    for (parameter, bounds) in signature
        .signature
        .generic_parameters
        .iter()
        .zip(&signature.signature.generic_parameter_requirements)
    {
        let Some(actual) = substitutions.get(parameter) else {
            continue;
        };
        if let Some(requirement_span) = bounds.copy_span()
            && !type_is_copy_in_environment(actual, resolved, environment)
        {
            let span =
                generic_argument_evidence_span(call, &signature.signature.parameters, parameter)
                    .unwrap_or(call.span);
            diagnostics.push(copy_requirement_not_satisfied_diagnostic(
                sources,
                span,
                actual,
                requirement_span,
            ));
        }
        for bound in bounds.type_bounds() {
            let bound_type = match bound {
                TypeExpr::Callable(_) => type_expr_to_type_with_substitutions(
                    bound,
                    resolved,
                    specialized_self_type.as_ref(),
                    substitutions,
                ),
                _ => {
                    let Some((_, bound_type)) =
                        interface_symbol_for_bound(bound, substitutions, resolved)
                    else {
                        continue;
                    };
                    bound_type
                }
            };
            if type_satisfies_bound_in_environment(actual, &bound_type, resolved, environment) {
                continue;
            }
            let span =
                generic_argument_evidence_span(call, &signature.signature.parameters, parameter)
                    .unwrap_or(call.span);
            if let Type::Closure(closure) = actual
                && let Some(expected_capability) =
                    super::closures::callable_bound_capability(&bound_type)
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

    let Some(clause) = signature.signature.where_clause.as_ref() else {
        return;
    };
    for refinement in clause.refinements() {
        let Some(actual) = substitutions.get(&refinement.name) else {
            continue;
        };
        let expected = type_expr_to_type_with_substitutions(
            &refinement.value,
            resolved,
            specialized_self_type.as_ref(),
            substitutions,
        );
        if expected.is_unknown_or_unresolved() || environment.types_equal(actual, &expected) {
            continue;
        }
        diagnostics.push(type_equality_not_satisfied_diagnostic(
            sources,
            call.span,
            actual,
            &expected,
            refinement.span,
        ));
    }
    for equality in clause.equalities() {
        let left = type_expr_to_type_with_substitutions(
            &equality.left,
            resolved,
            specialized_self_type.as_ref(),
            substitutions,
        );
        let right = type_expr_to_type_with_substitutions(
            &equality.right,
            resolved,
            specialized_self_type.as_ref(),
            substitutions,
        );
        if left.is_unknown_or_unresolved()
            || right.is_unknown_or_unresolved()
            || environment.types_equal(&left, &right)
        {
            continue;
        }
        diagnostics.push(type_equality_not_satisfied_diagnostic(
            sources,
            call.span,
            &left,
            &right,
            equality.span,
        ));
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
        Type::Named(parameter) if environment.generic_requirements(parameter).is_some() => {
            parameter
        }
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
        TypeExpr::Callable(callable) => {
            callable
                .parameters
                .iter()
                .any(|input| type_expr_mentions_parameter(&input.ty, parameter))
                || type_expr_mentions_parameter(&callable.return_type, parameter)
        }
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
        TypeExpr::Opaque(opaque) => {
            type_expr_mentions_parameter(&opaque.interface, parameter)
                || opaque
                    .associated_bindings
                    .iter()
                    .any(|binding| type_expr_mentions_parameter(&binding.value, parameter))
                || opaque
                    .witness
                    .as_ref()
                    .is_some_and(|witness| type_expr_mentions_parameter(witness, parameter))
        }
        TypeExpr::Reference(reference) => reference.name == parameter,
        TypeExpr::Generic(generic) => {
            generic.name == parameter
                || generic
                    .arguments
                    .iter()
                    .any(|argument| type_expr_mentions_parameter(argument, parameter))
        }
        TypeExpr::Projection(projection) => {
            type_expr_mentions_parameter(&projection.base, parameter)
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
    let Some(owner_target_ty) = &method.owner_target_ty else {
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
        owner_target_ty,
        receiver_type,
        resolved,
        None,
        &parameters,
        &mut substitutions,
    );
    let expected =
        type_expr_to_type_with_substitutions(owner_target_ty, resolved, None, &substitutions);

    if let Some(clause) = &method.signature.where_clause {
        for refinement in clause.refinements() {
            let Some(actual) = substitutions.get(&refinement.name) else {
                return false;
            };
            let refined = type_expr_to_type_with_substitutions(
                &refinement.value,
                resolved,
                Some(receiver_type),
                &substitutions,
            );
            if refined.is_unknown_or_unresolved() || actual != &refined {
                return false;
            }
        }
    }

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
    if matches!(self_type, Type::Parameter(_) | Type::Projection { .. }) {
        let candidates = bounded_method_candidates(&self_type, member, environment, resolved);
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
        let mut candidates = super::interface_methods::candidates(
            &self_type,
            &member.member,
            member.member_span.source,
            resolved,
        );
        if let Some(owner) = inherent_method_owner_for_type(&self_type, resolved)
            && let Some(method) = owner.methods.iter().find(|method| {
                method_is_accessible(method, member.member_span.source, resolved)
                    && method.name == member.member
                    && method_applies_to_receiver(method, &self_type, resolved)
            })
        {
            candidates.push((owner, method));
        }
        if candidates.len() > 1 {
            diagnostics.push(ambiguous_concrete_method_diagnostic(
                sources,
                member.member_span,
                &member.member,
                &candidates,
            ));
            return;
        }

        if candidates.is_empty() {
            let coerced = receiver_coerced_method_candidates(
                resolved,
                member,
                &receiver_type,
                &self_type,
                environment,
            );
            if coerced.len() > 1 {
                diagnostics.push(receiver_coercion_ambiguity_diagnostic(
                    sources, member, &coerced,
                ));
                return;
            }
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

fn receiver_coercion_ambiguity_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    candidates: &[ResolvedMethodCall<'_>],
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0453",
        format!(
            "method `{}` is ambiguous across receiver coercions",
            member.member
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    for candidate in candidates {
        let Some(coercion) = &candidate.receiver_coercion else {
            continue;
        };
        if let Ok(span) = sources.span_to_json(coercion.focus_span) {
            diagnostic.notes.push(DiagnosticNote {
                message: format!(
                    "coercion to `{}` reaches `{}.{}`",
                    coercion.target_type.display(),
                    candidate.owner.canonical_name,
                    candidate.method.name
                ),
                span: Some(span),
            });
        }
    }
    diagnostic.help =
        Some("use an explicit `as` coercion to choose one borrowed receiver type".to_string());
    diagnostic
}

fn bounded_method_candidates<'a>(
    self_type: &Type,
    member: &MemberExpr,
    environment: &TypeEnvironment,
    resolved: &'a ResolveOutput,
) -> Vec<(&'a TypeSymbol, &'a MethodSignature)> {
    interface_symbols_for_constrained_type(self_type, environment, resolved)
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
        || environment.get(name).is_some_and(|ty| {
            matches!(
                ty,
                Type::Borrow {
                    is_readwrite: true,
                    ..
                }
            )
        })
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
    super::builtin_types::symbol_for_type(ty, resolved)
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
        Type::Str => Type::StrData,
        Type::View { element, .. } => Type::ArrayData {
            element: element.clone(),
        },
        Type::Borrow { inner, .. } => inner.as_ref().clone(),
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
    if environment.generic_requirements(name).is_some() {
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

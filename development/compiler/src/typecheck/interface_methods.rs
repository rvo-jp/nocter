//! Deterministic selection of methods owned by explicit interface conformances.

use super::conformance::implemented_interface_conformances;
use super::model::Type;
use super::type_expr::{infer_type_expr_substitutions, type_expr_to_type};
use super::visibility::member_visibility_is_accessible;
use crate::ast::TypeExpr;
use crate::resolve::{
    InterfaceConformance, MethodSignature, ResolveOutput, TypeSymbol, TypeSymbolKind,
};
use crate::source::{ByteSpan, SourceId};
use std::collections::{HashMap, HashSet};

pub(crate) struct InterfaceMethodCompletionCandidate<'a> {
    pub(crate) method: &'a MethodSignature,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
}

pub(super) fn candidates<'a>(
    receiver: &Type,
    name: &str,
    use_source: SourceId,
    resolved: &'a ResolveOutput,
) -> Vec<(&'a TypeSymbol, &'a MethodSignature)> {
    method_candidates(receiver, Some(name), use_source, resolved)
        .into_iter()
        .map(|candidate| (candidate.dispatch_owner, candidate.method))
        .collect()
}

pub(super) fn implementation_for_interface<'a>(
    receiver: &Type,
    interface_canonical_name: &str,
    method_name: &str,
    resolved: &'a ResolveOutput,
) -> Option<&'a MethodSignature> {
    let mut candidates = interface_method_candidates(receiver, resolved)
        .into_iter()
        .filter(|candidate| {
            candidate.contract_owner.canonical_name == interface_canonical_name
                && candidate.method.name == method_name
        });
    let candidate = candidates.next()?;
    candidates.next().is_none().then_some(candidate.method)
}

pub(crate) fn implementation_for_interface_type_expr<'a>(
    receiver: &TypeExpr,
    interface_canonical_name: &str,
    method_name: &str,
    resolved: &'a ResolveOutput,
) -> Option<&'a MethodSignature> {
    implementation_for_interface(
        &type_expr_to_type(receiver, resolved),
        interface_canonical_name,
        method_name,
        resolved,
    )
}

pub(crate) fn completion_candidates_for_type_expr<'a>(
    receiver: &TypeExpr,
    use_source: SourceId,
    resolved: &'a ResolveOutput,
) -> Vec<InterfaceMethodCompletionCandidate<'a>> {
    let receiver_type = type_expr_to_type(receiver, resolved);
    method_candidates(&receiver_type, None, use_source, resolved)
        .into_iter()
        .filter_map(|candidate| {
            let mut substitutions = HashMap::from([("Self".to_string(), receiver.clone())]);
            if let Type::Generic { arguments, .. } = &candidate.interface_type {
                for (parameter, argument) in candidate
                    .contract_owner
                    .generic_parameters
                    .iter()
                    .zip(arguments)
                {
                    substitutions.insert(
                        parameter.clone(),
                        type_to_type_expr(argument, receiver.span())?,
                    );
                }
            }
            substitutions.extend(conformance_type_expr_substitutions(
                candidate.conformance,
                &receiver_type,
                resolved,
                receiver.span(),
            )?);
            Some(InterfaceMethodCompletionCandidate {
                method: candidate.method,
                substitutions,
            })
        })
        .collect()
}

struct InterfaceMethodCandidate<'a> {
    contract_owner: &'a TypeSymbol,
    dispatch_owner: &'a TypeSymbol,
    conformance: &'a InterfaceConformance,
    contract: &'a MethodSignature,
    method: &'a MethodSignature,
    interface_type: Type,
}

fn method_candidates<'a>(
    receiver: &Type,
    name: Option<&str>,
    use_source: SourceId,
    resolved: &'a ResolveOutput,
) -> Vec<InterfaceMethodCandidate<'a>> {
    let mut candidates = interface_method_candidates(receiver, resolved)
        .into_iter()
        .filter(|candidate| {
            name.is_none_or(|name| candidate.method.name == name)
                && candidate.contract.is_accessible
                && member_visibility_is_accessible(
                    candidate.contract.visibility,
                    candidate.contract.name_span,
                    use_source,
                    resolved,
                )
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| {
        (
            candidate.contract_owner.canonical_name.as_str(),
            candidate.method.name_span.source.raw(),
            candidate.method.name_span.start,
        )
    });
    candidates.dedup_by_key(|candidate| candidate.method.name_span);
    candidates
}

fn interface_method_candidates<'a>(
    receiver: &Type,
    resolved: &'a ResolveOutput,
) -> Vec<InterfaceMethodCandidate<'a>> {
    let Some(receiver_owner) = receiver
        .nominal_name()
        .and_then(|name| resolved.type_symbol_by_canonical_name(name))
    else {
        return Vec::new();
    };
    implemented_interface_conformances(receiver, resolved)
        .into_iter()
        .filter_map(|(conformance, interface_type)| {
            let contract_owner =
                resolved.type_symbol_by_canonical_name(interface_type.nominal_name()?)?;
            (contract_owner.kind == TypeSymbolKind::Interface).then_some((
                contract_owner,
                conformance,
                interface_type,
            ))
        })
        .flat_map(|(contract_owner, conformance, interface_type)| {
            contract_owner.methods.iter().filter_map(move |contract| {
                let implementation = conformance
                    .methods
                    .iter()
                    .find(|method| method.name == contract.name);
                let method =
                    implementation.or_else(|| contract.has_default_body.then_some(contract))?;
                Some(InterfaceMethodCandidate {
                    contract_owner,
                    dispatch_owner: if implementation.is_some() {
                        receiver_owner
                    } else {
                        contract_owner
                    },
                    conformance,
                    contract,
                    method,
                    interface_type: interface_type.clone(),
                })
            })
        })
        .collect()
}

fn conformance_type_expr_substitutions(
    conformance: &InterfaceConformance,
    receiver: &Type,
    resolved: &ResolveOutput,
    span: ByteSpan,
) -> Option<HashMap<String, TypeExpr>> {
    let parameters = conformance
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut types = HashMap::new();
    infer_type_expr_substitutions(
        &conformance.target_ty,
        receiver,
        resolved,
        None,
        &parameters,
        &mut types,
    );
    types
        .into_iter()
        .map(|(name, ty)| type_to_type_expr(&ty, span).map(|ty| (name, ty)))
        .collect()
}

fn type_to_type_expr(ty: &Type, span: ByteSpan) -> Option<TypeExpr> {
    let mut free_parameters = HashSet::new();
    super::facts::type_to_type_expr_allowing_parameters(ty, span, &mut free_parameters)
}

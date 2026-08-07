use super::{InputId, StorageOrigin, ValueProvenance};
use crate::ast::{
    MethodDecl, MethodReceiverMode, Parameter, ResultProvenanceClause, ResultProvenanceOriginKind,
    TypeExpr,
};
use crate::resolve::ResolveOutput;
use crate::typecheck::returns::type_expr_contains_borrow_like;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::typecheck) enum ContractOriginError {
    ReceiverOutsideMethod,
    OwnedReceiver,
    UnknownParameter(String),
    NonBorrowLikeParameter(String),
    Duplicate(String),
}

pub(in crate::typecheck) fn result_provenance_contract<'a>(
    clause: &'a ResultProvenanceClause,
    method: Option<&MethodDecl>,
    parameters: &[Parameter],
    resolved: &ResolveOutput,
) -> Result<ValueProvenance, Vec<(&'a crate::ast::ResultProvenanceOrigin, ContractOriginError)>> {
    let mut origins = Vec::new();
    let mut errors = Vec::new();
    let mut seen = HashSet::new();

    for origin in &clause.origins {
        let label = origin.kind.source_label().to_string();
        if !seen.insert(label.clone()) {
            errors.push((origin, ContractOriginError::Duplicate(label)));
            continue;
        }

        let storage_origin = match &origin.kind {
            ResultProvenanceOriginKind::Receiver => match method {
                None => {
                    errors.push((origin, ContractOriginError::ReceiverOutsideMethod));
                    continue;
                }
                Some(method) if method.receiver.mode == MethodReceiverMode::Owned => {
                    errors.push((origin, ContractOriginError::OwnedReceiver));
                    continue;
                }
                Some(method) => {
                    StorageOrigin::Input(InputId::declared_at(method.receiver.name_span))
                }
            },
            ResultProvenanceOriginKind::Parameter(name) => {
                let Some(parameter) = parameters.iter().find(|parameter| parameter.name == *name)
                else {
                    errors.push((origin, ContractOriginError::UnknownParameter(name.clone())));
                    continue;
                };
                if !type_expression_is_borrow_like(&parameter.ty, resolved) {
                    errors.push((
                        origin,
                        ContractOriginError::NonBorrowLikeParameter(name.clone()),
                    ));
                    continue;
                }
                StorageOrigin::Input(InputId::declared_at(parameter.name_span))
            }
            ResultProvenanceOriginKind::Static => StorageOrigin::Static,
            ResultProvenanceOriginKind::CurrentAllocationContext => {
                StorageOrigin::CurrentAllocationContext
            }
        };
        origins.push(storage_origin);
    }

    if errors.is_empty() {
        Ok(ValueProvenance::Origins(origins))
    } else {
        Err(errors)
    }
}

pub(in crate::typecheck) fn elided_result_provenance_contract(
    method: Option<&MethodDecl>,
    parameters: &[Parameter],
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<ValueProvenance> {
    if !type_expression_is_borrow_like(return_type, resolved) {
        return Some(ValueProvenance::Independent);
    }
    let mut origins = eligible_input_origins(method, parameters, resolved);
    (origins.len() == 1).then(|| ValueProvenance::Origins(vec![origins.remove(0)]))
}

pub(in crate::typecheck) fn eligible_input_origin_count(
    method: Option<&MethodDecl>,
    parameters: &[Parameter],
    resolved: &ResolveOutput,
) -> usize {
    eligible_input_origins(method, parameters, resolved).len()
}

fn eligible_input_origins(
    method: Option<&MethodDecl>,
    parameters: &[Parameter],
    resolved: &ResolveOutput,
) -> Vec<StorageOrigin> {
    let receiver = method
        .filter(|method| method.receiver.mode != MethodReceiverMode::Owned)
        .map(|method| StorageOrigin::Input(InputId::declared_at(method.receiver.name_span)));
    receiver
        .into_iter()
        .chain(
            parameters
                .iter()
                .filter(|parameter| type_expression_is_borrow_like(&parameter.ty, resolved))
                .map(|parameter| StorageOrigin::Input(InputId::declared_at(parameter.name_span))),
        )
        .collect()
}

fn type_expression_is_borrow_like(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    type_expr_contains_borrow_like(ty, resolved, &HashMap::new(), &mut HashSet::new())
}

pub(in crate::typecheck) fn provenance_satisfies_contract(
    actual: &ValueProvenance,
    contract: &ValueProvenance,
) -> bool {
    let mut allowed = Vec::new();
    collect_origins(contract, &mut allowed);
    actual_origins_satisfy(actual, &allowed)
}

fn collect_origins(provenance: &ValueProvenance, origins: &mut Vec<StorageOrigin>) {
    match provenance {
        ValueProvenance::Independent => {}
        ValueProvenance::Origins(items) => origins.extend(items.iter().cloned()),
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            if let Some(fallback) = fallback {
                collect_origins(fallback, origins);
            }
            fields
                .values()
                .for_each(|value| collect_origins(value, origins));
            elements
                .values()
                .for_each(|value| collect_origins(value, origins));
        }
        ValueProvenance::Fallible { success, error } => {
            if let Some(success) = success {
                collect_origins(success, origins);
            }
            if let Some(error) = error {
                collect_origins(error, origins);
            }
        }
    }
}

fn actual_origins_satisfy(actual: &ValueProvenance, allowed: &[StorageOrigin]) -> bool {
    match actual {
        ValueProvenance::Independent => true,
        ValueProvenance::Origins(origins) => origins.iter().all(|origin| match origin {
            StorageOrigin::Allocated(domain) => match domain.allocation_domain() {
                StorageOrigin::Static => true,
                StorageOrigin::CurrentAllocationContext | StorageOrigin::Input(_) => {
                    allowed.contains(domain.allocation_domain())
                }
                StorageOrigin::Scope { .. }
                | StorageOrigin::Region { .. }
                | StorageOrigin::Unknown => false,
                StorageOrigin::Allocated(_) => unreachable!("allocation domains are unwrapped"),
            },
            StorageOrigin::Static => true,
            StorageOrigin::CurrentAllocationContext | StorageOrigin::Input(_) => {
                allowed.contains(origin)
            }
            StorageOrigin::Scope { .. } | StorageOrigin::Region { .. } | StorageOrigin::Unknown => {
                false
            }
        }),
        ValueProvenance::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            fallback
                .as_deref()
                .is_none_or(|value| actual_origins_satisfy(value, allowed))
                && fields
                    .values()
                    .all(|value| actual_origins_satisfy(value, allowed))
                && elements
                    .values()
                    .all(|value| actual_origins_satisfy(value, allowed))
        }
        ValueProvenance::Fallible { success, error } => {
            success
                .as_deref()
                .is_none_or(|value| actual_origins_satisfy(value, allowed))
                && error
                    .as_deref()
                    .is_none_or(|value| actual_origins_satisfy(value, allowed))
        }
    }
}

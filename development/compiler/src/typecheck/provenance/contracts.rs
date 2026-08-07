use super::{InputId, StorageOrigin, ValueProvenance};
use crate::ast::{
    MethodDecl, Parameter, ResultProvenanceClause, ResultProvenanceOriginKind, TypeExpr,
};
use crate::resolve::ResolveOutput;
use crate::typecheck::model::Type;
use crate::typecheck::provenance::type_may_carry_result_provenance;
use crate::typecheck::type_expr::type_expr_to_type_with_substitutions;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::typecheck) enum ContractOriginError {
    ReceiverOutsideMethod,
    UnknownParameter(String),
    NonStorageCarryingParameter(String),
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
                if !type_expression_may_carry_result_provenance(&parameter.ty, resolved) {
                    errors.push((
                        origin,
                        ContractOriginError::NonStorageCarryingParameter(name.clone()),
                    ));
                    continue;
                }
                StorageOrigin::Input(InputId::declared_at(parameter.name_span))
            }
            ResultProvenanceOriginKind::Static => StorageOrigin::Static,
        };
        origins.push(storage_origin);
    }

    if errors.is_empty() {
        Ok(ValueProvenance::Origins(origins))
    } else {
        Err(errors)
    }
}

fn type_expression_may_carry_result_provenance(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    let ty = type_expr_to_type_with_substitutions(ty, resolved, None, &HashMap::new());
    type_may_carry_result_provenance(&ty, resolved)
        || crate::typecheck::allocation::allocator_capability_kind(&ty, resolved).is_some()
        || matches!(ty, Type::Parameter(_) | Type::Unresolved(_) | Type::Unknown)
}

pub(in crate::typecheck) fn provenance_satisfies_contract(
    actual: &ValueProvenance,
    contract: &ValueProvenance,
    return_type: &Type,
    resolved: &ResolveOutput,
) -> bool {
    let mut allowed = Vec::new();
    collect_origins(contract, &mut allowed);
    super::storage_projection::external_origins_satisfy(actual, &allowed, return_type, resolved)
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

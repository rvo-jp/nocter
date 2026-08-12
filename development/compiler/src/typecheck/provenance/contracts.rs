use super::{InputId, StorageOrigin, ValueProvenance};
use crate::ast::{
    LiteralCapture, LiteralDecl, Parameter, ResultProvenanceClause, ResultProvenanceOriginKind,
    TypeExpr,
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

#[derive(Clone, Copy)]
pub(in crate::typecheck) struct ResultProvenanceInputs<'a> {
    parameters: &'a [Parameter],
    literal_capture: Option<&'a LiteralCapture>,
}

impl<'a> ResultProvenanceInputs<'a> {
    pub(in crate::typecheck) fn parameters(parameters: &'a [Parameter]) -> Self {
        Self {
            parameters,
            literal_capture: None,
        }
    }

    pub(in crate::typecheck) fn literal(literal: &'a LiteralDecl) -> Self {
        Self {
            parameters: &literal.parameters.parameters,
            literal_capture: literal.capture.as_ref(),
        }
    }

    fn find(self, name: &str) -> Option<ProvenanceInput<'a>> {
        if let Some(parameter) = self
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
        {
            return Some(ProvenanceInput {
                name_span: parameter.name_span,
                ty: &parameter.ty,
            });
        }
        self.literal_capture
            .filter(|capture| capture.name == name)
            .map(|capture| ProvenanceInput {
                name_span: capture.name_span,
                ty: &capture.element_type,
            })
    }

    pub(in crate::typecheck) fn elision_inputs(self) -> Vec<ElisionInput<'a>> {
        let mut inputs = self
            .parameters
            .iter()
            .map(|parameter| ElisionInput {
                label: parameter.name.as_str(),
                name_span: parameter.name_span,
                ty: &parameter.ty,
            })
            .collect::<Vec<_>>();
        if let Some(capture) = self.literal_capture {
            inputs.push(ElisionInput {
                label: capture.name.as_str(),
                name_span: capture.name_span,
                ty: &capture.element_type,
            });
        }
        inputs
    }
}

#[derive(Clone, Copy)]
pub(in crate::typecheck) struct ElisionInput<'a> {
    pub(in crate::typecheck) label: &'a str,
    pub(in crate::typecheck) name_span: crate::source::ByteSpan,
    pub(in crate::typecheck) ty: &'a TypeExpr,
}

#[derive(Clone, Copy)]
struct ProvenanceInput<'a> {
    name_span: crate::source::ByteSpan,
    ty: &'a TypeExpr,
}

pub(in crate::typecheck) fn result_provenance_contract<'a>(
    clause: &'a ResultProvenanceClause,
    method: Option<&crate::ast::CallableDecl>,
    inputs: ResultProvenanceInputs<'_>,
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
                    StorageOrigin::Input(InputId::resolved_at(resolved, method.receiver.name_span))
                }
            },
            ResultProvenanceOriginKind::Parameter(name) => {
                let Some(input) = inputs.find(name) else {
                    errors.push((origin, ContractOriginError::UnknownParameter(name.clone())));
                    continue;
                };
                if !type_expression_may_carry_result_provenance(input.ty, resolved) {
                    errors.push((
                        origin,
                        ContractOriginError::NonStorageCarryingParameter(name.clone()),
                    ));
                    continue;
                }
                StorageOrigin::Input(InputId::resolved_at(resolved, input.name_span))
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

pub(in crate::typecheck) fn result_provenance_contract_for_signature(
    clause: &ResultProvenanceClause,
    parameters: &[crate::resolve::ParameterSignature],
    resolved: &ResolveOutput,
) -> Option<ValueProvenance> {
    let mut origins = Vec::new();
    for origin in &clause.origins {
        let storage_origin = match &origin.kind {
            ResultProvenanceOriginKind::Parameter(name) => {
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.name == *name)?;
                StorageOrigin::Input(InputId::resolved_at(resolved, parameter.name_span))
            }
            ResultProvenanceOriginKind::Static => StorageOrigin::Static,
            ResultProvenanceOriginKind::Receiver => return None,
        };
        if !origins.contains(&storage_origin) {
            origins.push(storage_origin);
        }
    }
    Some(ValueProvenance::Origins(origins))
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
    // `from` describes the callable's usable result, not the transport value
    // used to propagate a recoverable failure. Error storage is tracked for
    // escape safety, but it is not an origin candidate and must not force a
    // source clause onto every fallible API.
    match (actual, return_type) {
        (
            ValueProvenance::Fallible { success, .. },
            Type::Fallible {
                success: success_type,
                ..
            },
        ) => success.as_deref().is_none_or(|success| {
            super::storage_projection::external_origins_satisfy(
                success,
                &allowed,
                success_type,
                resolved,
            )
        }),
        _ => super::storage_projection::external_origins_satisfy(
            actual,
            &allowed,
            return_type,
            resolved,
        ),
    }
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

//! Deterministic selection of reusable interface method bodies.

use super::interface_bounds::implemented_interface_types;
use super::model::Type;
use super::type_expr::type_expr_to_type;
use super::visibility::member_visibility_is_accessible;
use crate::ast::TypeExpr;
use crate::resolve::{MethodSignature, ResolveOutput, TypeSymbol, TypeSymbolKind};
use crate::source::{ByteSpan, SourceId};
use std::collections::{HashMap, HashSet};

pub(crate) struct DefaultMethodCompletionCandidate<'a> {
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
        .map(|(owner, method, _)| (owner, method))
        .collect()
}

pub(crate) fn completion_candidates_for_type_expr<'a>(
    receiver: &TypeExpr,
    use_source: SourceId,
    resolved: &'a ResolveOutput,
) -> Vec<DefaultMethodCompletionCandidate<'a>> {
    let receiver_type = type_expr_to_type(receiver, resolved);
    method_candidates(&receiver_type, None, use_source, resolved)
        .into_iter()
        .filter_map(|(owner, method, interface_type)| {
            let mut substitutions = HashMap::from([("Self".to_string(), receiver.clone())]);
            if let Type::Generic { arguments, .. } = interface_type {
                for (parameter, argument) in owner.generic_parameters.iter().zip(arguments) {
                    substitutions.insert(
                        parameter.clone(),
                        type_to_type_expr(&argument, receiver.span())?,
                    );
                }
            }
            Some(DefaultMethodCompletionCandidate {
                method,
                substitutions,
            })
        })
        .collect()
}

fn method_candidates<'a>(
    receiver: &Type,
    name: Option<&str>,
    use_source: SourceId,
    resolved: &'a ResolveOutput,
) -> Vec<(&'a TypeSymbol, &'a MethodSignature, Type)> {
    let mut candidates = implemented_interface_types(receiver, resolved)
        .into_iter()
        .filter_map(|interface_type| {
            let owner = resolved.type_symbol_by_canonical_name(interface_type.nominal_name()?)?;
            (owner.kind == TypeSymbolKind::Interface).then_some((owner, interface_type))
        })
        .flat_map(|(owner, interface_type)| {
            owner.methods.iter().filter_map(move |method| {
                (method.has_default_body
                    && name.is_none_or(|name| method.name == name)
                    && method.is_accessible
                    && member_visibility_is_accessible(
                        method.visibility,
                        method.name_span,
                        use_source,
                        resolved,
                    ))
                .then_some((owner, method, interface_type.clone()))
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(owner, method, _)| {
        (
            owner.canonical_name.as_str(),
            method.name_span.source.raw(),
            method.name_span.start,
        )
    });
    candidates.dedup_by_key(|(_, method, _)| method.name_span);
    candidates
}

fn type_to_type_expr(ty: &Type, span: ByteSpan) -> Option<TypeExpr> {
    let mut free_parameters = HashSet::new();
    super::facts::type_to_type_expr_allowing_parameters(ty, span, &mut free_parameters)
}

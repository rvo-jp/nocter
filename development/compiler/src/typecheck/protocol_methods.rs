//! Resolved static method plans for exact interface contracts.

use super::facts::{TypecheckProtocolMethod, type_to_type_expr_allowing_parameters};
use super::interface_bounds::interface_symbols_for_constrained_type;
use super::interface_methods::conformance_method_for_interface;
use super::model::{Type, TypeEnvironment};
use crate::resolve::{MethodSignature, ResolveOutput};
use crate::source::ByteSpan;
use std::collections::HashSet;

pub(super) fn resolved_protocol_method(
    receiver: &Type,
    interface_canonical_name: &str,
    method_name: &str,
    span: ByteSpan,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<TypecheckProtocolMethod> {
    let method = if matches!(receiver, Type::Parameter(_) | Type::Projection { .. }) {
        interface_symbols_for_constrained_type(receiver, environment, resolved)
            .into_iter()
            .find(|(interface, _)| interface.canonical_name == interface_canonical_name)
            .and_then(|(interface, _)| {
                interface
                    .methods
                    .iter()
                    .find(|method| method.name == method_name)
            })?
    } else {
        conformance_method_for_interface(receiver, interface_canonical_name, method_name, resolved)?
    };
    protocol_method_fact(receiver, method, span, resolved)
}

fn protocol_method_fact(
    receiver: &Type,
    method: &MethodSignature,
    span: ByteSpan,
    resolved: &ResolveOutput,
) -> Option<TypecheckProtocolMethod> {
    let mut free_type_parameters = HashSet::new();
    let self_ty = type_to_type_expr_allowing_parameters(
        receiver.opaque_lowering_view(),
        span,
        &mut free_type_parameters,
    )?;
    Some(TypecheckProtocolMethod::new(
        resolved
            .semantic_db
            .definition_at(method.name_span)
            .expect("resolved protocol method must have a semantic definition"),
        method.name_span,
        format!("{}.{}", receiver.display(), method.name),
        self_ty,
        method.receiver.mode,
        method.name.clone(),
        free_type_parameters,
    ))
}

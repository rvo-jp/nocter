//! Canonical expansion of authored callable parameters.
//!
//! Receivers are implicit in source syntax but ordinary parameters in MIR and
//! the native ABI. Keeping their expansion here prevents buildability and the
//! backend from constructing different callable contracts.

use crate::ast::{CallableDecl, Parameter, TypeExpr, substitute_type_expr_parameters};
use std::collections::HashMap;

pub(crate) fn function(
    parameters: &[Parameter],
    substitutions: &HashMap<String, TypeExpr>,
) -> Vec<Parameter> {
    parameters
        .iter()
        .cloned()
        .map(|mut parameter| {
            parameter.ty = substitute_type_expr_parameters(&parameter.ty, substitutions);
            parameter
        })
        .collect()
}

pub(crate) fn instance(
    callable: &CallableDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> Vec<Parameter> {
    let mut context = substitutions.clone();
    context.insert("Self".to_string(), self_ty.clone());
    let mut parameters = Vec::with_capacity(callable.parameters.parameters.len() + 1);
    let mut receiver = callable.receiver.implicit_parameter();
    receiver.ty = substitute_type_expr_parameters(&receiver.ty, &context);
    parameters.push(receiver);
    parameters.extend(function(&callable.parameters.parameters, &context));
    parameters
}

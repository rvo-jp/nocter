//! Closure-local type environments.

use super::model::{Type, TypeEnvironment};
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{ClosureCaptureMode, ClosureExpr};
use crate::resolve::ResolveOutput;

/// Builds the only value environment visible while checking a closure body.
/// Captures retain the source value type; their access capability is tracked by
/// the capture symbol and ownership plan rather than encoded as an extra
/// user-visible dereference layer.
pub(super) fn environment_for_closure(
    closure: &ClosureExpr,
    resolved: &ResolveOutput,
    outer: &TypeEnvironment,
) -> TypeEnvironment {
    let mut environment = outer.nested_callable_scope();
    for capture in &closure.captures {
        let ty = outer.get(&capture.name).cloned().unwrap_or(Type::Unknown);
        environment.define_binding(
            capture.name.clone(),
            ty,
            capture.mode == ClosureCaptureMode::ReadwriteBorrow,
        );
    }
    for parameter in &closure.parameters {
        let ty = parameter
            .ty
            .as_ref()
            .map(|ty| type_expr_to_type_in_environment(ty, resolved, &environment))
            .unwrap_or(Type::Unknown);
        environment.define(parameter.name.clone(), ty);
    }
    environment
}

//! Structural resolution for direct calls through callable values.

use super::calls::{CheckedCallKind, CheckedCallSignature, check_known_function_call};
use super::expressions::expression_type;
use super::facts::type_to_type_expr_allowing_parameters;
use super::model::{Type, TypeEnvironment};
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{CallExpr, CallableCapability, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::resolve::{FunctionSignature, ParameterSignature, ResolveOutput};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub(super) struct ResolvedCallableContract {
    pub(super) capability: CallableCapability,
    pub(super) callee_type: Type,
    pub(super) return_type: Type,
    pub(super) signature: FunctionSignature,
}

pub(super) fn callable_contract_for_call(
    call: &CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<ResolvedCallableContract> {
    let callee_type = expression_type(&call.callee, resolved, environment);
    callable_contract_for_type(&callee_type, call.span, resolved, environment)
}

pub(super) fn consuming_callable_identifier<'a>(
    call: &'a CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<&'a crate::ast::IdentifierExpr> {
    let contract = callable_contract_for_call(call, resolved, environment)?;
    if contract.capability != CallableCapability::Consuming {
        return None;
    }
    callable_identifier(&call.callee)
}

fn callable_identifier(expression: &crate::ast::Expr) -> Option<&crate::ast::IdentifierExpr> {
    match expression {
        crate::ast::Expr::Identifier(identifier) => Some(identifier),
        crate::ast::Expr::Group(group) => callable_identifier(&group.expression),
        _ => None,
    }
}

fn callable_contract_for_type(
    callee_type: &Type,
    fallback_span: ByteSpan,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<ResolvedCallableContract> {
    match callee_type {
        Type::Closure(closure) => callable_contract(
            closure.capability,
            callee_type.clone(),
            type_expr_to_type_in_environment(&closure.return_type, resolved, environment),
            closure
                .parameters
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, ty)| ParameterSignature {
                    name: format!("argument{index}"),
                    name_span: fallback_span,
                    ty,
                })
                .collect(),
            (*closure.return_type).clone(),
            None,
        ),
        Type::Parameter(name) | Type::Named(name) => {
            let requirements = environment.generic_requirements(name)?;
            let mut callables = requirements.callable_bounds().filter_map(|bound| {
                let TypeExpr::Callable(callable) = bound else {
                    return None;
                };
                Some(callable)
            });
            let callable = callables.next()?;
            if callables.next().is_some() {
                return None;
            }
            let parameter_types = callable
                .parameters
                .iter()
                .map(|parameter| {
                    type_expr_to_type_in_environment(&parameter.ty, resolved, environment)
                })
                .collect::<Vec<_>>();
            let return_type =
                type_expr_to_type_in_environment(&callable.return_type, resolved, environment);
            let mut free = HashSet::new();
            let parameter_type_exprs = parameter_types
                .iter()
                .map(|ty| type_to_type_expr_allowing_parameters(ty, fallback_span, &mut free))
                .collect::<Option<Vec<_>>>()?;
            let parameters = callable
                .parameters
                .iter()
                .zip(parameter_type_exprs)
                .enumerate()
                .map(|(index, (parameter, ty))| ParameterSignature {
                    name: parameter
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("argument{index}")),
                    name_span: parameter.name_span.unwrap_or(parameter.span),
                    ty,
                })
                .collect();
            let return_type_expr =
                type_to_type_expr_allowing_parameters(&return_type, fallback_span, &mut free)?;
            callable_contract(
                callable.capability,
                callee_type.clone(),
                return_type,
                parameters,
                return_type_expr,
                callable.result_provenance.clone(),
            )
        }
        _ => None,
    }
}

fn callable_contract(
    capability: CallableCapability,
    callee_type: Type,
    return_type: Type,
    parameters: Vec<ParameterSignature>,
    return_type_expr: TypeExpr,
    result_provenance: Option<crate::ast::ResultProvenanceClause>,
) -> Option<ResolvedCallableContract> {
    Some(ResolvedCallableContract {
        capability,
        callee_type,
        return_type,
        signature: FunctionSignature {
            generic_parameters: Vec::new(),
            generic_parameter_requirements: Vec::new(),
            where_clause: None,
            parameters,
            return_type: return_type_expr,
            result_provenance,
        },
    })
}

pub(super) fn check_callable_call(
    sources: &SourceMap,
    call: &CallExpr,
    contract: &ResolvedCallableContract,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let signature = CheckedCallSignature {
        signature: &contract.signature,
        self_type: None,
        owner_target_ty: None,
        name: contract.callee_type.display(),
        kind: CheckedCallKind::Function,
        declaration_span: None,
    };
    check_known_function_call(
        sources,
        call,
        &signature,
        resolved,
        diagnostics,
        environment,
    );

    if contract.capability == CallableCapability::Readwrite
        && !super::places::expression_is_writable_place(&call.callee, resolved, environment)
    {
        diagnostics.push(
            super::diagnostics::callable_readwrite_requires_writable_diagnostic(
                sources,
                call.callee.span(),
            ),
        );
    }
}

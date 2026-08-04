//! Closure-local type environments.

use super::expressions::block_result_type;
use super::facts::type_to_type_expr_allowing_parameters;
use super::interface_bounds::interface_symbol_for_bound;
use super::model::{Type, TypeEnvironment};
use super::type_expr::{
    infer_type_expr_substitutions, type_expr_to_type_in_environment,
    type_expr_to_type_with_substitutions,
};
use crate::ast::{
    BorrowType, ClosureCallableCapability, ClosureCaptureMode, ClosureCaptureType, ClosureExpr,
    ClosureTypeExpr, Expr, Stmt, TypeExpr, UnaryOperator,
};
use crate::resolve::FunctionSignature;
use crate::resolve::ResolveOutput;
use std::collections::{HashMap, HashSet};

pub(super) struct ExpectedCallableContract<'a> {
    pub(super) bound: &'a TypeExpr,
    pub(super) parameters: Vec<Type>,
    pub(super) return_type: Type,
}

pub(super) fn expected_callable_contract_for_generic<'a>(
    generic: &str,
    signature: &'a FunctionSignature,
    substitutions: &HashMap<String, Type>,
    resolved: &ResolveOutput,
) -> Option<ExpectedCallableContract<'a>> {
    let index = signature
        .generic_parameters
        .iter()
        .position(|parameter| parameter == generic)?;
    let runtime = resolved.trusted_declarations.callable_runtime()?;
    signature
        .generic_parameter_bounds
        .get(index)?
        .iter()
        .find_map(|bound| {
            let (interface, bound_type) =
                interface_symbol_for_bound(bound, substitutions, resolved)?;
            if interface.canonical_name != runtime.readonly.interface_canonical_name
                && interface.canonical_name != runtime.repeated.interface_canonical_name
                && interface.canonical_name != runtime.consuming.interface_canonical_name
            {
                return None;
            }
            let Type::Generic { arguments, .. } = bound_type else {
                return None;
            };
            let [input, output] = arguments.as_slice() else {
                return None;
            };
            Some(ExpectedCallableContract {
                bound,
                parameters: vec![input.clone()],
                return_type: output.clone(),
            })
        })
}

pub(super) fn infer_substitutions_from_closure_contract(
    contract: &ExpectedCallableContract<'_>,
    closure_type: &Type,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    parameters: &HashSet<&str>,
    substitutions: &mut HashMap<String, Type>,
) {
    let Type::Closure(closure) = closure_type else {
        return;
    };
    let TypeExpr::Generic(bound) = contract.bound else {
        return;
    };
    let actual = closure
        .parameters
        .iter()
        .map(|ty| type_expr_to_type_with_substitutions(ty, resolved, self_type, substitutions))
        .chain(std::iter::once(type_expr_to_type_with_substitutions(
            &closure.return_type,
            resolved,
            self_type,
            substitutions,
        )))
        .collect::<Vec<_>>();
    for (expected, actual) in bound.arguments.iter().zip(actual) {
        infer_type_expr_substitutions(
            expected,
            &actual,
            resolved,
            self_type,
            parameters,
            substitutions,
        );
    }
}

pub(super) fn closure_satisfies_callable_bound(
    closure: &ClosureTypeExpr,
    bound: &Type,
    resolved: &ResolveOutput,
) -> bool {
    let Some(name) = bound.nominal_name() else {
        return false;
    };
    let Some(runtime) = resolved.trusted_declarations.callable_runtime() else {
        return false;
    };
    let expected = if name == runtime.readonly.interface_canonical_name {
        ClosureCallableCapability::Readonly
    } else if name == runtime.repeated.interface_canonical_name {
        ClosureCallableCapability::Readwrite
    } else if name == runtime.consuming.interface_canonical_name {
        ClosureCallableCapability::Consuming
    } else {
        return false;
    };
    let capability_matches = match expected {
        ClosureCallableCapability::Readonly => {
            closure.capability == ClosureCallableCapability::Readonly
        }
        ClosureCallableCapability::Readwrite => {
            closure.capability <= ClosureCallableCapability::Readwrite
        }
        ClosureCallableCapability::Consuming => true,
    };
    if !capability_matches {
        return false;
    }
    let Type::Generic { arguments, .. } = bound else {
        return false;
    };
    let [expected_input, expected_output] = arguments.as_slice() else {
        return false;
    };
    let [actual_input] = closure.parameters.as_slice() else {
        return false;
    };
    type_expr_to_type_with_substitutions(actual_input, resolved, None, &HashMap::new())
        == *expected_input
        && type_expr_to_type_with_substitutions(
            &closure.return_type,
            resolved,
            None,
            &HashMap::new(),
        ) == *expected_output
}

pub(super) fn callable_bound_capability(
    bound: &Type,
    resolved: &ResolveOutput,
) -> Option<ClosureCallableCapability> {
    let name = bound.nominal_name()?;
    let runtime = resolved.trusted_declarations.callable_runtime()?;
    if name == runtime.readonly.interface_canonical_name {
        Some(ClosureCallableCapability::Readonly)
    } else if name == runtime.repeated.interface_canonical_name {
        Some(ClosureCallableCapability::Readwrite)
    } else if name == runtime.consuming.interface_canonical_name {
        Some(ClosureCallableCapability::Consuming)
    } else {
        None
    }
}

/// Builds the only value environment visible while checking a closure body.
/// Captures retain the source value type; their access capability is tracked by
/// the capture symbol and ownership plan rather than encoded as an extra
/// user-visible dereference layer.
pub(super) fn environment_for_closure(
    closure: &ClosureExpr,
    resolved: &ResolveOutput,
    outer: &TypeEnvironment,
) -> TypeEnvironment {
    let parameter_types = closure
        .parameters
        .iter()
        .map(|parameter| {
            parameter
                .ty
                .as_ref()
                .map(|ty| type_expr_to_type_in_environment(ty, resolved, outer))
                .unwrap_or(Type::Unknown)
        })
        .collect::<Vec<_>>();
    environment_for_closure_with_parameters(closure, resolved, outer, &parameter_types)
}

pub(super) fn environment_for_closure_with_parameters(
    closure: &ClosureExpr,
    _resolved: &ResolveOutput,
    outer: &TypeEnvironment,
    parameter_types: &[Type],
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
    for (parameter, ty) in closure.parameters.iter().zip(parameter_types) {
        environment.define(parameter.name.clone(), ty.clone());
    }
    environment
}

pub(super) fn infer_closure_type(
    closure: &ClosureExpr,
    resolved: &ResolveOutput,
    outer: &TypeEnvironment,
    expected_parameters: Option<&[Type]>,
    expected_return: Option<&Type>,
) -> Option<Type> {
    if let Some(expected) = expected_parameters
        && expected.len() != closure.parameters.len()
    {
        return None;
    }
    let parameter_types = closure
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            parameter
                .ty
                .as_ref()
                .map(|ty| type_expr_to_type_in_environment(ty, resolved, outer))
                .or_else(|| expected_parameters.and_then(|types| types.get(index).cloned()))
        })
        .collect::<Option<Vec<_>>>()?;
    let closure_environment =
        environment_for_closure_with_parameters(closure, resolved, outer, &parameter_types);
    let return_type = closure
        .return_type
        .as_ref()
        .map(|ty| type_expr_to_type_in_environment(ty, resolved, &closure_environment))
        .or_else(|| {
            let inferred = block_result_type(&closure.body, resolved, &closure_environment);
            (!inferred.is_unknown_or_unresolved()).then_some(inferred)
        })
        .or_else(|| expected_return.cloned())?;

    let mut free_parameters = HashSet::new();
    let parameter_type_exprs = parameter_types
        .iter()
        .map(|ty| {
            type_to_type_expr_allowing_parameters(ty, closure.parameters_span, &mut free_parameters)
        })
        .collect::<Option<Vec<_>>>()?;
    let return_type_expr = type_to_type_expr_allowing_parameters(
        &return_type,
        closure
            .return_type
            .as_ref()
            .map_or(closure.body.span, TypeExpr::span),
        &mut free_parameters,
    )?;
    let captures = closure
        .captures
        .iter()
        .map(|capture| {
            let source_ty = outer.get(&capture.name)?;
            let source_ty = type_to_type_expr_allowing_parameters(
                source_ty,
                capture.name_span,
                &mut free_parameters,
            )?;
            let ty = match capture.mode {
                ClosureCaptureMode::Move => source_ty,
                ClosureCaptureMode::ReadonlyBorrow | ClosureCaptureMode::ReadwriteBorrow => {
                    TypeExpr::Borrow(BorrowType {
                        span: capture.span,
                        is_readwrite: capture.mode == ClosureCaptureMode::ReadwriteBorrow,
                        inner: Box::new(source_ty),
                    })
                }
            };
            Some(ClosureCaptureType {
                name: capture.name.clone(),
                mode: capture.mode,
                ty,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Type::Closure(ClosureTypeExpr {
        span: closure.span,
        captures,
        parameters: parameter_type_exprs,
        return_type: Box::new(return_type_expr),
        capability: closure_capability(closure),
    }))
}

fn closure_capability(closure: &ClosureExpr) -> ClosureCallableCapability {
    let captures = closure
        .captures
        .iter()
        .map(|capture| capture.name.as_str())
        .collect::<HashSet<_>>();
    if block_moves_capture(&closure.body, &captures) {
        ClosureCallableCapability::Consuming
    } else if closure
        .captures
        .iter()
        .any(|capture| capture.mode == ClosureCaptureMode::ReadwriteBorrow)
        || block_mutates_capture(&closure.body, &captures)
    {
        ClosureCallableCapability::Readwrite
    } else {
        ClosureCallableCapability::Readonly
    }
}

fn block_moves_capture(block: &crate::ast::Block, captures: &HashSet<&str>) -> bool {
    let mut found = false;
    crate::ast::visit_block_expressions_without_nested_closures(block, &mut |expression| {
        if matches!(
            expression,
            Expr::Unary(unary)
                if unary.operator == UnaryOperator::Move
                    && matches!(unary.operand.without_groups(), Expr::Identifier(identifier) if captures.contains(identifier.name.as_str()))
        ) {
            found = true;
        }
    });
    found
}

fn block_mutates_capture(block: &crate::ast::Block, captures: &HashSet<&str>) -> bool {
    block.statements.iter().any(|statement| match statement {
        Stmt::Assignment(assignment) => {
            assignment_target_root(&assignment.target).is_some_and(|name| captures.contains(name))
        }
        Stmt::If(statement) => {
            block_mutates_capture(&statement.then_block, captures)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_mutates_capture(block, captures))
        }
        Stmt::IfIs(statement) => {
            block_mutates_capture(&statement.then_block, captures)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_mutates_capture(block, captures))
        }
        Stmt::Switch(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_mutates_capture(&arm.body, captures))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_mutates_capture(&arm.body, captures))
        }
        Stmt::While(statement) => block_mutates_capture(&statement.body, captures),
        Stmt::Loop(statement) => block_mutates_capture(&statement.body, captures),
        Stmt::ForRange(statement) => block_mutates_capture(&statement.body, captures),
        Stmt::CollectionFor(statement) => block_mutates_capture(&statement.body, captures),
        Stmt::LiteralPackFor(statement) => block_mutates_capture(&statement.body, captures),
        Stmt::Region(statement) => block_mutates_capture(&statement.body, captures),
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Return(_)
        | Stmt::Binding(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_)
        | Stmt::Expression(_) => false,
    })
}

fn assignment_target_root(expression: &Expr) -> Option<&str> {
    match expression.without_groups() {
        Expr::Identifier(identifier) => Some(&identifier.name),
        Expr::Member(member) => assignment_target_root(&member.object),
        Expr::Index(index) => assignment_target_root(&index.object),
        _ => None,
    }
}

//! Scalar body selection and control-flow construction from typed HIR.

use super::ids::{BasicBlockId, LocalId};
use super::locals::{Local, LocalOrigin, LocalStorage, ScalarType};
use super::model::{Body, ReturnMode, Terminator};
#[cfg(test)]
use super::validate;
use super::validate::ValidationError;
use super::{Scope, ScopeId};
use crate::ast::{Block, Expr, Parameter};
use crate::resolve::ResolveOutput;
use crate::semantic::SemanticDb;
use crate::typecheck::TypedHir;
use std::collections::HashMap;

mod body_builder;
mod coverage;
mod expressions;
mod statements;
use body_builder::ControlFlowBuilder;
use coverage::*;
use expressions::{lower_call, lower_expression_to_place, lower_operand};
use statements::StatementLowerer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildError {
    MissingSourceBody,
    MissingTypedExpression,
    InvalidScalarConstant,
    MissingLocalSymbol,
    MissingParameterType,
    MissingCallTarget,
    MissingOpenBlock,
    OpenBlockNotTerminated,
    BlockAlreadyTerminated,
    UnterminatedReservedBlock,
    UnsupportedClaimedExpression,
    InvalidMir(Vec<ValidationError>),
}

#[cfg(test)]
fn try_build_scalar_body(
    block: &Block,
    parameters: &[Parameter],
    return_scalar: ScalarType,
    semantic_db: &SemanticDb,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> Option<Result<Body, BuildError>> {
    try_build_scalar_body_with_return_mode(
        block,
        parameters,
        return_scalar,
        ReturnMode::Plain,
        semantic_db,
        resolved,
        typed_hir,
    )
}

pub(crate) fn try_build_scalar_body_with_return_mode(
    block: &Block,
    parameters: &[Parameter],
    return_scalar: ScalarType,
    return_mode: ReturnMode,
    semantic_db: &SemanticDb,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> Option<Result<Body, BuildError>> {
    let (source_statements, tail) = scalar_body_parts(block)?;
    if !source_statements
        .iter()
        .all(|statement| statement.is_supported(resolved, typed_hir))
        || !tail.is_supported(resolved, typed_hir)
        || !parameters.iter().all(|parameter| {
            resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .is_some_and(|symbol| binding_scalar_type(symbol, typed_hir).is_some())
                && typed_hir.type_id(&parameter.ty).is_some()
        })
    {
        return None;
    }

    let return_ty = tail.result_type(typed_hir)?;
    if scalar_type(return_ty, typed_hir) != Some(return_scalar) {
        return None;
    }
    Some((|| {
        let source_body = semantic_db
            .body_at(block.span)
            .ok_or(BuildError::MissingSourceBody)?;
        let return_local = LocalId::from_index(0);
        let root_scope = ScopeId::from_index(0);
        let mut scopes = vec![Scope::root(block.span)];
        let mut locals = vec![Local::scalar(
            return_ty,
            return_scalar,
            LocalStorage::Return,
            LocalOrigin::Return,
            root_scope,
        )];
        let mut locals_by_symbol = HashMap::new();
        for (index, parameter) in parameters.iter().enumerate() {
            let ty = typed_hir
                .type_id(&parameter.ty)
                .ok_or(BuildError::MissingParameterType)?;
            let symbol = resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let scalar = binding_scalar_type(symbol, typed_hir)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            let local = LocalId::from_index(locals.len());
            locals.push(Local::scalar(
                ty,
                scalar,
                LocalStorage::Parameter(index),
                LocalOrigin::Parameter(symbol),
                root_scope,
            ));
            locals_by_symbol.insert(symbol, local);
        }
        let mut control_flow = ControlFlowBuilder::new(root_scope);
        let mut loop_regions = Vec::new();
        StatementLowerer::new(
            resolved,
            typed_hir,
            &mut locals,
            &mut locals_by_symbol,
            &mut control_flow,
            &mut loop_regions,
            &mut scopes,
        )
        .lower(&source_statements, root_scope)?;
        if let Some(if_) = tail.conditional() {
            let condition_ty = known_expression_type(&if_.condition, typed_hir)
                .ok_or(BuildError::MissingTypedExpression)?;
            let condition = lower_operand(
                &if_.condition,
                condition_ty,
                ScalarType::Bool,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut control_flow,
                root_scope,
            )?;
            let then_result = scalar_branch_result(&if_.then_block)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            let else_result = if_
                .else_block
                .as_ref()
                .and_then(scalar_branch_result)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            let then_scope = ScopeId::from_index(scopes.len());
            scopes.push(Scope::child(root_scope, if_.then_block.span));
            let else_scope = ScopeId::from_index(scopes.len());
            scopes.push(Scope::child(
                root_scope,
                if_.else_block.as_ref().map_or(if_.span, |block| block.span),
            ));
            let then_target = control_flow.reserve_block(then_scope);
            let else_target = control_flow.reserve_block(else_scope);
            let join_target = control_flow.reserve_block(root_scope);
            control_flow.terminate(Terminator::Switch {
                condition,
                then_target,
                else_target,
            })?;
            control_flow.select_block(then_target)?;
            lower_expression_to_place(
                return_local,
                then_result,
                return_ty,
                return_scalar,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut control_flow,
                then_scope,
            )?;
            control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
            control_flow.select_block(else_target)?;
            lower_expression_to_place(
                return_local,
                else_result,
                return_ty,
                return_scalar,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut control_flow,
                else_scope,
            )?;
            control_flow.terminate(Terminator::Goto {
                target: join_target,
            })?;
            control_flow.select_block(join_target)?;
            control_flow.terminate(Terminator::Return)?;
        } else if let Some(Expr::Call(call)) = tail.expression() {
            let source = typed_hir
                .expression(call.span)
                .ok_or(BuildError::MissingTypedExpression)?
                .id;
            let (callee, arguments, returns_never) = lower_call(
                call,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut control_flow,
                root_scope,
            )?;
            if returns_never {
                control_flow.emit_never_call(source, callee, arguments)?;
            } else {
                control_flow.emit_returning_call(source, callee, arguments, return_local)?;
                control_flow.terminate(Terminator::Return)?;
            }
        } else {
            let expression = tail
                .expression()
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            lower_expression_to_place(
                return_local,
                expression,
                return_ty,
                return_scalar,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut control_flow,
                root_scope,
            )?;
            control_flow.terminate(Terminator::Return)?;
        }
        let blocks = control_flow.finish()?;
        let body = Body {
            source_body,
            source_span: block.span,
            return_local,
            return_mode,
            root_scope,
            scopes,
            locals,
            entry: BasicBlockId::from_index(0),
            blocks,
            loop_regions,
            loans: Vec::new(),
            projections: Vec::new(),
        };
        super::finalize(body).map_err(BuildError::InvalidMir)
    })())
}

#[cfg(test)]
#[path = "lower/tests.rs"]
mod tests;

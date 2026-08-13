//! Scalar body selection and control-flow construction from typed HIR.

use super::ids::{BasicBlockId, LocalId};
use super::model::{Body, Local, LocalSource, ReturnMode, ScalarType, Terminator};
use super::validate;
use super::validate::ValidationError;
use crate::ast::{Block, Expr, Parameter};
use crate::resolve::ResolveOutput;
use crate::semantic::SemanticDb;
use crate::typecheck::TypedHir;
use std::collections::HashMap;

mod body_builder;
mod coverage;
mod expressions;
use body_builder::ControlFlowBuilder;
use coverage::*;
use expressions::{lower_call, lower_expression_to_place, lower_operand};

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
        let mut locals = vec![Local {
            ty: return_ty,
            scalar: return_scalar,
            source: LocalSource::Return,
        }];
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
            locals.push(Local {
                ty,
                scalar,
                source: LocalSource::Parameter { symbol, index },
            });
            locals_by_symbol.insert(symbol, local);
        }
        let mut control_flow = ControlFlowBuilder::new();
        for source_statement in source_statements {
            let (local, value) = match source_statement {
                ScalarStatement::Binding(binding) => {
                    let symbol = resolved
                        .local_symbol_id_at_name_span(binding.name_span)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let ty = typed_hir
                        .binding_type_expr(symbol)
                        .and_then(|ty| typed_hir.type_id(ty))
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let scalar = binding_scalar_type(symbol, typed_hir)
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    let local = LocalId::from_index(locals.len());
                    locals.push(Local {
                        ty,
                        scalar,
                        source: LocalSource::Binding(symbol),
                    });
                    locals_by_symbol.insert(symbol, local);
                    (local, &binding.initializer)
                }
                ScalarStatement::Assignment(assignment) => {
                    let Expr::Identifier(identifier) = &assignment.target else {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    };
                    let symbol = resolved
                        .local_symbol_for_identifier(identifier)
                        .map(|symbol| symbol.id)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    (
                        *locals_by_symbol
                            .get(&symbol)
                            .ok_or(BuildError::MissingLocalSymbol)?,
                        &assignment.value,
                    )
                }
            };
            let destination_local = &locals[local.index()];
            let destination_ty = destination_local.ty;
            let destination_scalar = destination_local.scalar;
            lower_expression_to_place(
                local,
                value,
                destination_ty,
                destination_scalar,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut control_flow,
            )?;
        }
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
            )?;
            let then_result = scalar_branch_result(&if_.then_block)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            let else_result = if_
                .else_block
                .as_ref()
                .and_then(scalar_branch_result)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            let then_target = control_flow.reserve_block();
            let else_target = control_flow.reserve_block();
            let join_target = control_flow.reserve_block();
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
            )?;
            control_flow.terminate(Terminator::Return)?;
        }
        let blocks = control_flow.finish()?;
        let body = Body {
            source_body,
            source_span: block.span,
            return_local,
            return_mode,
            locals,
            entry: BasicBlockId::from_index(0),
            blocks,
        };
        validate(&body).map_err(BuildError::InvalidMir)?;
        Ok(body)
    })())
}

#[cfg(test)]
#[path = "lower/tests.rs"]
mod tests;

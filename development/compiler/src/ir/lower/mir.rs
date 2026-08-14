//! MIR-to-machine-IR lowering. This module grows only after the corresponding
//! AST-driven lowering family has been removed from its production route.

use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgument, AggregateArgumentSource, BoolComparisonOperator, BoolLocation, BoolValue,
    DirectAggregateArgument, I32ComparisonOperator, I32Location, I32Value, Instruction,
    IntegerBinaryOperator, OutcomeFailureMode, ScalarArgument, StrLocation, StrValue, Type,
    U8Location, U8Value, UsizeLocation, UsizeValue,
};
use crate::mir::{
    BinaryOperator, Body, CallContinuation, ComparisonOperator, LocalId, LocalStorage, Operand,
    Place, ReturnMode, Rvalue, ScalarType, Statement, Terminator, UnaryOperator,
};
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::TypedHir;
use std::collections::HashSet;

mod control_flow;
mod drops;
mod loops;
mod outcomes;
mod parameters;
mod storage;

/// Immutable inputs shared by every control-flow structuring path.
///
/// Keeping this as one value prevents branches and loop helpers from growing
/// parallel parameter lists as MIR gains aggregate and borrow projections.
pub(super) struct BackendContext<'a> {
    body: &'a Body,
    return_type: &'a Type,
    resolved: &'a ResolveOutput,
    resolved_sources: &'a crate::resolve::ResolvedSources<'a>,
    typed_hir: &'a TypedHir,
    function_signatures: &'a super::context::FunctionSignatures,
    function_names: &'a super::context::FunctionNames,
    parameters: parameters::ParameterProjection,
    root_source: SourceId,
}

pub(super) fn try_lower_body(
    cache: &crate::mir::BodyCache,
    body: &crate::ast::Block,
    parameters: &[crate::ast::Parameter],
    return_type: &Type,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    substitutions: &std::collections::HashMap<String, crate::ast::TypeExpr>,
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    parameter_slots: &super::context::LoweringParameterSlots,
    root_source: SourceId,
    sources: &SourceMap,
) -> Option<Result<Vec<Instruction>, Vec<Diagnostic>>> {
    let specialized_hir = (!substitutions.is_empty()).then(|| typed_hir.specialized(substitutions));
    let typed_hir = specialized_hir.as_ref().unwrap_or(typed_hir);
    let (return_representation, return_mode) = match return_type {
        Type::I32 => (
            crate::mir::ValueRepresentation::Scalar(ScalarType::I32),
            ReturnMode::Plain,
        ),
        Type::U8 => (
            crate::mir::ValueRepresentation::Scalar(ScalarType::U8),
            ReturnMode::Plain,
        ),
        Type::Usize => (
            crate::mir::ValueRepresentation::Scalar(ScalarType::Usize),
            ReturnMode::Plain,
        ),
        Type::Integer(kind) => (
            crate::mir::ValueRepresentation::Scalar(ScalarType::Integer(*kind)),
            ReturnMode::Plain,
        ),
        Type::Bool => (
            crate::mir::ValueRepresentation::Scalar(ScalarType::Bool),
            ReturnMode::Plain,
        ),
        Type::Str => (
            crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str),
            ReturnMode::Plain,
        ),
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => (
            crate::mir::ValueRepresentation::Aggregate,
            ReturnMode::Plain,
        ),
        Type::Optional(_) | Type::ComposedOutcome { .. } => (
            crate::mir::ValueRepresentation::Aggregate,
            ReturnMode::Plain,
        ),
        Type::Fallible(success) => match success.as_ref() {
            Type::I32 => (
                crate::mir::ValueRepresentation::Scalar(ScalarType::I32),
                ReturnMode::Fallible,
            ),
            Type::U8 => (
                crate::mir::ValueRepresentation::Scalar(ScalarType::U8),
                ReturnMode::Fallible,
            ),
            Type::Usize => (
                crate::mir::ValueRepresentation::Scalar(ScalarType::Usize),
                ReturnMode::Fallible,
            ),
            Type::Integer(kind) => (
                crate::mir::ValueRepresentation::Scalar(ScalarType::Integer(*kind)),
                ReturnMode::Fallible,
            ),
            Type::Bool => (
                crate::mir::ValueRepresentation::Scalar(ScalarType::Bool),
                ReturnMode::Fallible,
            ),
            Type::Str => (
                crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str),
                ReturnMode::Fallible,
            ),
            _ => return None,
        },
        _ => return None,
    };
    let body_id = resolved.semantic_db.body_at(body.span)?;
    let parameter_projection =
        parameters::ParameterProjection::from_slots(parameters, parameter_slots)?;
    let mir_body = cache.get_or_build_specialized(body_id, substitutions, || {
        crate::mir::try_build_body_with_return_mode(
            body,
            parameters,
            return_representation,
            return_mode,
            crate::mir::BuildInputs {
                semantic_db: &resolved.semantic_db,
                resolved,
                resolved_sources,
                typed_hir,
            },
        )
    })?;
    Some(match mir_body {
        Ok(mir_body) => lower_scalar_body(
            &mir_body,
            return_type,
            resolved,
            resolved_sources,
            typed_hir,
            function_name,
            function_signatures,
            function_names,
            parameter_projection,
            root_source,
        )
        .map_err(|diagnostics| attach_primary_span(diagnostics, sources, body.span)),
        Err(error) => Err(attach_primary_span(
            vec![Diagnostic::error(
                "E8000",
                format!("compiler could not construct MIR: {error:?}"),
            )],
            sources,
            body.span,
        )),
    })
}

fn lower_scalar_body(
    body: &Body,
    return_type: &Type,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    parameter_projection: parameters::ParameterProjection,
    root_source: SourceId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    crate::mir::validate(body).map_err(invalid_mir_diagnostics)?;
    let context = BackendContext {
        body,
        return_type,
        resolved,
        resolved_sources,
        typed_hir,
        function_signatures,
        function_names,
        parameters: parameter_projection,
        root_source,
    };
    let mut instructions = Vec::new();
    let mut current = body.entry;
    let mut visited = HashSet::new();

    loop {
        if !visited.insert(current) {
            return Err(invalid_mir_diagnostics("control flow contains a cycle"));
        }
        if let Some(loop_) = body
            .loop_regions
            .iter()
            .find(|loop_| loop_.header == current)
        {
            let (condition_instructions, condition) = loops::lower_linear_loop_condition(
                &context,
                current,
                loop_.condition,
                &mut visited,
            )?;
            let body_instructions = loops::lower_linear_loop_body(
                &context,
                loop_.body,
                loop_.header,
                loop_.exit,
                loop_.continue_target,
                &mut visited,
            )?;
            instructions.push(Instruction::While {
                condition_instructions,
                condition,
                body_instructions,
            });
            current = loop_.exit;
            continue;
        }

        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(&context, &block.statements)?);

        match &block.terminator {
            Terminator::Goto { target } => current = *target,
            Terminator::Drop {
                place,
                plan,
                target,
            } => {
                instructions.extend(drops::lower_drop(&context, *place, *plan)?);
                current = *target;
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                let join = control_flow::structured_join(body, *then_target, *else_target, None)
                    .ok_or_else(|| {
                        invalid_mir_diagnostics(
                            "scalar conditional branches must share one join block",
                        )
                    })?;
                instructions.push(Instruction::If {
                    condition: lower_bool_operand(condition, &context)?,
                    then_instructions: lower_branch_to_join(
                        &context,
                        *then_target,
                        join,
                        &mut visited,
                    )?,
                    else_instructions: lower_branch_to_join(
                        &context,
                        *else_target,
                        join,
                        &mut visited,
                    )?,
                });
                current = join;
            }
            Terminator::Call {
                callee,
                arguments,
                continuation,
                ..
            } => {
                let (call_target, callee_name) =
                    lower_call_target(callee, resolved, typed_hir, function_names, root_source)?;
                let arguments = arguments
                    .iter()
                    .map(|argument| lower_call_argument(argument, &context))
                    .collect::<Result<Vec<_>, _>>()?;
                let fits_tail_call_abi = arguments
                    .iter()
                    .map(ScalarArgument::abi_word_count)
                    .sum::<usize>()
                    <= crate::abi::ARGUMENT_REGISTER_COUNT
                    && !arguments
                        .iter()
                        .any(ScalarArgument::requires_current_frame_for_tail_call);

                match continuation {
                    CallContinuation::Never => {
                        validate_never_call_return_type(
                            &call_target,
                            &callee_name,
                            function_signatures,
                        )?;
                        if fits_tail_call_abi {
                            instructions.push(Instruction::TailCall {
                                target: call_target,
                                arguments,
                            });
                        } else {
                            instructions.push(Instruction::CallVoid {
                                target: call_target,
                                arguments,
                            });
                            instructions.push(Instruction::Trap);
                        }
                        return Ok(instructions);
                    }
                    CallContinuation::Continue { target } => {
                        validate_effect_call_return_type(
                            &call_target,
                            &callee_name,
                            function_signatures,
                        )?;
                        instructions.push(Instruction::CallVoid {
                            target: call_target,
                            arguments,
                        });
                        current = *target;
                    }
                    CallContinuation::Return {
                        destination,
                        target,
                    } => {
                        let target_block = &body.blocks[target.index()];
                        let returns_directly = destination.local == body.return_local
                            && target_block.statements.is_empty()
                            && target_block.terminator == Terminator::Return;
                        let can_tail_call =
                            returns_directly && body.return_mode == ReturnMode::Plain;
                        if can_tail_call {
                            validate_tail_call_return_type(
                                &call_target,
                                &callee_name,
                                function_name,
                                return_type,
                                function_signatures,
                            )?;
                        }
                        if can_tail_call && fits_tail_call_abi {
                            instructions.push(Instruction::TailCall {
                                target: call_target,
                                arguments,
                            });
                            return Ok(instructions);
                        }
                        reserve_aggregate_destination(&context, destination, &mut instructions)?;
                        instructions.push(lower_returning_call(
                            &context,
                            destination,
                            call_target,
                            arguments,
                            &callee_name,
                            function_signatures,
                        )?);
                        current = *target;
                    }
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                        failure_payload,
                    } => {
                        let failure_mode = outcome_failure_mode(
                            &context,
                            *failure,
                            *success,
                            *failure_payload,
                            &mut visited,
                        )?;
                        reserve_aggregate_destination(&context, destination, &mut instructions)?;
                        instructions.push(lower_outcome_call(
                            &context,
                            destination,
                            call_target,
                            arguments,
                            failure_mode,
                            &callee_name,
                        )?);
                        current = *success;
                    }
                    CallContinuation::OutcomeEffect {
                        success,
                        failure,
                        failure_payload,
                    } => {
                        validate_outcome_effect_call_return_type(
                            &call_target,
                            &callee_name,
                            function_signatures,
                        )?;
                        instructions.push(Instruction::CallOutcomeVoid {
                            target: call_target,
                            arguments,
                            failure_mode: outcome_failure_mode(
                                &context,
                                *failure,
                                *success,
                                *failure_payload,
                                &mut visited,
                            )?,
                        });
                        current = *success;
                    }
                }
            }
            Terminator::InspectOutcome {
                source,
                layer,
                destination,
                success,
                failure,
                failure_payload,
                ..
            } => {
                reserve_aggregate_destination(&context, destination, &mut instructions)?;
                instructions.push(outcomes::lower(
                    &context,
                    outcomes::Inspection {
                        source: *source,
                        layer: *layer,
                        destination: *destination,
                        success: *success,
                        failure: *failure,
                        failure_payload: *failure_payload,
                        visited: &mut visited,
                    },
                )?);
                current = *success;
            }
            Terminator::Trap => {
                instructions.push(Instruction::Trap);
                return Ok(instructions);
            }
            Terminator::PropagateFailure => {
                instructions.push(Instruction::PropagateFailure);
                return Ok(instructions);
            }
            Terminator::ReturnOutcome { source } => {
                instructions.push(outcomes::lower_return(&context, source)?);
                return Ok(instructions);
            }
            Terminator::ReturnFailure { code, message } => {
                if body.return_mode != ReturnMode::Fallible {
                    return Err(invalid_mir_diagnostics(
                        "plain MIR body contains a recoverable failure return",
                    ));
                }
                instructions.push(Instruction::ReturnFallibleFailure {
                    code: lower_str_operand(code, &context)?,
                    message: lower_str_operand(message, &context)?,
                });
                return Ok(instructions);
            }
            Terminator::Return => {
                instructions.push(match body.return_mode {
                    ReturnMode::Plain => Instruction::Return,
                    ReturnMode::Fallible => Instruction::ReturnOutcomeSuccess,
                });
                return Ok(instructions);
            }
        }
    }
}

fn outcome_failure_mode(
    context: &BackendContext<'_>,
    failure: crate::mir::BasicBlockId,
    success: crate::mir::BasicBlockId,
    failure_payload: Option<LocalId>,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>> {
    let body = context.body;
    if let Some(payload) = failure_payload {
        let (code, message) = error_locations(body, payload)?;
        return Ok(OutcomeFailureMode::Catch {
            code,
            message,
            instructions: lower_branch_to_join(context, failure, success, visited)?,
            recovers: control_flow::can_reach(body, failure, success),
        });
    }
    if body.return_mode == ReturnMode::Fallible
        && let Some(path) = no_op_propagation_path(context, failure)?
    {
        visited.extend(path);
        return Ok(OutcomeFailureMode::Propagate);
    }
    let failure_block = &body.blocks[failure.index()];
    match &failure_block.terminator {
        Terminator::Trap if failure_block.statements.is_empty() => {
            visited.insert(failure);
            Ok(OutcomeFailureMode::Trap)
        }
        Terminator::PropagateFailure if body.return_mode == ReturnMode::Fallible => {
            if !failure_block.statements.is_empty() {
                return Err(invalid_mir_diagnostics(
                    "failure propagation block contains value instructions",
                ));
            }
            visited.insert(failure);
            Ok(OutcomeFailureMode::Propagate)
        }
        _ if control_flow::can_reach(body, failure, success) => Ok(OutcomeFailureMode::Recover {
            instructions: lower_branch_to_join(context, failure, success, visited)?,
        }),
        _ => Ok(OutcomeFailureMode::Handle {
            instructions: lower_branch_to_join(context, failure, success, visited)?,
        }),
    }
}

fn no_op_propagation_path(
    context: &BackendContext<'_>,
    start: crate::mir::BasicBlockId,
) -> Result<Option<Vec<crate::mir::BasicBlockId>>, Vec<Diagnostic>> {
    let mut current = start;
    let mut path = Vec::new();
    loop {
        if path.contains(&current) {
            return Ok(None);
        }
        path.push(current);
        let block = &context.body.blocks[current.index()];
        if !lower_statements(context, &block.statements)?.is_empty() {
            return Ok(None);
        }
        match block.terminator {
            Terminator::Goto { target } => current = target,
            Terminator::Drop {
                place,
                plan,
                target,
            } if drops::lower_drop(context, place, plan)?.is_empty() => current = target,
            Terminator::PropagateFailure => return Ok(Some(path)),
            _ => return Ok(None),
        }
    }
}

fn validate_outcome_call_return_type(
    target: &crate::ir::CallTarget,
    callee_name: &str,
    destination_scalar: ScalarType,
    function_signatures: &super::context::FunctionSignatures,
) -> Result<(), Vec<Diagnostic>> {
    let Some(return_type) = function_signatures.return_type(target) else {
        return Ok(());
    };
    let success = match return_type {
        Type::Fallible(success) | Type::Optional(success) => success,
        _ => {
            return Err(invalid_mir_diagnostics(format!(
                "outcome call to `{callee_name}` does not return a recoverable value"
            )));
        }
    };
    let expected = scalar_ir_type(destination_scalar);
    if success.as_ref() != &expected {
        return Err(invalid_mir_diagnostics(format!(
            "outcome call to `{callee_name}` returns `{}` but its MIR destination is `{}`",
            super::expressions::describe_type(success),
            super::expressions::describe_type(&expected),
        )));
    }
    super::expressions::validate_known_call_success_return_passing(
        function_signatures.success_return_passing(target),
        callee_name,
        &expected,
    )
}

fn validate_outcome_effect_call_return_type(
    target: &crate::ir::CallTarget,
    callee_name: &str,
    function_signatures: &super::context::FunctionSignatures,
) -> Result<(), Vec<Diagnostic>> {
    let Some(return_type) = function_signatures.return_type(target) else {
        return Ok(());
    };
    if return_type == &Type::Fallible(Box::new(Type::Void)) {
        return Ok(());
    }
    Err(invalid_mir_diagnostics(format!(
        "outcome effect call to `{callee_name}` returns `{}` instead of `void!`",
        super::expressions::describe_type(return_type),
    )))
}

fn lower_branch_to_join(
    context: &BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    join: crate::mir::BasicBlockId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let body = context.body;
    let mut instructions = Vec::new();
    let mut current = start;
    loop {
        if current == join {
            return Ok(instructions);
        }
        if !visited.insert(current) {
            return Err(invalid_mir_diagnostics(
                "control-flow branch reuses an already lowered block",
            ));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(context, &block.statements)?);
        match &block.terminator {
            Terminator::Goto { target } => current = *target,
            Terminator::Drop {
                place,
                plan,
                target,
            } => {
                instructions.extend(drops::lower_drop(context, *place, *plan)?);
                current = *target;
            }
            Terminator::Call {
                callee,
                arguments,
                continuation: CallContinuation::Continue { target },
                ..
            } => {
                let (call_target, callee_name) = lower_call_target(
                    callee,
                    context.resolved,
                    context.typed_hir,
                    context.function_names,
                    context.root_source,
                )?;
                let arguments = arguments
                    .iter()
                    .map(|argument| lower_call_argument(argument, context))
                    .collect::<Result<Vec<_>, _>>()?;
                validate_effect_call_return_type(
                    &call_target,
                    &callee_name,
                    context.function_signatures,
                )?;
                instructions.push(Instruction::CallVoid {
                    target: call_target,
                    arguments,
                });
                current = *target;
            }
            Terminator::Call {
                callee,
                arguments,
                continuation:
                    CallContinuation::Return {
                        destination,
                        target,
                    },
                ..
            } => {
                let (call_target, callee_name) = lower_call_target(
                    callee,
                    context.resolved,
                    context.typed_hir,
                    context.function_names,
                    context.root_source,
                )?;
                let arguments = arguments
                    .iter()
                    .map(|argument| lower_call_argument(argument, context))
                    .collect::<Result<Vec<_>, _>>()?;
                reserve_aggregate_destination(context, destination, &mut instructions)?;
                instructions.push(lower_returning_call(
                    context,
                    destination,
                    call_target,
                    arguments,
                    &callee_name,
                    context.function_signatures,
                )?);
                current = *target;
            }
            Terminator::Call {
                callee,
                arguments,
                continuation:
                    CallContinuation::OutcomeEffect {
                        success,
                        failure,
                        failure_payload,
                    },
                ..
            } => {
                let (call_target, callee_name) = lower_call_target(
                    callee,
                    context.resolved,
                    context.typed_hir,
                    context.function_names,
                    context.root_source,
                )?;
                let arguments = arguments
                    .iter()
                    .map(|argument| lower_call_argument(argument, context))
                    .collect::<Result<Vec<_>, _>>()?;
                validate_outcome_effect_call_return_type(
                    &call_target,
                    &callee_name,
                    context.function_signatures,
                )?;
                instructions.push(Instruction::CallOutcomeVoid {
                    target: call_target,
                    arguments,
                    failure_mode: outcome_failure_mode(
                        context,
                        *failure,
                        *success,
                        *failure_payload,
                        visited,
                    )?,
                });
                current = *success;
            }
            Terminator::InspectOutcome {
                source,
                layer,
                destination,
                success,
                failure,
                failure_payload,
                ..
            } => {
                reserve_aggregate_destination(context, destination, &mut instructions)?;
                instructions.push(outcomes::lower(
                    context,
                    outcomes::Inspection {
                        source: *source,
                        layer: *layer,
                        destination: *destination,
                        success: *success,
                        failure: *failure,
                        failure_payload: *failure_payload,
                        visited,
                    },
                )?);
                current = *success;
            }
            Terminator::Call {
                callee,
                arguments,
                continuation:
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                        failure_payload,
                    },
                ..
            } => {
                let (call_target, callee_name) = lower_call_target(
                    callee,
                    context.resolved,
                    context.typed_hir,
                    context.function_names,
                    context.root_source,
                )?;
                let arguments = arguments
                    .iter()
                    .map(|argument| lower_call_argument(argument, context))
                    .collect::<Result<Vec<_>, _>>()?;
                reserve_aggregate_destination(context, destination, &mut instructions)?;
                instructions.push(lower_outcome_call(
                    context,
                    destination,
                    call_target,
                    arguments,
                    outcome_failure_mode(context, *failure, *success, *failure_payload, visited)?,
                    &callee_name,
                )?);
                current = *success;
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                let branch_join =
                    control_flow::structured_join(body, *then_target, *else_target, Some(join))
                        .ok_or_else(|| {
                            invalid_mir_diagnostics(
                                "nested scalar conditional branches do not share a join",
                            )
                        })?;
                instructions.push(Instruction::If {
                    condition: lower_bool_operand(condition, context)?,
                    then_instructions: lower_branch_to_join(
                        context,
                        *then_target,
                        branch_join,
                        visited,
                    )?,
                    else_instructions: lower_branch_to_join(
                        context,
                        *else_target,
                        branch_join,
                        visited,
                    )?,
                });
                current = branch_join;
            }
            Terminator::Return => {
                instructions.push(match body.return_mode {
                    ReturnMode::Plain => Instruction::Return,
                    ReturnMode::Fallible => Instruction::ReturnOutcomeSuccess,
                });
                return Ok(instructions);
            }
            Terminator::Trap => {
                instructions.push(Instruction::Trap);
                return Ok(instructions);
            }
            Terminator::PropagateFailure if body.return_mode == ReturnMode::Fallible => {
                instructions.push(Instruction::PropagateFailure);
                return Ok(instructions);
            }
            Terminator::ReturnOutcome { source } => {
                instructions.push(outcomes::lower_return(context, source)?);
                return Ok(instructions);
            }
            Terminator::ReturnFailure { code, message } => {
                if body.return_mode != ReturnMode::Fallible {
                    return Err(invalid_mir_diagnostics(
                        "plain MIR branch contains a recoverable failure return",
                    ));
                }
                instructions.push(Instruction::ReturnFallibleFailure {
                    code: lower_str_operand(code, context)?,
                    message: lower_str_operand(message, context)?,
                });
                return Ok(instructions);
            }
            _ => {
                return Err(invalid_mir_diagnostics(
                    "scalar conditional branch does not reach its join",
                ));
            }
        }
    }
}

fn call_instruction(
    context: &BackendContext<'_>,
    scalar: ScalarType,
    destination: &Place,
    target: crate::ir::CallTarget,
    arguments: Vec<ScalarArgument>,
) -> Result<Instruction, Vec<Diagnostic>> {
    Ok(match scalar {
        ScalarType::I32 => Instruction::CallI32 {
            destination: i32_location(destination, context)?,
            target,
            arguments,
        },
        ScalarType::U8 => Instruction::CallU8 {
            destination: u8_location(destination, context)?,
            target,
            arguments,
        },
        ScalarType::Usize => Instruction::CallUsize {
            destination: usize_location(destination, context)?,
            target,
            arguments,
        },
        ScalarType::Integer(kind) => Instruction::CallUsize {
            destination: integer_location(destination, kind, context)?,
            target,
            arguments,
        },
        ScalarType::Bool => Instruction::CallBool {
            destination: bool_location(destination, context)?,
            target,
            arguments,
        },
    })
}

fn lower_returning_call(
    context: &BackendContext<'_>,
    destination: &Place,
    target: crate::ir::CallTarget,
    arguments: Vec<ScalarArgument>,
    callee_name: &str,
    function_signatures: &super::context::FunctionSignatures,
) -> Result<Instruction, Vec<Diagnostic>> {
    let representation = context.body.locals[destination.local.index()].representation;
    if representation == crate::mir::ValueRepresentation::Aggregate {
        let return_type = function_signatures.return_type(&target).ok_or_else(|| {
            invalid_mir_diagnostics(format!(
                "aggregate call to `{callee_name}` has no indexed return type"
            ))
        })?;
        if matches!(
            return_type,
            Type::Optional(_) | Type::Fallible(_) | Type::ComposedOutcome { .. }
        ) {
            let type_expr = context
                .typed_hir
                .type_expr_by_id(context.body.locals[destination.local.index()].ty)
                .ok_or_else(|| invalid_mir_diagnostics("stored outcome local type is missing"))?;
            let shape = crate::outcomes::outcome_shape_with_resolver(
                type_expr,
                context.resolved,
                |source| context.resolved_sources.get(&source).copied(),
            );
            let payload_abi = crate::abi::abi_value_from_type_expr_with_resolver(
                &shape.payload,
                context.resolved,
                |source| context.resolved_sources.get(&source).copied(),
            )
            .map_err(|error| {
                invalid_mir_diagnostics(format!(
                    "cannot lay out stored outcome payload for `{callee_name}`: {error:?}"
                ))
            })?;
            let storage = shape.storage_layout(payload_abi.layout).ok_or_else(|| {
                invalid_mir_diagnostics(format!(
                    "stored outcome returned by `{callee_name}` has an unsupported layer shape"
                ))
            })?;
            let payload_type = super::types::return_type_from_type_expr_with_resolver(
                &shape.payload,
                context.resolved,
                |source| context.resolved_sources.get(&source).copied(),
            )
            .ok_or_else(|| {
                invalid_mir_diagnostics(format!(
                    "stored outcome payload returned by `{callee_name}` is unsupported"
                ))
            })?;
            return Ok(Instruction::CallStoredOutcome {
                destination: aggregate_location(destination, context)?,
                target,
                arguments,
                storage,
                payload_type,
            });
        }
        let layout = match return_type {
            Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
            _ => {
                return Err(invalid_mir_diagnostics(format!(
                    "aggregate MIR destination received `{}` from `{callee_name}`",
                    super::expressions::describe_type(return_type)
                )));
            }
        };
        super::expressions::validate_known_call_success_return_passing(
            function_signatures.success_return_passing(&target),
            callee_name,
            return_type,
        )?;
        return Ok(super::aggregates::aggregate_call_instruction(
            return_type,
            aggregate_location(destination, context)?,
            target,
            arguments,
            layout,
        ));
    }
    if representation == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) {
        super::expressions::validate_known_call_success_return_passing(
            context.function_signatures.success_return_passing(&target),
            callee_name,
            &Type::Str,
        )?;
        return Ok(Instruction::CallStr {
            destination: str_location(destination, context)?,
            target,
            arguments,
        });
    }
    let scalar = local_scalar(context.body, destination.local)?;
    super::expressions::validate_known_call_success_return_passing(
        function_signatures.success_return_passing(&target),
        callee_name,
        &scalar_ir_type(scalar),
    )?;
    call_instruction(context, scalar, destination, target, arguments)
}

fn reserve_aggregate_destination(
    context: &BackendContext<'_>,
    destination: &Place,
    instructions: &mut Vec<Instruction>,
) -> Result<(), Vec<Diagnostic>> {
    let local = &context.body.locals[destination.local.index()];
    if local.representation != crate::mir::ValueRepresentation::Aggregate
        || local.storage != LocalStorage::Local
    {
        return Ok(());
    }
    let value = aggregate_local_abi_value(local.ty, context)?;
    let crate::ir::AggregateLocation::Slot(slot_index) = aggregate_location(destination, context)?
    else {
        return Err(invalid_mir_diagnostics(
            "aggregate local destination is not slot-backed",
        ));
    };
    instructions.push(Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    });
    Ok(())
}

pub(super) fn aggregate_local_abi_value(
    ty: crate::semantic::TyId,
    context: &BackendContext<'_>,
) -> Result<crate::abi::AbiValue, Vec<Diagnostic>> {
    let type_expr = context
        .typed_hir
        .type_expr_by_id(ty)
        .ok_or_else(|| invalid_mir_diagnostics("aggregate local type is missing"))?;
    crate::abi::abi_value_from_type_expr_with_resolver(type_expr, context.resolved, |source| {
        context.resolved_sources.get(&source).copied()
    })
    .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))
}

fn lower_outcome_call(
    context: &BackendContext<'_>,
    destination: &Place,
    target: crate::ir::CallTarget,
    arguments: Vec<ScalarArgument>,
    failure_mode: OutcomeFailureMode,
    callee_name: &str,
) -> Result<Instruction, Vec<Diagnostic>> {
    let representation = context.body.locals[destination.local.index()].representation;
    if representation == crate::mir::ValueRepresentation::Aggregate {
        let return_type = context
            .function_signatures
            .return_type(&target)
            .ok_or_else(|| {
                invalid_mir_diagnostics(format!(
                    "outcome aggregate call to `{callee_name}` has no indexed return type"
                ))
            })?;
        let success = match return_type {
            Type::Optional(success) | Type::Fallible(success) => success.as_ref(),
            _ => {
                return Err(invalid_mir_diagnostics(format!(
                    "outcome aggregate call to `{callee_name}` does not return one outcome layer"
                )));
            }
        };
        let layout =
            aggregate_local_abi_value(context.body.locals[destination.local.index()].ty, context)?
                .layout;
        if !matches!(
            success,
            Type::Aggregate { .. } | Type::DirectAggregate { .. }
        ) {
            return Err(invalid_mir_diagnostics(format!(
                "outcome call to `{callee_name}` has a non-aggregate success type"
            )));
        }
        super::expressions::validate_known_call_success_return_passing(
            context.function_signatures.success_return_passing(&target),
            callee_name,
            success,
        )?;
        return Ok(super::aggregates::fallible_aggregate_call_instruction(
            success,
            aggregate_location(destination, context)?,
            target,
            arguments,
            layout,
            failure_mode,
        ));
    }
    if representation == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) {
        return Ok(Instruction::CallOutcomeStr {
            destination: str_location(destination, context)?,
            target,
            arguments,
            failure_mode,
        });
    }
    if representation == crate::mir::ValueRepresentation::Borrow {
        return Ok(Instruction::CallOutcomeBorrow {
            destination: usize_location(destination, context)?,
            target,
            arguments,
            failure_mode,
        });
    }
    let scalar = local_scalar(context.body, destination.local)?;
    validate_outcome_call_return_type(&target, callee_name, scalar, context.function_signatures)?;
    outcome_call_instruction(
        context,
        scalar,
        destination,
        target,
        arguments,
        failure_mode,
    )
}

fn outcome_call_instruction(
    context: &BackendContext<'_>,
    scalar: ScalarType,
    destination: &Place,
    target: crate::ir::CallTarget,
    arguments: Vec<ScalarArgument>,
    failure_mode: OutcomeFailureMode,
) -> Result<Instruction, Vec<Diagnostic>> {
    Ok(match scalar {
        ScalarType::I32 => Instruction::CallOutcomeI32 {
            destination: i32_location(destination, context)?,
            target,
            arguments,
            failure_mode,
        },
        ScalarType::U8 => Instruction::CallOutcomeU8 {
            destination: u8_location(destination, context)?,
            target,
            arguments,
            failure_mode,
        },
        ScalarType::Usize => Instruction::CallOutcomeUsize {
            destination: usize_location(destination, context)?,
            target,
            arguments,
            failure_mode,
        },
        ScalarType::Integer(kind) => Instruction::CallOutcomeUsize {
            destination: integer_location(destination, kind, context)?,
            target,
            arguments,
            failure_mode,
        },
        ScalarType::Bool => Instruction::CallOutcomeBool {
            destination: bool_location(destination, context)?,
            target,
            arguments,
            failure_mode,
        },
    })
}

fn scalar_ir_type(scalar: ScalarType) -> Type {
    match scalar {
        ScalarType::I32 => Type::I32,
        ScalarType::U8 => Type::U8,
        ScalarType::Usize => Type::Usize,
        ScalarType::Integer(kind) => Type::Integer(kind),
        ScalarType::Bool => Type::Bool,
    }
}

fn validate_never_call_return_type(
    target: &crate::ir::CallTarget,
    callee_name: &str,
    function_signatures: &super::context::FunctionSignatures,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = function_signatures.return_type(target) else {
        return Ok(());
    };
    if callee_return_type == &Type::Never {
        return Ok(());
    }
    Err(invalid_mir_diagnostics(format!(
        "call to `{callee_name}` has a non-returning continuation but returns `{}`",
        super::expressions::describe_type(callee_return_type),
    )))
}

fn lower_call_target(
    callee: &crate::mir::CallInstance,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    function_names: &super::context::FunctionNames,
    root_source: SourceId,
) -> Result<(crate::ir::CallTarget, String), Vec<Diagnostic>> {
    let name = function_names
        .name_for_instance(callee, typed_hir)
        .ok_or_else(|| invalid_mir_diagnostics("call target has no indexed runtime name"))?
        .clone();
    let source = match callee.callable {
        crate::mir::CallableIdentity::Definition(definition) => {
            resolved
                .semantic_db
                .definition_anchor(definition)
                .ok_or_else(|| invalid_mir_diagnostics("call target has no source anchor"))?
                .source
        }
        crate::mir::CallableIdentity::Value { ty, .. } => {
            typed_hir
                .type_expr_by_id(ty)
                .ok_or_else(|| invalid_mir_diagnostics("callable-value type is missing"))?
                .span()
                .source
        }
    };
    Ok((
        super::call_target_for_source(source, root_source, name.clone()),
        name,
    ))
}

fn validate_tail_call_return_type(
    target: &crate::ir::CallTarget,
    callee_name: &str,
    function_name: &str,
    return_type: &Type,
    function_signatures: &super::context::FunctionSignatures,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = function_signatures.return_type(target) else {
        return Ok(());
    };
    if callee_return_type == &Type::Never || callee_return_type == return_type {
        return Ok(());
    }
    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "native lowering cannot lower tail call from function `{function_name}` returning `{}` to function `{callee_name}` returning `{}`",
            super::expressions::describe_type(return_type),
            super::expressions::describe_type(callee_return_type),
        ),
    )])
}

fn validate_effect_call_return_type(
    target: &crate::ir::CallTarget,
    callee_name: &str,
    function_signatures: &super::context::FunctionSignatures,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = function_signatures.return_type(target) else {
        return Ok(());
    };
    if callee_return_type == &Type::Void {
        return Ok(());
    }
    Err(invalid_mir_diagnostics(format!(
        "effect call to `{callee_name}` returns `{}` instead of `void`",
        super::expressions::describe_type(callee_return_type),
    )))
}

fn lower_call_argument(
    argument: &crate::mir::CallArgument,
    context: &BackendContext<'_>,
) -> Result<ScalarArgument, Vec<Diagnostic>> {
    Ok(match argument.representation {
        crate::mir::ValueRepresentation::Scalar(ScalarType::I32) => {
            ScalarArgument::I32(lower_i32_operand(&argument.operand, context)?)
        }
        crate::mir::ValueRepresentation::Scalar(ScalarType::U8) => {
            ScalarArgument::U8(lower_u8_operand(&argument.operand, context)?)
        }
        crate::mir::ValueRepresentation::Scalar(ScalarType::Usize) => {
            ScalarArgument::Usize(lower_usize_operand(&argument.operand, context)?)
        }
        crate::mir::ValueRepresentation::Scalar(ScalarType::Integer(kind)) => {
            ScalarArgument::Usize(lower_integer_operand(&argument.operand, kind, context)?)
        }
        crate::mir::ValueRepresentation::Scalar(ScalarType::Bool) => {
            ScalarArgument::Bool(lower_bool_operand(&argument.operand, context)?)
        }
        crate::mir::ValueRepresentation::Aggregate => {
            lower_aggregate_call_argument(&argument.operand, context)?
        }
        crate::mir::ValueRepresentation::Borrow => {
            ScalarArgument::Borrow(crate::ir::BorrowArgument {
                source: lower_borrow_argument_source(&argument.operand, context)?,
            })
        }
        crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) => {
            ScalarArgument::Str(lower_str_operand(&argument.operand, context)?)
        }
        crate::mir::ValueRepresentation::Error => {
            return Err(invalid_mir_diagnostics(
                "logical error values cannot be passed as scalar call arguments",
            ));
        }
    })
}

fn lower_borrow_argument_source(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<crate::ir::BorrowSource, Vec<Diagnostic>> {
    let (Operand::Copy(place) | Operand::Move(place)) = operand else {
        return Err(invalid_mir_diagnostics(
            "borrow call argument is not a stored place",
        ));
    };
    if place.projection.is_none()
        && let Some(source) = storage::inlined_borrow_source(context.body, place.local)
    {
        return lower_borrow_source(source, context);
    }
    if place.projection.is_some() {
        return Err(invalid_mir_diagnostics(
            "projected borrow call arguments require an explicit MIR loan",
        ));
    }
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::Borrow { abi_index }) => {
                Ok(crate::ir::BorrowSource::BorrowParameter(abi_index))
            }
            _ => Err(invalid_mir_diagnostics(
                "borrow MIR parameter has no matching ABI projection",
            )),
        },
        LocalStorage::Local => Ok(crate::ir::BorrowSource::BorrowLocal(UsizeLocation::Local(
            machine_local_index(context.body, place.local),
        ))),
        LocalStorage::Return => Err(invalid_mir_diagnostics(
            "return storage cannot be used as a borrow call argument",
        )),
    }
}

fn lower_aggregate_call_argument(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<ScalarArgument, Vec<Diagnostic>> {
    let place = match operand {
        Operand::Copy(place) | Operand::Move(place) => place,
        Operand::Constant(_) | Operand::StaticStr { .. } => {
            return Err(invalid_mir_diagnostics(
                "aggregate call argument is not a stored place",
            ));
        }
    };
    let local = &context.body.locals[place.local.index()];
    let argument_ty = place
        .projection
        .and_then(|projection| context.body.projections.get(projection.index()))
        .map_or(local.ty, |projection| projection.ty);
    let argument_value = aggregate_local_abi_value(argument_ty, context)?;
    let (layout, classification) = match local.storage {
        LocalStorage::Parameter { ordinal } => {
            let Some(parameters::ParameterStorage::Aggregate {
                layout,
                classification,
                ..
            }) = context.parameters.get(ordinal)
            else {
                return Err(invalid_mir_diagnostics(
                    "aggregate MIR parameter has no matching staging slot",
                ));
            };
            if place.projection.is_some() {
                (argument_value.layout, argument_value.classification)
            } else {
                (layout, classification)
            }
        }
        LocalStorage::Local => (argument_value.layout, argument_value.classification),
        LocalStorage::Return => {
            return Err(invalid_mir_diagnostics(
                "aggregate return storage cannot be a call argument",
            ));
        }
    };
    let crate::ir::AggregateLocation::Slot(slot_index) =
        aggregate_location(&Place::local(place.local), context)?
    else {
        return Err(invalid_mir_diagnostics(
            "aggregate argument is not slot-backed",
        ));
    };
    let offset = place
        .projection
        .map(|projection| aggregate_field_offset(context.body, place.local, projection))
        .transpose()?
        .unwrap_or(0);
    let source = if offset == 0 {
        AggregateArgumentSource::Slot(slot_index)
    } else {
        AggregateArgumentSource::SlotField { slot_index, offset }
    };
    Ok(match classification {
        crate::abi::ValueClassification::Direct { words } => {
            ScalarArgument::AggregateDirect(DirectAggregateArgument {
                source,
                layout,
                words,
            })
        }
        crate::abi::ValueClassification::Indirect => {
            ScalarArgument::AggregateIndirect(AggregateArgument { source })
        }
    })
}

fn aggregate_location(
    place: &Place,
    context: &BackendContext<'_>,
) -> Result<crate::ir::AggregateLocation, Vec<Diagnostic>> {
    if place.projection.is_some() {
        return Err(invalid_mir_diagnostics(
            "projected aggregate destination has no whole-slot projection",
        ));
    }
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Return => match context.return_type {
            Type::Aggregate { .. } => Ok(crate::ir::AggregateLocation::Return),
            Type::DirectAggregate { .. } => Ok(crate::ir::AggregateLocation::DirectReturn),
            _ => Err(invalid_mir_diagnostics(
                "aggregate MIR return local has a non-aggregate function return type",
            )),
        },
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::Aggregate { slot_index, .. }) => {
                Ok(crate::ir::AggregateLocation::Slot(slot_index))
            }
            _ => Err(invalid_mir_diagnostics(
                "aggregate MIR parameter has no matching staging slot",
            )),
        },
        LocalStorage::Local => {
            let preceding = context.body.locals[..place.local.index()]
                .iter()
                .filter(|local| {
                    local.storage == LocalStorage::Local
                        && local.representation == crate::mir::ValueRepresentation::Aggregate
                })
                .count();
            Ok(crate::ir::AggregateLocation::Slot(
                context.parameters.first_local_aggregate_slot() + preceding,
            ))
        }
    }
}

fn lower_statements(
    context: &BackendContext<'_>,
    statements: &[Statement],
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let body = context.body;
    let mut instructions = Vec::new();
    for statement in statements {
        if let Statement::BeginAggregate { destination, .. } = statement {
            reserve_aggregate_destination(context, destination, &mut instructions)?;
            continue;
        }
        if matches!(statement, Statement::FinishAggregate { .. }) {
            continue;
        }
        if let Statement::EnterRegion { region, .. } = statement {
            instructions.extend(lower_region_enter(*region, context)?);
            continue;
        }
        if let Statement::ExitRegion { region } = statement {
            instructions.push(lower_region_exit(*region, context)?);
            continue;
        }
        if let Statement::BeginLoan { loan, .. } = statement {
            let declaration = body.loans.get(loan.index()).ok_or_else(|| {
                invalid_mir_diagnostics("borrow statement has no matching loan declaration")
            })?;
            if storage::is_inlined_borrow_temporary(body, declaration.destination) {
                continue;
            }
            instructions.push(Instruction::SetUsizeFromBorrow {
                destination: UsizeLocation::Local(machine_local_index(
                    body,
                    declaration.destination,
                )),
                source: lower_borrow_source(declaration.source, context)?,
            });
            continue;
        }
        if matches!(statement, Statement::EndLoan { .. }) {
            continue;
        }
        let Statement::Assign {
            destination, value, ..
        } = statement
        else {
            unreachable!("all MIR statement kinds handled above");
        };
        if let Rvalue::Variant { variant, leaves } = value {
            reserve_aggregate_destination(context, destination, &mut instructions)?;
            let location = aggregate_location(destination, context)?;
            let abi =
                aggregate_local_abi_value(body.locals[destination.local.index()].ty, context)?;
            let crate::abi::AbiType::Enum(enum_) = &abi.ty else {
                return Err(invalid_mir_diagnostics(
                    "variant MIR rvalue destination is not enum storage",
                ));
            };
            let signature = enum_variant_signature(context, *variant)?;
            let abi_variant = enum_
                .variants
                .iter()
                .find(|candidate| candidate.name == signature.name)
                .ok_or_else(|| {
                    invalid_mir_diagnostics("MIR variant has no matching ABI variant")
                })?;
            instructions.push(Instruction::StoreAggregateU8 {
                destination: location,
                offset: 0,
                value: U8Value::Const(abi_variant.tag),
            });
            for leaf in leaves {
                let (offset, leaf_abi) =
                    enum_variant_leaf_projection(enum_, abi_variant, &leaf.path)?;
                if !abi_type_matches_scalar(leaf_abi, leaf.scalar) {
                    return Err(invalid_mir_diagnostics(
                        "variant MIR leaf scalar does not match its ABI projection",
                    ));
                }
                instructions.push(store_aggregate_scalar(
                    location,
                    offset,
                    leaf.scalar,
                    &leaf.operand,
                    context,
                )?);
            }
            continue;
        }
        let destination_representation = destination
            .projection
            .and_then(|projection| body.projections.get(projection.index()))
            .map_or(
                body.locals[destination.local.index()].representation,
                |path| path.representation,
            );
        if destination_representation == crate::mir::ValueRepresentation::Aggregate {
            let Rvalue::Use(Operand::Copy(source) | Operand::Move(source)) = value else {
                return Err(invalid_mir_diagnostics(
                    "aggregate MIR assignment requires a stored source place",
                ));
            };
            if source.projection.is_some() || destination.projection.is_some() {
                return Err(invalid_mir_diagnostics(
                    "projected aggregate MIR assignment requires range projection",
                ));
            }
            reserve_aggregate_destination(context, destination, &mut instructions)?;
            let layout =
                aggregate_local_abi_value(body.locals[destination.local.index()].ty, context)?
                    .layout;
            instructions.push(Instruction::CopyAggregate {
                destination: aggregate_location(destination, context)?,
                source: aggregate_location(source, context)?,
                layout,
            });
            continue;
        }
        if destination_representation
            == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str)
        {
            let Rvalue::Use(operand) = value else {
                return Err(invalid_mir_diagnostics(
                    "string-view assignment requires a direct MIR operand",
                ));
            };
            instructions.push(Instruction::SetStr {
                destination: str_location(destination, context)?,
                value: lower_str_operand(operand, context)?,
            });
            continue;
        }
        if destination.projection.is_some() {
            let Rvalue::Use(operand) = value else {
                return Err(invalid_mir_diagnostics(
                    "projected scalar assignment must materialize its value first",
                ));
            };
            instructions.push(aggregate_scalar_store(destination, operand, context)?);
            continue;
        }
        match local_scalar(body, destination.local)? {
            ScalarType::I32 => {
                let destination = i32_location(destination, context)?;
                match value {
                    Rvalue::Variant { .. } => {
                        unreachable!("aggregate rvalue handled above")
                    }
                    Rvalue::Use(operand) => {
                        if let Some(instruction) = aggregate_scalar_load(
                            ScalarDestination::I32(destination),
                            operand,
                            context,
                        )? {
                            instructions.push(instruction);
                        } else {
                            instructions.push(Instruction::SetI32 {
                                destination,
                                value: lower_i32_operand(operand, context)?,
                            });
                        }
                    }
                    Rvalue::Cast {
                        operand,
                        source_scalar,
                        ..
                    } => instructions.push(Instruction::SetI32 {
                        destination,
                        value: lower_cast_to_i32(*source_scalar, operand, context)?,
                    }),
                    Rvalue::Unary {
                        operator: UnaryOperator::Negate,
                        operand,
                        ..
                    } => instructions.push(Instruction::SubtractI32 {
                        destination,
                        left: I32Value::Const(0),
                        right: lower_i32_operand(operand, context)?,
                    }),
                    Rvalue::Unary { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "i32 scalar route received a non-numeric unary operation",
                        ));
                    }
                    Rvalue::Binary {
                        operator,
                        left,
                        right,
                        ..
                    } => instructions.push(i32_binary_instruction(
                        *operator,
                        destination,
                        lower_i32_operand(left, context)?,
                        lower_i32_operand(right, context)?,
                    )),
                    Rvalue::Compare { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "i32 scalar route received a comparison result",
                        ));
                    }
                }
            }
            ScalarType::U8 => {
                let destination = u8_location(destination, context)?;
                match value {
                    Rvalue::Variant { .. } => {
                        unreachable!("aggregate rvalue handled above")
                    }
                    Rvalue::Use(operand) => {
                        if let Some(instruction) = aggregate_scalar_load(
                            ScalarDestination::U8(destination),
                            operand,
                            context,
                        )? {
                            instructions.push(instruction);
                        } else {
                            instructions.push(Instruction::SetU8 {
                                destination,
                                value: lower_u8_operand(operand, context)?,
                            });
                        }
                    }
                    Rvalue::Cast {
                        operand,
                        source_scalar,
                        ..
                    } => instructions.push(Instruction::SetU8 {
                        destination,
                        value: lower_cast_to_u8(*source_scalar, operand, context)?,
                    }),
                    Rvalue::Unary { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "u8 scalar route received an invalid unary operation",
                        ));
                    }
                    Rvalue::Binary {
                        operator,
                        left,
                        right,
                        ..
                    } => instructions.push(u8_binary_instruction(
                        *operator,
                        destination,
                        lower_u8_operand(left, context)?,
                        lower_u8_operand(right, context)?,
                    )),
                    Rvalue::Compare { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "u8 scalar route received a comparison result",
                        ));
                    }
                }
            }
            ScalarType::Usize => {
                let destination = usize_location(destination, context)?;
                match value {
                    Rvalue::Variant { .. } => {
                        unreachable!("aggregate rvalue handled above")
                    }
                    Rvalue::Use(operand) => {
                        if let Some(instruction) = aggregate_scalar_load(
                            ScalarDestination::Usize(destination),
                            operand,
                            context,
                        )? {
                            instructions.push(instruction);
                        } else {
                            instructions.push(Instruction::SetUsize {
                                destination,
                                value: lower_usize_operand(operand, context)?,
                            });
                        }
                    }
                    Rvalue::Cast {
                        operand,
                        source_scalar,
                        ..
                    } => instructions.push(Instruction::SetUsize {
                        destination,
                        value: lower_cast_to_word(*source_scalar, operand, context)?,
                    }),
                    Rvalue::Unary { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "usize scalar route received an invalid unary operation",
                        ));
                    }
                    Rvalue::Binary {
                        operator,
                        left,
                        right,
                        ..
                    } => instructions.push(usize_binary_instruction(
                        *operator,
                        destination,
                        lower_usize_operand(left, context)?,
                        lower_usize_operand(right, context)?,
                    )),
                    Rvalue::Compare { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "usize scalar route received a comparison result",
                        ));
                    }
                }
            }
            ScalarType::Integer(kind) => {
                let destination = integer_location(destination, kind, context)?;
                match value {
                    Rvalue::Variant { .. } => {
                        unreachable!("aggregate rvalue handled above")
                    }
                    Rvalue::Use(operand) => {
                        if let Some(instruction) = aggregate_scalar_load(
                            ScalarDestination::Integer(kind, destination),
                            operand,
                            context,
                        )? {
                            instructions.push(instruction);
                        } else {
                            instructions.push(Instruction::SetUsize {
                                destination,
                                value: lower_integer_operand(operand, kind, context)?,
                            });
                        }
                    }
                    Rvalue::Cast {
                        operand,
                        source_scalar,
                        ..
                    } => instructions.push(Instruction::SetUsize {
                        destination,
                        value: lower_cast_to_word(*source_scalar, operand, context)?,
                    }),
                    Rvalue::Unary {
                        operator: UnaryOperator::Negate,
                        operand,
                        ..
                    } if kind.is_signed() => instructions.push(Instruction::IntegerBinary {
                        kind,
                        operator: IntegerBinaryOperator::Subtract,
                        destination,
                        left: UsizeValue::Const(0),
                        right: lower_integer_operand(operand, kind, context)?,
                    }),
                    Rvalue::Unary { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "integer scalar route received an invalid unary operation",
                        ));
                    }
                    Rvalue::Binary {
                        operator,
                        left,
                        right,
                        ..
                    } => instructions.push(Instruction::IntegerBinary {
                        kind,
                        operator: integer_binary_operator(*operator),
                        destination,
                        left: lower_integer_operand(left, kind, context)?,
                        right: lower_integer_operand(right, kind, context)?,
                    }),
                    Rvalue::Compare { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "integer scalar route received a comparison result",
                        ));
                    }
                }
            }
            ScalarType::Bool => {
                let destination = bool_location(destination, context)?;
                match value {
                    Rvalue::Variant { .. } => {
                        unreachable!("aggregate rvalue handled above")
                    }
                    Rvalue::Use(operand) => {
                        if let Some(instruction) = aggregate_scalar_load(
                            ScalarDestination::Bool(destination),
                            operand,
                            context,
                        )? {
                            instructions.push(instruction);
                        } else {
                            instructions.push(Instruction::SetBool {
                                destination,
                                value: lower_bool_operand(operand, context)?,
                            });
                        }
                    }
                    Rvalue::Cast {
                        operand,
                        source_scalar: ScalarType::Bool,
                        ..
                    } => instructions.push(Instruction::SetBool {
                        destination,
                        value: lower_bool_operand(operand, context)?,
                    }),
                    Rvalue::Cast { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "boolean scalar route received a numeric cast",
                        ));
                    }
                    Rvalue::Unary {
                        operator: UnaryOperator::LogicalNot,
                        operand,
                        ..
                    } => instructions.push(Instruction::SetBool {
                        destination,
                        value: BoolValue::Not(Box::new(lower_bool_operand(operand, context)?)),
                    }),
                    Rvalue::Unary { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "boolean scalar route received a numeric unary operation",
                        ));
                    }
                    Rvalue::Binary { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "boolean scalar route received an arithmetic operation",
                        ));
                    }
                    Rvalue::Compare {
                        operator,
                        left,
                        right,
                        operand_scalar,
                        ..
                    } => instructions.push(Instruction::SetBool {
                        destination,
                        value: lower_comparison(*operator, left, right, *operand_scalar, context)?,
                    }),
                }
            }
        }
    }
    Ok(instructions)
}

fn lower_region_enter(
    region: crate::mir::RegionId,
    context: &BackendContext<'_>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    const REGION_ALLOCATOR_KIND: u64 = 1;
    let region = context
        .body
        .allocation_regions
        .get(region.index())
        .ok_or_else(|| invalid_mir_diagnostics("MIR region declaration is missing"))?;
    let (state_offset, kind_offset) =
        allocator_field_offsets(context.body.locals[region.allocator.index()].ty, context)?;
    let allocator = Place::local(region.allocator);
    let allocator_location = aggregate_location(&allocator, context)?;
    let parent_state = usize_location(&Place::local(region.parent_state), context)?;
    let parent_kind = usize_location(&Place::local(region.parent_kind), context)?;
    let state = usize_location(&Place::local(region.state), context)?;
    let mut instructions = Vec::new();
    reserve_aggregate_destination(context, &allocator, &mut instructions)?;
    instructions.extend([
        Instruction::SetUsize {
            destination: parent_state,
            value: UsizeValue::CurrentAllocationState,
        },
        Instruction::SetUsize {
            destination: parent_kind,
            value: UsizeValue::CurrentAllocationKind,
        },
        Instruction::RegionEnter { destination: state },
        Instruction::StoreAggregateUsize {
            destination: allocator_location,
            offset: state_offset,
            value: UsizeValue::Location(state),
        },
        Instruction::StoreAggregateUsize {
            destination: allocator_location,
            offset: kind_offset,
            value: UsizeValue::Const(REGION_ALLOCATOR_KIND),
        },
        Instruction::SetCurrentAllocationContext {
            state: UsizeValue::Location(state),
            kind: UsizeValue::Const(REGION_ALLOCATOR_KIND),
        },
    ]);
    Ok(instructions)
}

fn lower_region_exit(
    region: crate::mir::RegionId,
    context: &BackendContext<'_>,
) -> Result<Instruction, Vec<Diagnostic>> {
    let region = context
        .body
        .allocation_regions
        .get(region.index())
        .ok_or_else(|| invalid_mir_diagnostics("MIR region declaration is missing"))?;
    Ok(Instruction::RegionRelease {
        state: UsizeValue::Location(usize_location(&Place::local(region.state), context)?),
        parent_state: UsizeValue::Location(usize_location(
            &Place::local(region.parent_state),
            context,
        )?),
        parent_kind: UsizeValue::Location(usize_location(
            &Place::local(region.parent_kind),
            context,
        )?),
    })
}

fn allocator_field_offsets(
    ty: crate::semantic::TyId,
    context: &BackendContext<'_>,
) -> Result<(u32, u32), Vec<Diagnostic>> {
    let value = aggregate_local_abi_value(ty, context)?;
    let crate::abi::AbiType::Struct(fields) = value.ty else {
        return Err(invalid_mir_diagnostics(
            "MIR region allocator is not struct storage",
        ));
    };
    let layout = crate::abi::layout_struct(&fields)
        .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
    let offset = |name: &str| {
        fields
            .iter()
            .position(|field| field.name == name && field.ty == crate::abi::AbiType::Usize)
            .and_then(|index| layout.fields.get(index))
            .and_then(|field| u32::try_from(field.offset).ok())
            .ok_or_else(|| {
                invalid_mir_diagnostics(format!("MIR region allocator is missing `{name}: usize`"))
            })
    };
    Ok((offset("state")?, offset("kind")?))
}

fn aggregate_leaf_projection<'a>(
    root: &'a crate::abi::AbiType,
    path: &[crate::mir::AggregateElement],
) -> Result<(u32, &'a crate::abi::AbiType), Vec<Diagnostic>> {
    if path.is_empty() {
        return Err(invalid_mir_diagnostics(
            "aggregate MIR leaf has an empty projection path",
        ));
    }
    let mut ty = root;
    let mut offset = 0_u64;
    for element in path {
        match (element, ty) {
            (crate::mir::AggregateElement::Field(index), crate::abi::AbiType::Struct(fields)) => {
                let layout = crate::abi::layout_struct(fields)
                    .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
                let field = fields.get(*index).ok_or_else(|| {
                    invalid_mir_diagnostics("aggregate MIR field path is outside its struct")
                })?;
                let field_layout = layout.fields.get(*index).ok_or_else(|| {
                    invalid_mir_diagnostics("aggregate MIR field has no ABI layout")
                })?;
                offset = offset.checked_add(field_layout.offset).ok_or_else(|| {
                    invalid_mir_diagnostics("aggregate MIR field offset overflowed")
                })?;
                ty = &field.ty;
            }
            (
                crate::mir::AggregateElement::Index(index),
                crate::abi::AbiType::Array { element, length },
            ) => {
                let index = u64::try_from(*index).map_err(|_| {
                    invalid_mir_diagnostics("aggregate MIR array index is not representable")
                })?;
                if index >= *length {
                    return Err(invalid_mir_diagnostics(
                        "aggregate MIR array index is outside its array",
                    ));
                }
                let stride = crate::abi::array_element_stride(element)
                    .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
                offset = offset
                    .checked_add(stride.checked_mul(index).ok_or_else(|| {
                        invalid_mir_diagnostics("aggregate MIR array offset overflowed")
                    })?)
                    .ok_or_else(|| {
                        invalid_mir_diagnostics("aggregate MIR array offset overflowed")
                    })?;
                ty = element;
            }
            _ => {
                return Err(invalid_mir_diagnostics(
                    "aggregate MIR path does not match its aggregate layout",
                ));
            }
        }
    }
    let offset = u32::try_from(offset)
        .map_err(|_| invalid_mir_diagnostics("aggregate MIR leaf offset is not representable"))?;
    Ok((offset, ty))
}

fn enum_variant_signature<'a>(
    context: &'a BackendContext<'_>,
    definition: crate::semantic::DefId,
) -> Result<&'a crate::resolve::EnumVariantSignature, Vec<Diagnostic>> {
    let anchor = context
        .resolved
        .semantic_db
        .definition_anchor(definition)
        .ok_or_else(|| invalid_mir_diagnostics("enum variant has no semantic anchor"))?;
    let owner = context
        .resolved
        .enum_variant_owner(definition)
        .ok_or_else(|| invalid_mir_diagnostics("enum variant has no semantic owner"))?;
    owner
        .variants
        .iter()
        .find(|variant| variant.name_span == anchor)
        .ok_or_else(|| invalid_mir_diagnostics("enum variant signature is missing"))
}

fn enum_variant_leaf_projection<'a>(
    enum_: &'a crate::abi::AbiEnum,
    variant: &'a crate::abi::AbiEnumVariant,
    path: &[crate::mir::AggregateElement],
) -> Result<(u32, &'a crate::abi::AbiType), Vec<Diagnostic>> {
    let Some(crate::mir::AggregateElement::VariantPayload(payload_index)) = path.first() else {
        return Err(invalid_mir_diagnostics(
            "variant MIR leaf has no payload projection",
        ));
    };
    let payload = variant
        .payload
        .as_ref()
        .ok_or_else(|| invalid_mir_diagnostics("payloadless MIR variant has payload leaves"))?;
    let (payload_offset, payload_ty) = match payload {
        crate::abi::AbiType::Struct(fields) if fields.len() > 1 => {
            let layout = crate::abi::layout_struct(fields)
                .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
            let field = fields.get(*payload_index).ok_or_else(|| {
                invalid_mir_diagnostics("variant MIR payload field is outside its variant")
            })?;
            let offset = layout
                .fields
                .get(*payload_index)
                .map(|field| field.offset)
                .ok_or_else(|| invalid_mir_diagnostics("variant MIR payload layout is missing"))?;
            (offset, &field.ty)
        }
        payload if *payload_index == 0 => (0, payload),
        _ => {
            return Err(invalid_mir_diagnostics(
                "variant MIR payload index does not match its ABI",
            ));
        }
    };
    let (nested_offset, leaf) = if path.len() == 1 {
        (0, payload_ty)
    } else {
        aggregate_leaf_projection(payload_ty, &path[1..])?
    };
    let offset = enum_
        .payload_offset
        .checked_add(payload_offset)
        .and_then(|offset| offset.checked_add(u64::from(nested_offset)))
        .and_then(|offset| u32::try_from(offset).ok())
        .ok_or_else(|| invalid_mir_diagnostics("variant MIR leaf offset is invalid"))?;
    Ok((offset, leaf))
}

fn abi_type_matches_scalar(abi: &crate::abi::AbiType, scalar: ScalarType) -> bool {
    match scalar {
        ScalarType::I32 => *abi == crate::abi::AbiType::I32,
        ScalarType::U8 => *abi == crate::abi::AbiType::U8,
        ScalarType::Usize => *abi == crate::abi::AbiType::Usize,
        ScalarType::Bool => *abi == crate::abi::AbiType::Bool,
        ScalarType::Integer(kind) => abi.integer_type() == Some(kind),
    }
}

fn store_aggregate_scalar(
    destination: crate::ir::AggregateLocation,
    offset: u32,
    scalar: ScalarType,
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<Instruction, Vec<Diagnostic>> {
    Ok(match scalar {
        ScalarType::I32 => Instruction::StoreAggregateI32 {
            destination,
            offset,
            value: lower_i32_operand(operand, context)?,
        },
        ScalarType::U8 => Instruction::StoreAggregateU8 {
            destination,
            offset,
            value: lower_u8_operand(operand, context)?,
        },
        ScalarType::Usize => Instruction::StoreAggregateUsize {
            destination,
            offset,
            value: lower_usize_operand(operand, context)?,
        },
        ScalarType::Integer(kind) => Instruction::StoreAggregateInteger {
            kind,
            destination,
            offset,
            value: lower_integer_operand(operand, kind, context)?,
        },
        ScalarType::Bool => Instruction::StoreAggregateBool {
            destination,
            offset,
            value: lower_bool_operand(operand, context)?,
        },
    })
}

fn lower_borrow_source(
    place: Place,
    context: &BackendContext<'_>,
) -> Result<crate::ir::BorrowSource, Vec<Diagnostic>> {
    if let Some(projection) = place.projection {
        let projection = aggregate_borrow_projection(context.body, place.local, projection)?;
        let local = &context.body.locals[place.local.index()];
        if let AggregateBorrowProjection::Index {
            base_offset,
            index,
            length,
            stride,
        } = projection
        {
            if local.representation != crate::mir::ValueRepresentation::Aggregate {
                return Err(invalid_mir_diagnostics(
                    "indexed MIR loan base is not aggregate storage",
                ));
            }
            return Ok(crate::ir::BorrowSource::AggregateIndex {
                source: aggregate_location(&Place::local(place.local), context)?,
                base_offset,
                index: match lower_direct_usize_index(&index, context)? {
                    UsizeValue::Const(value) => crate::ir::SliceElementIndex::Const(value),
                    UsizeValue::Location(location) => {
                        crate::ir::SliceElementIndex::Location(location)
                    }
                    _ => unreachable!("direct index validation accepts only constants and places"),
                },
                length,
                stride,
            });
        }
        let AggregateBorrowProjection::Field { offset } = projection else {
            unreachable!("indexed projection returned above")
        };
        if local.representation == crate::mir::ValueRepresentation::Borrow {
            return match local.storage {
                LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
                    Some(parameters::ParameterStorage::Borrow { abi_index }) => {
                        Ok(crate::ir::BorrowSource::AggregateParameterField {
                            parameter_index: abi_index,
                            offset,
                        })
                    }
                    _ => Err(invalid_mir_diagnostics(
                        "borrow MIR parameter has no matching ABI projection",
                    )),
                },
                LocalStorage::Local => Ok(crate::ir::BorrowSource::BorrowLocalField {
                    pointer: UsizeLocation::Local(machine_local_index(context.body, place.local)),
                    offset,
                }),
                LocalStorage::Return => Err(invalid_mir_diagnostics(
                    "return borrow storage cannot be projected",
                )),
            };
        }
        let location = aggregate_location(&Place::local(place.local), context)?;
        let crate::ir::AggregateLocation::Slot(slot_index) = location else {
            return Err(invalid_mir_diagnostics(
                "projected MIR loan source is not backed by an aggregate slot",
            ));
        };
        return Ok(if offset == 0 {
            crate::ir::BorrowSource::AggregateSlot(slot_index)
        } else {
            crate::ir::BorrowSource::AggregateSlotField { slot_index, offset }
        });
    }
    let local = &context.body.locals[place.local.index()];
    Ok(match local.representation {
        crate::mir::ValueRepresentation::Scalar(ScalarType::I32) => {
            crate::ir::BorrowSource::I32(i32_location(&place, context)?)
        }
        crate::mir::ValueRepresentation::Scalar(ScalarType::U8) => {
            crate::ir::BorrowSource::U8(u8_location(&place, context)?)
        }
        crate::mir::ValueRepresentation::Scalar(ScalarType::Usize)
        | crate::mir::ValueRepresentation::Scalar(ScalarType::Integer(_)) => {
            crate::ir::BorrowSource::Usize(match local.representation {
                crate::mir::ValueRepresentation::Scalar(ScalarType::Integer(kind)) => {
                    integer_location(&place, kind, context)?
                }
                _ => usize_location(&place, context)?,
            })
        }
        crate::mir::ValueRepresentation::Scalar(ScalarType::Bool) => {
            crate::ir::BorrowSource::Bool(bool_location(&place, context)?)
        }
        crate::mir::ValueRepresentation::Borrow => {
            return lower_borrow_argument_source(&Operand::Copy(place), context);
        }
        crate::mir::ValueRepresentation::View(_) => {
            return Err(invalid_mir_diagnostics(
                "view values cannot be borrowed as scalar places",
            ));
        }
        crate::mir::ValueRepresentation::Error => {
            return Err(invalid_mir_diagnostics(
                "logical error values cannot be borrowed as scalar places",
            ));
        }
        crate::mir::ValueRepresentation::Aggregate => match local.storage {
            LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
                Some(parameters::ParameterStorage::Aggregate { slot_index, .. }) => {
                    crate::ir::BorrowSource::AggregateSlot(slot_index)
                }
                _ => {
                    return Err(invalid_mir_diagnostics(
                        "aggregate MIR loan source has no matching storage projection",
                    ));
                }
            },
            LocalStorage::Local => {
                let crate::ir::AggregateLocation::Slot(slot_index) =
                    aggregate_location(&place, context)?
                else {
                    return Err(invalid_mir_diagnostics(
                        "aggregate MIR loan source is not backed by a local slot",
                    ));
                };
                crate::ir::BorrowSource::AggregateSlot(slot_index)
            }
            LocalStorage::Return => {
                return Err(invalid_mir_diagnostics(
                    "aggregate return storage cannot be borrowed",
                ));
            }
        },
    })
}

fn error_locations(
    body: &Body,
    local: LocalId,
) -> Result<(StrLocation, StrLocation), Vec<Diagnostic>> {
    let Some(declaration) = body.locals.get(local.index()) else {
        return Err(invalid_mir_diagnostics(
            "error payload refers to a missing MIR local",
        ));
    };
    if declaration.storage != LocalStorage::Local
        || declaration.representation != crate::mir::ValueRepresentation::Error
    {
        return Err(invalid_mir_diagnostics(
            "error payload is not backed by logical local storage",
        ));
    }
    let base = storage::machine_local_index(body, local);
    Ok((StrLocation::Local(base), StrLocation::Local(base + 2)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AggregateBorrowProjection {
    Field {
        offset: u32,
    },
    Index {
        base_offset: u32,
        index: Operand,
        length: u64,
        stride: u32,
    },
}

fn aggregate_borrow_projection(
    body: &Body,
    base: LocalId,
    mut projection: crate::mir::ProjectionPathId,
) -> Result<AggregateBorrowProjection, Vec<Diagnostic>> {
    let mut elements = Vec::new();
    loop {
        let path = body
            .projections
            .get(projection.index())
            .ok_or_else(|| invalid_mir_diagnostics("aggregate projection is missing"))?;
        if path.base != base {
            return Err(invalid_mir_diagnostics(
                "aggregate projection changed base local",
            ));
        }
        elements.push(path.element.clone());
        let Some(parent) = path.parent else {
            break;
        };
        projection = parent;
    }
    elements.reverse();

    let mut offset = 0u32;
    let mut index = None;
    for element in elements {
        match element {
            crate::mir::ProjectionElement::Field {
                offset: field_offset,
            } => {
                offset = offset
                    .checked_add(field_offset)
                    .ok_or_else(|| invalid_mir_diagnostics("aggregate field offset overflowed"))?;
            }
            crate::mir::ProjectionElement::Index {
                index: operand,
                length,
                stride,
            } => {
                if let Operand::Constant(constant) = &operand {
                    if constant.scalar != ScalarType::Usize || constant.value >= u128::from(length)
                    {
                        return Err(invalid_mir_diagnostics(
                            "constant MIR aggregate index is out of bounds",
                        ));
                    }
                    let indexed_offset = constant
                        .value
                        .checked_mul(u128::from(stride))
                        .and_then(|indexed| u128::from(offset).checked_add(indexed))
                        .and_then(|indexed| u32::try_from(indexed).ok())
                        .ok_or_else(|| {
                            invalid_mir_diagnostics("aggregate index offset overflowed")
                        })?;
                    offset = indexed_offset;
                    continue;
                }
                if index.is_some() {
                    return Err(invalid_mir_diagnostics(
                        "nested MIR indexes require a multidimensional machine projection",
                    ));
                }
                index = Some((operand, length, stride));
            }
            crate::mir::ProjectionElement::ErrorField(_) => {
                return Err(invalid_mir_diagnostics(
                    "error field cannot participate in an aggregate projection",
                ));
            }
        }
    }
    Ok(match index {
        Some((index, length, stride)) => AggregateBorrowProjection::Index {
            base_offset: offset,
            index,
            length,
            stride,
        },
        None => AggregateBorrowProjection::Field { offset },
    })
}

#[derive(Debug, Clone, Copy)]
enum ScalarDestination {
    I32(I32Location),
    U8(U8Location),
    Usize(UsizeLocation),
    Integer(crate::integer::IntegerType, UsizeLocation),
    Bool(BoolLocation),
}

fn aggregate_scalar_store(
    destination: &Place,
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<Instruction, Vec<Diagnostic>> {
    let projection_id = destination.projection.ok_or_else(|| {
        invalid_mir_diagnostics("aggregate scalar store has no destination projection")
    })?;
    let contract = context
        .body
        .projections
        .get(projection_id.index())
        .filter(|projection| projection.base == destination.local)
        .ok_or_else(|| invalid_mir_diagnostics("aggregate scalar store projection is missing"))?;
    let crate::mir::ValueRepresentation::Scalar(scalar) = contract.representation else {
        return Err(invalid_mir_diagnostics(
            "aggregate scalar store projection is not scalar",
        ));
    };
    let location = aggregate_location(&Place::local(destination.local), context)?;
    match aggregate_borrow_projection(context.body, destination.local, projection_id)? {
        AggregateBorrowProjection::Field { offset } => {
            store_aggregate_scalar(location, offset, scalar, operand, context)
        }
        AggregateBorrowProjection::Index {
            base_offset,
            index,
            length,
            stride,
        } => {
            let index = lower_direct_usize_index(&index, context)?;
            Ok(match scalar {
                ScalarType::I32 => Instruction::StoreAggregateI32Indexed {
                    destination: location,
                    base_offset,
                    index,
                    length,
                    stride,
                    value: lower_i32_operand(operand, context)?,
                },
                ScalarType::U8 => Instruction::StoreAggregateU8Indexed {
                    destination: location,
                    base_offset,
                    index,
                    length,
                    stride,
                    value: lower_u8_operand(operand, context)?,
                },
                ScalarType::Usize => Instruction::StoreAggregateUsizeIndexed {
                    destination: location,
                    base_offset,
                    index,
                    length,
                    stride,
                    value: lower_usize_operand(operand, context)?,
                },
                ScalarType::Integer(kind) => Instruction::StoreAggregateIntegerIndexed {
                    kind,
                    destination: location,
                    base_offset,
                    index,
                    length,
                    stride,
                    value: lower_integer_operand(operand, kind, context)?,
                },
                ScalarType::Bool => Instruction::StoreAggregateBoolIndexed {
                    destination: location,
                    base_offset,
                    index,
                    length,
                    stride,
                    value: lower_bool_operand(operand, context)?,
                },
            })
        }
    }
}

fn aggregate_scalar_load(
    destination: ScalarDestination,
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<Option<Instruction>, Vec<Diagnostic>> {
    let place = match operand {
        Operand::Constant(_) | Operand::StaticStr { .. } => return Ok(None),
        Operand::Copy(place) | Operand::Move(place) => place,
    };
    let Some(projection) = place.projection else {
        return Ok(None);
    };
    let source = aggregate_location(&Place::local(place.local), context)?;
    let projection = aggregate_borrow_projection(context.body, place.local, projection)?;
    Ok(Some(match projection {
        AggregateBorrowProjection::Field { offset } => match destination {
            ScalarDestination::I32(destination) => Instruction::LoadAggregateI32 {
                destination,
                source,
                offset,
            },
            ScalarDestination::U8(destination) => Instruction::LoadAggregateU8 {
                destination,
                source,
                offset,
            },
            ScalarDestination::Usize(destination) => Instruction::LoadAggregateUsize {
                destination,
                source,
                offset,
            },
            ScalarDestination::Integer(kind, destination) => Instruction::LoadAggregateInteger {
                kind,
                destination,
                source,
                offset,
            },
            ScalarDestination::Bool(destination) => Instruction::LoadAggregateBool {
                destination,
                source,
                offset,
            },
        },
        AggregateBorrowProjection::Index {
            base_offset,
            index,
            length,
            stride,
        } => {
            let index = lower_direct_usize_index(&index, context)?;
            match destination {
                ScalarDestination::I32(destination) => Instruction::LoadAggregateI32Indexed {
                    destination,
                    source,
                    base_offset,
                    index,
                    length,
                    stride,
                },
                ScalarDestination::U8(destination) => Instruction::LoadAggregateU8Indexed {
                    destination,
                    source,
                    base_offset,
                    index,
                    length,
                    stride,
                },
                ScalarDestination::Usize(destination) => Instruction::LoadAggregateUsizeIndexed {
                    destination,
                    source,
                    base_offset,
                    index,
                    length,
                    stride,
                },
                ScalarDestination::Integer(kind, destination) => {
                    Instruction::LoadAggregateIntegerIndexed {
                        kind,
                        destination,
                        source,
                        base_offset,
                        index,
                        length,
                        stride,
                    }
                }
                ScalarDestination::Bool(destination) => Instruction::LoadAggregateBoolIndexed {
                    destination,
                    source,
                    base_offset,
                    index,
                    length,
                    stride,
                },
            }
        }
    }))
}

fn lower_direct_usize_index(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match lower_usize_operand(operand, context)? {
        value @ (UsizeValue::Const(_) | UsizeValue::Location(_)) => Ok(value),
        _ => Err(invalid_mir_diagnostics(
            "MIR index operand did not lower to a direct usize value",
        )),
    }
}

fn aggregate_field_offset(
    body: &Body,
    base: LocalId,
    mut projection: crate::mir::ProjectionPathId,
) -> Result<u32, Vec<Diagnostic>> {
    let mut offset = 0u32;
    loop {
        let path = body
            .projections
            .get(projection.index())
            .ok_or_else(|| invalid_mir_diagnostics("aggregate field projection is missing"))?;
        if path.base != base {
            return Err(invalid_mir_diagnostics(
                "aggregate projection changed base local",
            ));
        }
        let crate::mir::ProjectionElement::Field {
            offset: field_offset,
        } = path.element
        else {
            return Err(invalid_mir_diagnostics(
                "indexed aggregate projection has not been projected to machine IR",
            ));
        };
        offset = offset
            .checked_add(field_offset)
            .ok_or_else(|| invalid_mir_diagnostics("aggregate field offset overflowed"))?;
        let Some(parent) = path.parent else {
            return Ok(offset);
        };
        projection = parent;
    }
}

fn lower_comparison(
    operator: ComparisonOperator,
    left: &Operand,
    right: &Operand,
    operand_scalar: ScalarType,
    context: &BackendContext<'_>,
) -> Result<BoolValue, Vec<Diagnostic>> {
    Ok(match operand_scalar {
        ScalarType::I32 => BoolValue::I32Comparison {
            operator: integer_comparison_operator(operator),
            left: lower_i32_operand(left, context)?,
            right: lower_i32_operand(right, context)?,
        },
        ScalarType::U8 => BoolValue::IntegerComparison {
            kind: crate::integer::IntegerType::U8,
            operator: integer_comparison_operator(operator),
            left: UsizeValue::U8ZeroExtend(Box::new(lower_u8_operand(left, context)?)),
            right: UsizeValue::U8ZeroExtend(Box::new(lower_u8_operand(right, context)?)),
        },
        ScalarType::Usize => BoolValue::UsizeComparison {
            operator: integer_comparison_operator(operator),
            left: lower_usize_operand(left, context)?,
            right: lower_usize_operand(right, context)?,
        },
        ScalarType::Integer(kind) => BoolValue::IntegerComparison {
            kind,
            operator: integer_comparison_operator(operator),
            left: lower_integer_operand(left, kind, context)?,
            right: lower_integer_operand(right, kind, context)?,
        },
        ScalarType::Bool => BoolValue::BoolComparison {
            operator: bool_comparison_operator(operator)?,
            left: Box::new(lower_bool_operand(left, context)?),
            right: Box::new(lower_bool_operand(right, context)?),
        },
    })
}

fn integer_comparison_operator(operator: ComparisonOperator) -> I32ComparisonOperator {
    match operator {
        ComparisonOperator::Equal => I32ComparisonOperator::Equal,
        ComparisonOperator::NotEqual => I32ComparisonOperator::NotEqual,
        ComparisonOperator::Less => I32ComparisonOperator::Less,
        ComparisonOperator::LessEqual => I32ComparisonOperator::LessEqual,
        ComparisonOperator::Greater => I32ComparisonOperator::Greater,
        ComparisonOperator::GreaterEqual => I32ComparisonOperator::GreaterEqual,
    }
}

fn bool_comparison_operator(
    operator: ComparisonOperator,
) -> Result<BoolComparisonOperator, Vec<Diagnostic>> {
    match operator {
        ComparisonOperator::Equal => Ok(BoolComparisonOperator::Equal),
        ComparisonOperator::NotEqual => Ok(BoolComparisonOperator::NotEqual),
        ComparisonOperator::Less
        | ComparisonOperator::LessEqual
        | ComparisonOperator::Greater
        | ComparisonOperator::GreaterEqual => Err(invalid_mir_diagnostics(
            "boolean scalar route received an ordered comparison",
        )),
    }
}

fn attach_primary_span(
    diagnostics: Vec<Diagnostic>,
    sources: &SourceMap,
    span: ByteSpan,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.with_primary_span_if_absent(sources, span))
        .collect()
}

fn i32_binary_instruction(
    operator: BinaryOperator,
    destination: I32Location,
    left: I32Value,
    right: I32Value,
) -> Instruction {
    match operator {
        BinaryOperator::Add => Instruction::AddI32 {
            destination,
            left,
            right,
        },
        BinaryOperator::Subtract => Instruction::SubtractI32 {
            destination,
            left,
            right,
        },
        BinaryOperator::Multiply => Instruction::MultiplyI32 {
            destination,
            left,
            right,
        },
        BinaryOperator::Divide => Instruction::DivideI32 {
            destination,
            left,
            right,
        },
        BinaryOperator::Remainder => Instruction::RemainderI32 {
            destination,
            left,
            right,
        },
        BinaryOperator::ShiftLeft => Instruction::ShiftLeftI32 {
            destination,
            left,
            right,
        },
        BinaryOperator::ShiftRight => Instruction::ShiftRightI32 {
            destination,
            left,
            right,
        },
    }
}

fn u8_binary_instruction(
    operator: BinaryOperator,
    destination: U8Location,
    left: U8Value,
    right: U8Value,
) -> Instruction {
    match operator {
        BinaryOperator::Add => Instruction::AddU8 {
            destination,
            left,
            right,
        },
        BinaryOperator::Subtract => Instruction::SubtractU8 {
            destination,
            left,
            right,
        },
        BinaryOperator::Multiply => Instruction::MultiplyU8 {
            destination,
            left,
            right,
        },
        BinaryOperator::Divide => Instruction::DivideU8 {
            destination,
            left,
            right,
        },
        BinaryOperator::Remainder => Instruction::RemainderU8 {
            destination,
            left,
            right,
        },
        BinaryOperator::ShiftLeft => Instruction::ShiftLeftU8 {
            destination,
            left,
            right,
        },
        BinaryOperator::ShiftRight => Instruction::ShiftRightU8 {
            destination,
            left,
            right,
        },
    }
}

fn usize_binary_instruction(
    operator: BinaryOperator,
    destination: UsizeLocation,
    left: UsizeValue,
    right: UsizeValue,
) -> Instruction {
    match operator {
        BinaryOperator::Add => Instruction::AddUsize {
            destination,
            left,
            right,
        },
        BinaryOperator::Subtract => Instruction::SubtractUsize {
            destination,
            left,
            right,
        },
        BinaryOperator::Multiply => Instruction::MultiplyUsize {
            destination,
            left,
            right,
        },
        BinaryOperator::Divide => Instruction::DivideUsize {
            destination,
            left,
            right,
        },
        BinaryOperator::Remainder => Instruction::RemainderUsize {
            destination,
            left,
            right,
        },
        BinaryOperator::ShiftLeft => Instruction::ShiftLeftUsize {
            destination,
            left,
            right,
        },
        BinaryOperator::ShiftRight => Instruction::ShiftRightUsize {
            destination,
            left,
            right,
        },
    }
}

fn integer_binary_operator(operator: BinaryOperator) -> IntegerBinaryOperator {
    match operator {
        BinaryOperator::Add => IntegerBinaryOperator::Add,
        BinaryOperator::Subtract => IntegerBinaryOperator::Subtract,
        BinaryOperator::Multiply => IntegerBinaryOperator::Multiply,
        BinaryOperator::Divide => IntegerBinaryOperator::Divide,
        BinaryOperator::Remainder => IntegerBinaryOperator::Remainder,
        BinaryOperator::ShiftLeft => IntegerBinaryOperator::ShiftLeft,
        BinaryOperator::ShiftRight => IntegerBinaryOperator::ShiftRight,
    }
}

fn i32_location(
    place: &Place,
    context: &BackendContext<'_>,
) -> Result<I32Location, Vec<Diagnostic>> {
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Return => Ok(I32Location::Return),
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::I32 { abi_index }) => {
                Ok(I32Location::Parameter(abi_index))
            }
            _ => Err(invalid_mir_diagnostics(
                "i32 MIR parameter has no matching ABI projection",
            )),
        },
        LocalStorage::Local => Ok(I32Location::Local(machine_local_index(
            context.body,
            place.local,
        ))),
    }
}

fn u8_location(place: &Place, context: &BackendContext<'_>) -> Result<U8Location, Vec<Diagnostic>> {
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Return => Ok(U8Location::Return),
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::U8 { abi_index }) => {
                Ok(U8Location::Parameter(abi_index))
            }
            _ => Err(invalid_mir_diagnostics(
                "u8 MIR parameter has no matching ABI projection",
            )),
        },
        LocalStorage::Local => Ok(U8Location::Local(machine_local_index(
            context.body,
            place.local,
        ))),
    }
}

fn usize_location(
    place: &Place,
    context: &BackendContext<'_>,
) -> Result<UsizeLocation, Vec<Diagnostic>> {
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Return => Ok(UsizeLocation::Return),
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::Usize { abi_index }) => {
                Ok(UsizeLocation::Parameter(abi_index))
            }
            _ => Err(invalid_mir_diagnostics(
                "usize MIR parameter has no matching ABI projection",
            )),
        },
        LocalStorage::Local => Ok(UsizeLocation::Local(machine_local_index(
            context.body,
            place.local,
        ))),
    }
}

fn integer_location(
    place: &Place,
    kind: crate::integer::IntegerType,
    context: &BackendContext<'_>,
) -> Result<UsizeLocation, Vec<Diagnostic>> {
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Return => Ok(UsizeLocation::Return),
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::Integer {
                kind: actual,
                abi_index,
            }) if actual == kind => Ok(UsizeLocation::Parameter(abi_index)),
            _ => Err(invalid_mir_diagnostics(
                "integer MIR parameter has no matching ABI projection",
            )),
        },
        LocalStorage::Local => Ok(UsizeLocation::Local(machine_local_index(
            context.body,
            place.local,
        ))),
    }
}

fn bool_location(
    place: &Place,
    context: &BackendContext<'_>,
) -> Result<BoolLocation, Vec<Diagnostic>> {
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Return => Ok(BoolLocation::Return),
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::Bool { abi_index }) => {
                Ok(BoolLocation::Parameter(abi_index))
            }
            _ => Err(invalid_mir_diagnostics(
                "bool MIR parameter has no matching ABI projection",
            )),
        },
        LocalStorage::Local => Ok(BoolLocation::Local(machine_local_index(
            context.body,
            place.local,
        ))),
    }
}

fn machine_local_index(body: &Body, local: LocalId) -> usize {
    storage::machine_local_index(body, local)
}

fn local_scalar(body: &Body, local: LocalId) -> Result<ScalarType, Vec<Diagnostic>> {
    body.locals[local.index()]
        .scalar_type()
        .ok_or_else(|| invalid_mir_diagnostics("scalar MIR lowering received a non-scalar local"))
}

fn lower_cast_to_i32(
    source: ScalarType,
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<I32Value, Vec<Diagnostic>> {
    match source {
        ScalarType::I32 => lower_i32_operand(operand, context),
        ScalarType::U8 => Ok(I32Value::U8ZeroExtend(Box::new(lower_u8_operand(
            operand, context,
        )?))),
        ScalarType::Integer(kind) => Ok(I32Value::IntegerWord(Box::new(lower_integer_operand(
            operand, kind, context,
        )?))),
        ScalarType::Usize | ScalarType::Bool => Err(invalid_mir_diagnostics(
            "i32 MIR destination received a non-lossless scalar cast",
        )),
    }
}

fn lower_cast_to_u8(
    source: ScalarType,
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<U8Value, Vec<Diagnostic>> {
    match source {
        ScalarType::U8 => lower_u8_operand(operand, context),
        ScalarType::I32 | ScalarType::Usize | ScalarType::Integer(_) | ScalarType::Bool => Err(
            invalid_mir_diagnostics("u8 MIR destination received a non-lossless scalar cast"),
        ),
    }
}

fn lower_cast_to_word(
    source: ScalarType,
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match source {
        ScalarType::I32 => Ok(UsizeValue::I32SignExtend(Box::new(lower_i32_operand(
            operand, context,
        )?))),
        ScalarType::U8 => Ok(UsizeValue::U8ZeroExtend(Box::new(lower_u8_operand(
            operand, context,
        )?))),
        ScalarType::Usize => lower_usize_operand(operand, context),
        ScalarType::Integer(kind) => lower_integer_operand(operand, kind, context),
        ScalarType::Bool => Err(invalid_mir_diagnostics(
            "integer MIR destination received a boolean cast",
        )),
    }
}

fn lower_i32_operand(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<I32Value, Vec<Diagnostic>> {
    match operand {
        Operand::Constant(constant) => {
            i32::try_from(constant.value)
                .map(I32Value::Const)
                .map_err(|_| {
                    invalid_mir_diagnostics("i32 constant is outside its runtime representation")
                })
        }
        Operand::StaticStr { .. } => Err(invalid_mir_diagnostics(
            "string literal used as an i32 operand",
        )),
        Operand::Copy(place) | Operand::Move(place) => {
            i32_location(place, context).map(I32Value::Location)
        }
    }
}

fn lower_u8_operand(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<U8Value, Vec<Diagnostic>> {
    match operand {
        Operand::Constant(constant) => {
            u8::try_from(constant.value)
                .map(U8Value::Const)
                .map_err(|_| {
                    invalid_mir_diagnostics("u8 constant is outside its runtime representation")
                })
        }
        Operand::StaticStr { .. } => Err(invalid_mir_diagnostics(
            "string literal used as a u8 operand",
        )),
        Operand::Copy(place) | Operand::Move(place) => {
            u8_location(place, context).map(U8Value::Location)
        }
    }
}

fn lower_usize_operand(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match operand {
        Operand::Constant(constant) => u64::try_from(constant.value)
            .map(UsizeValue::Const)
            .map_err(|_| {
                invalid_mir_diagnostics("usize constant is outside its runtime representation")
            }),
        Operand::StaticStr { .. } => Err(invalid_mir_diagnostics(
            "string literal used as a usize operand",
        )),
        Operand::Copy(place) | Operand::Move(place) => {
            usize_location(place, context).map(UsizeValue::Location)
        }
    }
}

fn lower_integer_operand(
    operand: &Operand,
    kind: crate::integer::IntegerType,
    context: &BackendContext<'_>,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    match operand {
        Operand::Constant(constant) => u64::try_from(constant.value)
            .map(|value| UsizeValue::Const(kind.canonical_word(value)))
            .map_err(|_| {
                invalid_mir_diagnostics("integer constant is outside its runtime representation")
            }),
        Operand::StaticStr { .. } => Err(invalid_mir_diagnostics(
            "string literal used as an integer operand",
        )),
        Operand::Copy(place) | Operand::Move(place) => {
            integer_location(place, kind, context).map(UsizeValue::Location)
        }
    }
}

fn lower_bool_operand(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match operand {
        Operand::Constant(constant) => match constant.value {
            0 => Ok(BoolValue::Const(false)),
            1 => Ok(BoolValue::Const(true)),
            _ => Err(invalid_mir_diagnostics(
                "bool constant is outside its runtime representation",
            )),
        },
        Operand::StaticStr { .. } => Err(invalid_mir_diagnostics(
            "string literal used as a bool operand",
        )),
        Operand::Copy(place) | Operand::Move(place) => {
            bool_location(place, context).map(BoolValue::Location)
        }
    }
}

fn lower_str_operand(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<StrValue, Vec<Diagnostic>> {
    match operand {
        Operand::StaticStr { bytes, .. } => Ok(StrValue::StaticBytes(bytes.clone())),
        Operand::Copy(place) | Operand::Move(place) => {
            str_location(place, context).map(StrValue::Location)
        }
        Operand::Constant(_) => Err(invalid_mir_diagnostics(
            "scalar constant used as a string-view operand",
        )),
    }
}

fn str_location(
    place: &Place,
    context: &BackendContext<'_>,
) -> Result<StrLocation, Vec<Diagnostic>> {
    if let Some(projection) = place.projection {
        let path = context
            .body
            .projections
            .get(projection.index())
            .ok_or_else(|| invalid_mir_diagnostics("string view projection is missing"))?;
        let crate::mir::ProjectionElement::ErrorField(field) = path.element else {
            return Err(invalid_mir_diagnostics(
                "string view projection is not an error field",
            ));
        };
        let declaration = &context.body.locals[place.local.index()];
        if declaration.representation != crate::mir::ValueRepresentation::Error
            || declaration.storage != LocalStorage::Local
        {
            return Err(invalid_mir_diagnostics(
                "error field projection is not backed by a logical error local",
            ));
        }
        let base = machine_local_index(context.body, place.local);
        return Ok(StrLocation::Local(
            base + match field {
                crate::builtin_types::BuiltinErrorField::Code => 0,
                crate::builtin_types::BuiltinErrorField::Message => 2,
            },
        ));
    }
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Return => Ok(StrLocation::Return),
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::Str { abi_index }) => {
                Ok(StrLocation::Parameter(abi_index))
            }
            _ => Err(invalid_mir_diagnostics(
                "string-view MIR parameter has no matching ABI projection",
            )),
        },
        LocalStorage::Local => Ok(StrLocation::Local(machine_local_index(
            context.body,
            place.local,
        ))),
    }
}

fn invalid_mir_diagnostics(error: impl std::fmt::Debug) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8000",
        format!("compiler produced invalid MIR: {error:?}"),
    )]
}

//! MIR-to-machine-IR lowering. This module grows only after the corresponding
//! AST-driven lowering family has been removed from its production route.

use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgument, AggregateArgumentSource, BoolComparisonOperator, BoolLocation, BoolValue,
    DirectAggregateArgument, I32ComparisonOperator, I32Location, I32Value, Instruction,
    IntegerBinaryOperator, OutcomeFailureMode, ScalarArgument, Type, U8Location, U8Value,
    UsizeLocation, UsizeValue,
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
mod parameters;
mod storage;

/// Immutable inputs shared by every control-flow structuring path.
///
/// Keeping this as one value prevents branches and loop helpers from growing
/// parallel parameter lists as MIR gains aggregate and borrow projections.
pub(super) struct BackendContext<'a> {
    body: &'a Body,
    resolved: &'a ResolveOutput,
    resolved_sources: &'a crate::resolve::ResolvedSources<'a>,
    typed_hir: &'a TypedHir,
    function_signatures: &'a super::context::FunctionSignatures,
    function_names: &'a super::context::FunctionNames,
    parameters: parameters::ParameterProjection,
    root_source: SourceId,
}

pub(super) fn try_lower_scalar_body(
    cache: &crate::mir::BodyCache,
    body: &crate::ast::Block,
    parameters: &[crate::ast::Parameter],
    return_type: &Type,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    parameter_slots: &super::context::LoweringParameterSlots,
    root_source: SourceId,
    sources: &SourceMap,
) -> Option<Result<Vec<Instruction>, Vec<Diagnostic>>> {
    let (return_scalar, return_mode) = match return_type {
        Type::I32 => (ScalarType::I32, ReturnMode::Plain),
        Type::U8 => (ScalarType::U8, ReturnMode::Plain),
        Type::Usize => (ScalarType::Usize, ReturnMode::Plain),
        Type::Integer(kind) => (ScalarType::Integer(*kind), ReturnMode::Plain),
        Type::Bool => (ScalarType::Bool, ReturnMode::Plain),
        Type::Fallible(success) => match success.as_ref() {
            Type::I32 => (ScalarType::I32, ReturnMode::Fallible),
            Type::U8 => (ScalarType::U8, ReturnMode::Fallible),
            Type::Usize => (ScalarType::Usize, ReturnMode::Fallible),
            Type::Integer(kind) => (ScalarType::Integer(*kind), ReturnMode::Fallible),
            Type::Bool => (ScalarType::Bool, ReturnMode::Fallible),
            _ => return None,
        },
        _ => return None,
    };
    let body_id = resolved.semantic_db.body_at(body.span)?;
    let parameter_projection =
        parameters::ParameterProjection::from_slots(parameters, parameter_slots)?;
    let mir_body = cache.get_or_build(body_id, || {
        crate::mir::try_build_scalar_body_with_return_mode(
            body,
            parameters,
            return_scalar,
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
                    lower_call_target(*callee, resolved, function_names, root_source)?;
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
                    } => {
                        let failure_mode =
                            outcome_failure_mode(&context, *failure, *success, &mut visited)?;
                        let destination_scalar = local_scalar(body, destination.local)?;
                        instructions.push(lower_outcome_call(
                            &context,
                            destination_scalar,
                            destination,
                            call_target,
                            arguments,
                            failure_mode,
                            &callee_name,
                        )?);
                        current = *success;
                    }
                }
            }
            Terminator::Drop {
                place,
                plan,
                target,
            } => {
                instructions.extend(drops::lower_drop(&context, *place, *plan)?);
                current = *target;
            }
            Terminator::Trap => {
                instructions.push(Instruction::Trap);
                return Ok(instructions);
            }
            Terminator::PropagateFailure => {
                instructions.push(Instruction::PropagateFailure);
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
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>> {
    let body = context.body;
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
        _ => Err(invalid_mir_diagnostics(
            "outcome call failure block has an invalid terminator",
        )),
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
                    *callee,
                    context.resolved,
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
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                    },
                ..
            } => {
                let (call_target, callee_name) = lower_call_target(
                    *callee,
                    context.resolved,
                    context.function_names,
                    context.root_source,
                )?;
                let arguments = arguments
                    .iter()
                    .map(|argument| lower_call_argument(argument, context))
                    .collect::<Result<Vec<_>, _>>()?;
                let destination_scalar = local_scalar(body, destination.local)?;
                instructions.push(lower_outcome_call(
                    context,
                    destination_scalar,
                    destination,
                    call_target,
                    arguments,
                    outcome_failure_mode(context, *failure, *success, visited)?,
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

fn aggregate_local_abi_value(
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
    scalar: ScalarType,
    destination: &Place,
    target: crate::ir::CallTarget,
    arguments: Vec<ScalarArgument>,
    failure_mode: OutcomeFailureMode,
    callee_name: &str,
) -> Result<Instruction, Vec<Diagnostic>> {
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
    callee: crate::semantic::DefId,
    resolved: &ResolveOutput,
    function_names: &super::context::FunctionNames,
    root_source: SourceId,
) -> Result<(crate::ir::CallTarget, String), Vec<Diagnostic>> {
    let name = function_names
        .name_for_definition(callee)
        .ok_or_else(|| invalid_mir_diagnostics("call target has no indexed runtime name"))?
        .clone();
    let source = resolved
        .semantic_db
        .definition_anchor(callee)
        .ok_or_else(|| invalid_mir_diagnostics("call target has no source anchor"))?
        .source;
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
    })
}

fn lower_borrow_argument_source(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<crate::ir::BorrowSource, Vec<Diagnostic>> {
    let Operand::Copy(place) = operand else {
        return Err(invalid_mir_diagnostics(
            "borrow call argument is not a copied stored place",
        ));
    };
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
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_none() => place,
        Operand::Copy(_) | Operand::Move(_) | Operand::Constant(_) => {
            return Err(invalid_mir_diagnostics(
                "aggregate call argument is not a whole stored place",
            ));
        }
    };
    let local = &context.body.locals[place.local.index()];
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
            (layout, classification)
        }
        LocalStorage::Local => {
            let value = aggregate_local_abi_value(local.ty, context)?;
            (value.layout, value.classification)
        }
        LocalStorage::Return => {
            return Err(invalid_mir_diagnostics(
                "aggregate return storage cannot be a call argument",
            ));
        }
    };
    let crate::ir::AggregateLocation::Slot(slot_index) = aggregate_location(place, context)? else {
        return Err(invalid_mir_diagnostics(
            "aggregate argument is not slot-backed",
        ));
    };
    let source = AggregateArgumentSource::Slot(slot_index);
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
        LocalStorage::Return => Ok(crate::ir::AggregateLocation::Return),
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
        if let Statement::BeginLoan { loan, .. } = statement {
            let declaration = body.loans.get(loan.index()).ok_or_else(|| {
                invalid_mir_diagnostics("borrow statement has no matching loan declaration")
            })?;
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
        match local_scalar(body, destination.local)? {
            ScalarType::I32 => {
                let destination = i32_location(destination, context)?;
                match value {
                    Rvalue::Use(operand) => {
                        if let Some((source, offset)) = aggregate_field_source(operand, context)? {
                            instructions.push(Instruction::LoadAggregateI32 {
                                destination,
                                source,
                                offset,
                            });
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
                    Rvalue::Use(operand) => {
                        if let Some((source, offset)) = aggregate_field_source(operand, context)? {
                            instructions.push(Instruction::LoadAggregateU8 {
                                destination,
                                source,
                                offset,
                            });
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
                    Rvalue::Use(operand) => {
                        if let Some((source, offset)) = aggregate_field_source(operand, context)? {
                            instructions.push(Instruction::LoadAggregateUsize {
                                destination,
                                source,
                                offset,
                            });
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
                    Rvalue::Use(operand) => {
                        if let Some((source, offset)) = aggregate_field_source(operand, context)? {
                            instructions.push(Instruction::LoadAggregateInteger {
                                kind,
                                destination,
                                source,
                                offset,
                            });
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
                    Rvalue::Use(operand) => {
                        if let Some((source, offset)) = aggregate_field_source(operand, context)? {
                            instructions.push(Instruction::LoadAggregateBool {
                                destination,
                                source,
                                offset,
                            });
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

fn lower_borrow_source(
    place: Place,
    context: &BackendContext<'_>,
) -> Result<crate::ir::BorrowSource, Vec<Diagnostic>> {
    if place.projection.is_some() {
        return Err(invalid_mir_diagnostics(
            "projected MIR loans have not been projected to machine IR",
        ));
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
            LocalStorage::Local | LocalStorage::Return => {
                return Err(invalid_mir_diagnostics(
                    "aggregate local MIR loans have not been projected to machine IR",
                ));
            }
        },
    })
}

fn aggregate_field_source(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<Option<(crate::ir::AggregateLocation, u32)>, Vec<Diagnostic>> {
    let place = match operand {
        Operand::Constant(_) => return Ok(None),
        Operand::Copy(place) | Operand::Move(place) => place,
    };
    let Some(projection) = place.projection else {
        return Ok(None);
    };
    let offset = aggregate_field_offset(context.body, place.local, projection)?;
    Ok(Some((
        aggregate_location(&Place::local(place.local), context)?,
        offset,
    )))
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
        .ok_or_else(|| invalid_mir_diagnostics("scalar MIR lowering received an aggregate local"))
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
        Operand::Copy(place) | Operand::Move(place) => {
            bool_location(place, context).map(BoolValue::Location)
        }
    }
}

fn invalid_mir_diagnostics(error: impl std::fmt::Debug) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8000",
        format!("compiler produced invalid MIR: {error:?}"),
    )]
}

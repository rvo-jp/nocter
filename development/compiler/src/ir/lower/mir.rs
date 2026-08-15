//! Projection of validated MIR into machine IR.
//!
//! Source blocks appear only in the cache/construction facade at this module's
//! boundary. The projector below consumes `Body` and never reconstructs
//! execution semantics from source expressions or statements.

use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgument, AggregateArgumentSource, AggregateIndex, AggregateLocation, AggregateRange,
    BoolComparisonOperator, BoolLocation, BoolValue, DirectAggregateArgument,
    I32ComparisonOperator, I32Location, I32Value, Instruction, IntegerBinaryOperator,
    OutcomeFailureMode, ScalarArgument, SliceLocation, SliceValue, StrLocation, StrValue, Type,
    U8Location, U8Value, UsizeLocation, UsizeValue,
};
use crate::mir::{
    Body, CallContinuation, ComparisonOperator, LocalId, LocalStorage, Operand, Place, ReturnMode,
    Rvalue, ScalarType, Statement, Terminator, UnaryOperator,
};
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::TypedHir;
use std::collections::HashSet;

mod aggregate_projection;
mod control_flow;
mod drops;
mod integer_projection;
mod loops;
mod outcomes;
mod parameters;
mod storage;
mod type_projection;

/// Immutable inputs shared by every control-flow structuring path.
///
/// Keeping this as one value prevents branches and loop helpers from growing
/// parallel parameter lists as MIR gains aggregate and borrow projections.
pub(super) struct BackendContext<'a> {
    body: &'a Body,
    return_type: &'a Type,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    function_signatures: &'a super::context::FunctionSignatures,
    function_names: &'a super::context::FunctionNames,
    error_payloads: &'a super::context::ErrorPayloads,
    parameters: parameters::ParameterProjection,
    types: type_projection::TypeProjection<'a>,
    root_source: SourceId,
}

fn success_return_instruction(body: &Body) -> Instruction {
    if body.outcome_contract.is_some() {
        Instruction::ReturnOutcomeSuccess
    } else {
        Instruction::Return
    }
}

pub(super) fn lower_body(
    cache: &crate::mir::BodyCache,
    body: &crate::ast::Block,
    parameters: &[crate::ast::Parameter],
    return_type_expr: &crate::ast::TypeExpr,
    return_type: &Type,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    substitutions: &std::collections::HashMap<String, crate::ast::TypeExpr>,
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    error_payloads: &super::context::ErrorPayloads,
    parameter_slots: &super::parameter_slots::LoweringParameterSlots,
    root_source: SourceId,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let specialized_hir =
        crate::mir::prepare_typed_hir(typed_hir, substitutions, parameters, return_type_expr, None);
    let typed_hir = &specialized_hir;
    let specialized_return_type =
        crate::ast::substitute_type_expr_parameters(return_type_expr, substitutions);
    let return_contract =
        crate::mir::callable_return_contract(&specialized_return_type, resolved, resolved_sources)
            .ok_or_else(|| {
                unsupported_mir_boundary(sources, body.span, function_name, "return type")
            })?;
    let body_id = resolved.semantic_db.body_at(body.span).ok_or_else(|| {
        unsupported_mir_boundary(sources, body.span, function_name, "source body identity")
    })?;
    let parameter_projection =
        parameters::ParameterProjection::from_slots(parameters, parameter_slots).ok_or_else(
            || unsupported_mir_boundary(sources, body.span, function_name, "parameter projection"),
        )?;
    let mir_body = cache.get_or_build_specialized(body.span.source, body_id, substitutions, || {
        crate::mir::build_body(
            body,
            parameters,
            return_contract,
            crate::mir::BuildInputs {
                semantic_db: &resolved.semantic_db,
                resolved,
                resolved_sources,
                typed_hir,
                declared_return_ty: typed_hir.type_id(&specialized_return_type),
            },
        )
    });
    lower_cached_body(
        mir_body,
        return_type,
        resolved,
        resolved_sources,
        typed_hir,
        function_name,
        function_signatures,
        function_names,
        error_payloads,
        parameter_projection,
        root_source,
        sources,
        body.span,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_literal_body(
    cache: &crate::mir::BodyCache,
    body: &crate::ast::Block,
    parameters: &[crate::ast::Parameter],
    return_type_expr: &crate::ast::TypeExpr,
    return_type: &Type,
    literal_pack: crate::mir::LiteralPackInput,
    literal_instance: crate::mir::CallInstanceKey,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    substitutions: &std::collections::HashMap<String, crate::ast::TypeExpr>,
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    error_payloads: &super::context::ErrorPayloads,
    parameter_slots: &super::parameter_slots::LoweringParameterSlots,
    root_source: SourceId,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let specialized_hir = crate::mir::prepare_typed_hir(
        typed_hir,
        substitutions,
        parameters,
        return_type_expr,
        Some(&literal_pack),
    );
    let typed_hir = &specialized_hir;
    let specialized_return_type =
        crate::ast::substitute_type_expr_parameters(return_type_expr, substitutions);
    let return_contract =
        crate::mir::callable_return_contract(&specialized_return_type, resolved, resolved_sources)
            .ok_or_else(|| {
                unsupported_mir_boundary(sources, body.span, function_name, "return type")
            })?;
    let body_id = resolved.semantic_db.body_at(body.span).ok_or_else(|| {
        unsupported_mir_boundary(sources, body.span, function_name, "source body identity")
    })?;
    let parameter_projection =
        parameters::ParameterProjection::from_slots(parameters, parameter_slots).ok_or_else(
            || unsupported_mir_boundary(sources, body.span, function_name, "parameter projection"),
        )?;
    let mir_body = cache.get_or_build_literal_specialized(
        body.span.source,
        body_id,
        substitutions,
        literal_instance,
        || {
            crate::mir::build_literal_body(
                body,
                parameters,
                return_contract,
                crate::mir::BuildInputs {
                    semantic_db: &resolved.semantic_db,
                    resolved,
                    resolved_sources,
                    typed_hir,
                    declared_return_ty: typed_hir.type_id(&specialized_return_type),
                },
                literal_pack,
            )
        },
    );
    lower_cached_body(
        mir_body,
        return_type,
        resolved,
        resolved_sources,
        typed_hir,
        function_name,
        function_signatures,
        function_names,
        error_payloads,
        parameter_projection,
        root_source,
        sources,
        body.span,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_closure_body(
    cache: &crate::mir::BodyCache,
    expression: &crate::ast::ClosureExpr,
    closure_ty: &crate::ast::ClosureTypeExpr,
    receiver_mode: crate::ast::MethodReceiverMode,
    return_type: &Type,
    parameters: &[crate::ast::Parameter],
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    error_payloads: &super::context::ErrorPayloads,
    parameter_slots: &super::parameter_slots::LoweringParameterSlots,
    root_source: SourceId,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let return_contract = crate::mir::callable_return_contract(
        closure_ty.return_type.as_ref(),
        resolved,
        resolved_sources,
    )
    .ok_or_else(|| {
        unsupported_mir_boundary(sources, expression.body.span, function_name, "return type")
    })?;
    let body_id = resolved
        .semantic_db
        .body_at(expression.body.span)
        .ok_or_else(|| {
            unsupported_mir_boundary(
                sources,
                expression.body.span,
                function_name,
                "source body identity",
            )
        })?;
    let parameter_projection =
        parameters::ParameterProjection::from_slots(parameters, parameter_slots).ok_or_else(
            || {
                unsupported_mir_boundary(
                    sources,
                    expression.body.span,
                    function_name,
                    "parameter projection",
                )
            },
        )?;
    let substitutions = std::collections::HashMap::new();
    let mir_body = cache.get_or_build_specialized(
        expression.body.span.source,
        body_id,
        &substitutions,
        || {
            crate::mir::build_closure_body(
                expression,
                closure_ty,
                receiver_mode,
                return_contract,
                crate::mir::BuildInputs {
                    semantic_db: &resolved.semantic_db,
                    resolved,
                    resolved_sources,
                    typed_hir,
                    declared_return_ty: typed_hir.type_id(closure_ty.return_type.as_ref()),
                },
            )
        },
    );
    lower_cached_body(
        mir_body,
        return_type,
        resolved,
        resolved_sources,
        typed_hir,
        function_name,
        function_signatures,
        function_names,
        error_payloads,
        parameter_projection,
        root_source,
        sources,
        expression.body.span,
    )
}

fn unsupported_mir_boundary(
    sources: &SourceMap,
    span: ByteSpan,
    function_name: &str,
    boundary: &str,
) -> Vec<Diagnostic> {
    attach_primary_span(
        vec![Diagnostic::error(
            "E8000",
            format!("compiler could not construct MIR for `{function_name}`: missing {boundary}"),
        )],
        sources,
        span,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_cached_body(
    mir_body: Result<Body, crate::mir::BuildError>,
    return_type: &Type,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    error_payloads: &super::context::ErrorPayloads,
    parameter_projection: parameters::ParameterProjection,
    root_source: SourceId,
    sources: &SourceMap,
    span: ByteSpan,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match mir_body {
        Ok(mir_body) => lower_scalar_body(
            &mir_body,
            return_type,
            resolved,
            resolved_sources,
            typed_hir,
            function_name,
            function_signatures,
            function_names,
            error_payloads,
            parameter_projection,
            root_source,
        )
        .map_err(|diagnostics| attach_primary_span(diagnostics, sources, span)),
        Err(error) => Err(attach_primary_span(
            vec![Diagnostic::error(
                "E8000",
                format!("compiler could not construct MIR for `{function_name}`: {error:?}"),
            )],
            sources,
            span,
        )),
    }
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
    error_payloads: &super::context::ErrorPayloads,
    parameter_projection: parameters::ParameterProjection,
    root_source: SourceId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    crate::mir::validate(body).map_err(invalid_mir_diagnostics)?;
    let context = BackendContext {
        body,
        return_type,
        resolved,
        typed_hir,
        function_signatures,
        function_names,
        error_payloads,
        parameters: parameter_projection,
        types: type_projection::TypeProjection::new(typed_hir, resolved, resolved_sources),
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
                loop_.body,
                loop_.exit,
                loop_.continue_target,
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
            let loop_never_continues = condition == BoolValue::Const(true)
                && body_instructions.last().is_some_and(|instruction| {
                    matches!(
                        instruction,
                        Instruction::Return
                            | Instruction::ReturnOutcomeSuccess
                            | Instruction::ReturnFallibleFailure { .. }
                            | Instruction::TailCall { .. }
                            | Instruction::Trap
                    )
                });
            instructions.push(Instruction::While {
                condition_instructions,
                condition,
                body_instructions,
            });
            if loop_never_continues {
                return Ok(instructions);
            }
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
                join_target,
            } => {
                let then_instructions = match join_target {
                    Some(join) => {
                        lower_branch_to_join(&context, *then_target, *join, &mut visited)?
                    }
                    None => lower_branch_to_terminal(&context, *then_target, &mut visited)?,
                };
                let else_instructions = match join_target {
                    Some(join) => {
                        lower_branch_to_join(&context, *else_target, *join, &mut visited)?
                    }
                    None => lower_branch_to_terminal(&context, *else_target, &mut visited)?,
                };
                instructions.push(Instruction::If {
                    condition: lower_bool_operand(condition, &context)?,
                    then_instructions,
                    else_instructions,
                });
                let Some(join) = join_target else {
                    return Ok(instructions);
                };
                current = *join;
            }
            Terminator::Call {
                callee,
                arguments,
                continuation,
                ..
            } => {
                if let crate::mir::CallableIdentity::Intrinsic(intrinsic) = &callee.callable
                    && crate::mir::outcome_intrinsic_is_supported(*intrinsic)
                {
                    match continuation {
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
                            reserve_aggregate_destination(
                                &context,
                                destination,
                                &mut instructions,
                            )?;
                            instructions.extend(lower_outcome_intrinsic_call(
                                &context,
                                *intrinsic,
                                Some(destination),
                                arguments,
                                failure_mode,
                            )?);
                            current = *success;
                            continue;
                        }
                        CallContinuation::OutcomeEffect {
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
                            instructions.extend(lower_outcome_intrinsic_call(
                                &context,
                                *intrinsic,
                                None,
                                arguments,
                                failure_mode,
                            )?);
                            current = *success;
                            continue;
                        }
                        _ => {
                            return Err(invalid_mir_diagnostics(
                                "outcome intrinsic has a non-outcome continuation",
                            ));
                        }
                    }
                }
                if matches!(continuation, CallContinuation::Never)
                    && let Some(lowered) = lower_never_intrinsic_call(&context, callee, arguments)?
                {
                    instructions.extend(lowered);
                    return Ok(instructions);
                }
                let (call_target, callee_name) =
                    lower_call_target(callee, resolved, typed_hir, function_names, root_source)?;
                let arguments = lower_call_arguments(arguments, &context)?;
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
                        let returns_directly = target_block.statements.is_empty()
                            && (destination.local == body.return_local
                                && target_block.terminator == Terminator::Return
                                || matches!(
                                    &target_block.terminator,
                                    Terminator::ReturnValue {
                                        source: Operand::Copy(place) | Operand::Move(place),
                                    } if place == destination
                                ));
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
                        instructions.extend(lower_returning_call_sequence(
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
                        let failure_mode = outcome_failure_mode(
                            &context,
                            *failure,
                            *success,
                            *failure_payload,
                            &mut visited,
                        )?;
                        validate_outcome_effect_call_return_type(
                            &call_target,
                            &callee_name,
                            function_signatures,
                        )?;
                        instructions.push(Instruction::CallOutcomeVoid {
                            target: call_target,
                            arguments,
                            failure_mode,
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
                        source,
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
            Terminator::ReturnOutcomeSuccess { source } => {
                instructions.extend(lower_outcome_success_return(&context, source)?);
                instructions.push(Instruction::ReturnOutcomeSuccess);
                return Ok(instructions);
            }
            Terminator::ReturnOptionalNone => {
                instructions.push(Instruction::ReturnOptionalNone);
                return Ok(instructions);
            }
            Terminator::ReturnValue { source } => {
                instructions.extend(lower_value_return(&context, source)?);
                instructions.push(success_return_instruction(body));
                return Ok(instructions);
            }
            Terminator::Return => {
                instructions.push(success_return_instruction(body));
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
    if (body.return_mode == ReturnMode::Fallible || body.outcome_contract.is_some())
        && let Some(path) = no_op_propagation_path(context, failure)?
    {
        visited.extend(path);
        return Ok(OutcomeFailureMode::Propagate);
    }
    if body.return_mode == ReturnMode::Fallible && !control_flow::can_reach(body, failure, success)
    {
        let mut propagation_visited = visited.clone();
        let mut instructions =
            lower_branch_to_join(context, failure, success, &mut propagation_visited)?;
        if matches!(instructions.last(), Some(Instruction::PropagateFailure)) {
            instructions.pop();
            *visited = propagation_visited;
            let error_base = storage::machine_local_count(body) + 1;
            return Ok(OutcomeFailureMode::PropagateWithCleanup {
                code: StrLocation::Local(error_base),
                message: StrLocation::Local(error_base + 2),
                instructions,
            });
        }
    }
    let failure_block = &body.blocks[failure.index()];
    match &failure_block.terminator {
        Terminator::Trap if failure_block.statements.is_empty() => {
            visited.insert(failure);
            Ok(OutcomeFailureMode::Trap)
        }
        Terminator::PropagateFailure
            if body.return_mode == ReturnMode::Fallible || body.outcome_contract.is_some() =>
        {
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
            super::call_abi::describe_type(success),
            super::call_abi::describe_type(&expected),
        )));
    }
    super::call_abi::validate_success_return_passing(
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
        super::call_abi::describe_type(return_type),
    )))
}

fn lower_branch_to_join(
    context: &BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    join: crate::mir::BasicBlockId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_branch(context, start, Some(join), visited)
}

fn lower_branch_to_terminal(
    context: &BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_branch(context, start, None, visited)
}

fn lower_branch(
    context: &BackendContext<'_>,
    start: crate::mir::BasicBlockId,
    join: Option<crate::mir::BasicBlockId>,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let body = context.body;
    let mut instructions = Vec::new();
    let mut current = start;
    loop {
        if Some(current) == join {
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
                continuation: CallContinuation::Never,
                ..
            } => {
                if let Some(lowered) = lower_never_intrinsic_call(context, callee, arguments)? {
                    instructions.extend(lowered);
                    return Ok(instructions);
                }
                let (call_target, callee_name) = lower_call_target(
                    callee,
                    context.resolved,
                    context.typed_hir,
                    context.function_names,
                    context.root_source,
                )?;
                let arguments = lower_call_arguments(arguments, context)?;
                validate_never_call_return_type(
                    &call_target,
                    &callee_name,
                    context.function_signatures,
                )?;
                let fits_tail_call_abi = arguments
                    .iter()
                    .map(ScalarArgument::abi_word_count)
                    .sum::<usize>()
                    <= crate::abi::ARGUMENT_REGISTER_COUNT
                    && !arguments
                        .iter()
                        .any(ScalarArgument::requires_current_frame_for_tail_call);
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
                let arguments = lower_call_arguments(arguments, context)?;
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
                let arguments = lower_call_arguments(arguments, context)?;
                reserve_aggregate_destination(context, destination, &mut instructions)?;
                instructions.extend(lower_returning_call_sequence(
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
                let failure_mode =
                    outcome_failure_mode(context, *failure, *success, *failure_payload, visited)?;
                if let crate::mir::CallableIdentity::Intrinsic(intrinsic) = &callee.callable {
                    instructions.extend(lower_outcome_intrinsic_call(
                        context,
                        *intrinsic,
                        None,
                        arguments,
                        failure_mode,
                    )?);
                    current = *success;
                    continue;
                }
                let (call_target, callee_name) = lower_call_target(
                    callee,
                    context.resolved,
                    context.typed_hir,
                    context.function_names,
                    context.root_source,
                )?;
                let arguments = lower_call_arguments(arguments, context)?;
                validate_outcome_effect_call_return_type(
                    &call_target,
                    &callee_name,
                    context.function_signatures,
                )?;
                instructions.push(Instruction::CallOutcomeVoid {
                    target: call_target,
                    arguments,
                    failure_mode,
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
                        source,
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
                let failure_mode =
                    outcome_failure_mode(context, *failure, *success, *failure_payload, visited)?;
                reserve_aggregate_destination(context, destination, &mut instructions)?;
                if let crate::mir::CallableIdentity::Intrinsic(intrinsic) = &callee.callable {
                    instructions.extend(lower_outcome_intrinsic_call(
                        context,
                        *intrinsic,
                        Some(destination),
                        arguments,
                        failure_mode,
                    )?);
                    current = *success;
                    continue;
                }
                let (call_target, callee_name) = lower_call_target(
                    callee,
                    context.resolved,
                    context.typed_hir,
                    context.function_names,
                    context.root_source,
                )?;
                let arguments = lower_call_arguments(arguments, context)?;
                instructions.push(lower_outcome_call(
                    context,
                    destination,
                    call_target,
                    arguments,
                    failure_mode,
                    &callee_name,
                )?);
                current = *success;
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
                join_target,
            } => {
                let branch_join = join_target.or(join);
                instructions.push(Instruction::If {
                    condition: lower_bool_operand(condition, context)?,
                    then_instructions: match branch_join {
                        Some(join) => lower_branch_to_join(context, *then_target, join, visited)?,
                        None => lower_branch_to_terminal(context, *then_target, visited)?,
                    },
                    else_instructions: match branch_join {
                        Some(join) => lower_branch_to_join(context, *else_target, join, visited)?,
                        None => lower_branch_to_terminal(context, *else_target, visited)?,
                    },
                });
                let Some(branch_join) = branch_join else {
                    return Ok(instructions);
                };
                current = branch_join;
            }
            Terminator::Return => {
                instructions.push(success_return_instruction(body));
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
            Terminator::PropagateFailure
                if body.outcome_contract.as_ref().is_some_and(|contract| {
                    contract
                        .layers
                        .contains(&crate::outcomes::OutcomeLayer::Optional)
                }) =>
            {
                instructions.push(Instruction::ReturnOptionalNone);
                return Ok(instructions);
            }
            Terminator::ReturnOutcome { source } => {
                instructions.push(outcomes::lower_return(context, source)?);
                return Ok(instructions);
            }
            Terminator::ReturnOutcomeSuccess { source } => {
                instructions.extend(lower_outcome_success_return(context, source)?);
                instructions.push(Instruction::ReturnOutcomeSuccess);
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
            Terminator::ReturnOptionalNone => {
                instructions.push(Instruction::ReturnOptionalNone);
                return Ok(instructions);
            }
            Terminator::ReturnValue { source } => {
                instructions.extend(lower_value_return(context, source)?);
                instructions.push(success_return_instruction(body));
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

pub(super) fn lower_outcome_success_return(
    context: &BackendContext<'_>,
    source: &Operand,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let contract =
        context.body.outcome_contract.as_ref().ok_or_else(|| {
            invalid_mir_diagnostics("outcome success return has no body contract")
        })?;
    Ok(vec![match contract.payload_representation {
        crate::mir::ValueRepresentation::Scalar(ScalarType::I32) => Instruction::SetI32 {
            destination: I32Location::Return,
            value: lower_i32_operand(source, context)?,
        },
        crate::mir::ValueRepresentation::Scalar(ScalarType::U8) => Instruction::SetU8 {
            destination: U8Location::Return,
            value: lower_u8_operand(source, context)?,
        },
        crate::mir::ValueRepresentation::Scalar(ScalarType::Usize) => Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: lower_usize_operand(source, context)?,
        },
        crate::mir::ValueRepresentation::Scalar(ScalarType::Integer(kind)) => {
            Instruction::SetUsize {
                destination: UsizeLocation::Return,
                value: lower_integer_operand(source, kind, context)?,
            }
        }
        crate::mir::ValueRepresentation::Scalar(ScalarType::Bool) => Instruction::SetBool {
            destination: BoolLocation::Return,
            value: lower_bool_operand(source, context)?,
        },
        crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) => Instruction::SetStr {
            destination: StrLocation::Return,
            value: lower_str_operand(source, context)?,
        },
        crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice) => {
            Instruction::SetSlice {
                destination: SliceLocation::Return,
                value: lower_slice_operand(source, context)?,
            }
        }
        crate::mir::ValueRepresentation::Borrow => {
            let (Operand::Copy(source) | Operand::Move(source)) = source else {
                return Err(invalid_mir_diagnostics(
                    "borrow outcome success return is not backed by a place",
                ));
            };
            Instruction::SetUsizeFromBorrow {
                destination: UsizeLocation::Return,
                source: lower_borrow_source(*source, context)?,
            }
        }
        crate::mir::ValueRepresentation::Aggregate => {
            let (Operand::Copy(source) | Operand::Move(source)) = source else {
                return Err(invalid_mir_diagnostics(
                    "aggregate outcome success return is not backed by a place",
                ));
            };
            let (destination, layout) = match context.return_type.success_type() {
                Type::Aggregate { layout } => (AggregateLocation::Return, *layout),
                Type::DirectAggregate { layout, .. } => (AggregateLocation::DirectReturn, *layout),
                _ => {
                    return Err(invalid_mir_diagnostics(
                        "aggregate outcome success return has a non-aggregate payload",
                    ));
                }
            };
            Instruction::CopyAggregate {
                destination,
                source: aggregate_location(source, context)?,
                layout,
            }
        }
        crate::mir::ValueRepresentation::Unit | crate::mir::ValueRepresentation::Error => {
            return Err(invalid_mir_diagnostics(
                "outcome success return payload representation is not projected",
            ));
        }
    }])
}

fn lower_value_return(
    context: &BackendContext<'_>,
    source: &Operand,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_statements(
        context,
        &[Statement::Assign {
            destination: Place::local(context.body.return_local),
            value: Rvalue::Use(source.clone()),
            origin: crate::mir::Origin::Desugared(context.body.source_span),
        }],
    )
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
            let outcome = context
                .types
                .outcome(context.body.locals[destination.local.index()].ty)?;
            return Ok(Instruction::CallStoredOutcome {
                destination: aggregate_location(destination, context)?,
                target,
                arguments,
                storage: outcome.storage,
                payload_type: outcome.payload_type,
            });
        }
        let layout = match return_type {
            Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
            _ => {
                return Err(invalid_mir_diagnostics(format!(
                    "aggregate MIR destination received `{}` from `{callee_name}`",
                    super::call_abi::describe_type(return_type)
                )));
            }
        };
        super::call_abi::validate_success_return_passing(
            function_signatures.success_return_passing(&target),
            callee_name,
            return_type,
        )?;
        return Ok(super::call_abi::aggregate_call_instruction(
            return_type,
            aggregate_location(destination, context)?,
            target,
            arguments,
            layout,
        ));
    }
    if representation == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) {
        super::call_abi::validate_success_return_passing(
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
    if representation == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice) {
        let return_type = context
            .function_signatures
            .return_type(&target)
            .ok_or_else(|| invalid_mir_diagnostics("slice call has no indexed return type"))?;
        if !matches!(return_type, Type::Slice { .. }) {
            return Err(invalid_mir_diagnostics(
                "slice MIR call target does not return a slice",
            ));
        }
        return Ok(Instruction::CallSlice {
            destination: slice_location(destination, context)?,
            target,
            arguments,
        });
    }
    if representation == crate::mir::ValueRepresentation::Borrow {
        let return_type = context
            .function_signatures
            .return_type(&target)
            .ok_or_else(|| invalid_mir_diagnostics("borrow call has no indexed return type"))?;
        if !matches!(return_type, Type::Borrow { .. }) {
            return Err(invalid_mir_diagnostics(
                "borrow MIR call target does not return a borrow",
            ));
        }
        return Ok(Instruction::CallBorrow {
            destination: usize_location(destination, context)?,
            target,
            arguments,
        });
    }
    let scalar = local_scalar(context.body, destination.local)?;
    super::call_abi::validate_success_return_passing(
        function_signatures.success_return_passing(&target),
        callee_name,
        &scalar_ir_type(scalar),
    )?;
    call_instruction(context, scalar, destination, target, arguments)
}

fn lower_returning_call_sequence(
    context: &BackendContext<'_>,
    destination: &Place,
    target: crate::ir::CallTarget,
    arguments: Vec<ScalarArgument>,
    callee_name: &str,
    function_signatures: &super::context::FunctionSignatures,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if context.body.locals[destination.local.index()].representation
        == crate::mir::ValueRepresentation::Error
    {
        if !arguments.is_empty() {
            return Err(invalid_mir_diagnostics(format!(
                "static error helper `{callee_name}` unexpectedly has runtime arguments"
            )));
        }
        let payload = context
            .error_payloads
            .get(&target)
            .cloned()
            .ok_or_else(|| {
                invalid_mir_diagnostics(format!(
                    "logical error call `{callee_name}` has no indexed static payload"
                ))
            })?;
        let (code, message) = error_place_locations(*destination, context)?;
        return Ok(payload.into_store_instructions(code, message));
    }
    Ok(vec![lower_returning_call(
        context,
        destination,
        target,
        arguments,
        callee_name,
        function_signatures,
    )?])
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
    let crate::ir::AggregateLocation::Slot(slot_index) =
        aggregate_location(&Place::local(destination.local), context)?
    else {
        return Err(invalid_mir_diagnostics(
            "aggregate local destination is not slot-backed",
        ));
    };
    if instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::ReserveAggregateSlot { slot_index: reserved, .. }
            if *reserved == slot_index)
    }) {
        return Ok(());
    }
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
    context.types.abi_value(ty)
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
        super::call_abi::validate_success_return_passing(
            context.function_signatures.success_return_passing(&target),
            callee_name,
            success,
        )?;
        return Ok(super::call_abi::fallible_aggregate_call_instruction(
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
    if representation == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice) {
        return Ok(Instruction::CallOutcomeSlice {
            destination: slice_location(destination, context)?,
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
    if representation == crate::mir::ValueRepresentation::Error {
        return Err(invalid_mir_diagnostics(format!(
            "outcome call to `{callee_name}` targets logical error local {destination:?}"
        )));
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

fn lower_never_intrinsic_call(
    context: &BackendContext<'_>,
    callee: &crate::mir::CallInstance,
    arguments: &[crate::mir::CallArgument],
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let crate::mir::CallableIdentity::Intrinsic(intrinsic) = &callee.callable else {
        return Ok(None);
    };
    let arguments = lower_call_arguments(arguments, context)?;
    let instruction = match (*intrinsic, arguments.as_slice()) {
        (crate::intrinsics::IntrinsicId::AllocationAbortRaw, []) => Instruction::ProcessExit {
            code: I32Value::Const(70),
        },
        (crate::intrinsics::IntrinsicId::ExitRaw, [ScalarArgument::I32(code)]) => {
            Instruction::ProcessExit { code: code.clone() }
        }
        (
            crate::intrinsics::IntrinsicId::Trap | crate::intrinsics::IntrinsicId::Unreachable,
            [],
        ) => Instruction::Trap,
        _ => {
            return Err(invalid_mir_diagnostics(format!(
                "never intrinsic `{}` has an invalid checked MIR call shape",
                intrinsic.source_name()
            )));
        }
    };
    Ok(Some(vec![instruction]))
}

pub(super) fn lower_outcome_intrinsic_call(
    context: &BackendContext<'_>,
    intrinsic: crate::intrinsics::IntrinsicId,
    destination: Option<&Place>,
    arguments: &[crate::mir::CallArgument],
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let arguments = lower_call_arguments(arguments, context)?;
    let invalid_shape = || {
        invalid_mir_diagnostics(format!(
            "intrinsic `{}` has an invalid checked MIR call shape",
            intrinsic.source_name()
        ))
    };
    let instruction = match (intrinsic, destination, arguments.as_slice()) {
        (
            crate::intrinsics::IntrinsicId::OpenReadRaw
            | crate::intrinsics::IntrinsicId::CreateRaw
            | crate::intrinsics::IntrinsicId::AppendRaw,
            Some(destination),
            [ScalarArgument::Usize(path)],
        ) => {
            let (flags, mode) = match intrinsic {
                crate::intrinsics::IntrinsicId::CreateRaw => (1 + 512 + 1024, 438),
                crate::intrinsics::IntrinsicId::AppendRaw => (1 + 8 + 512, 438),
                _ => (0, 0),
            };
            Instruction::OpenRead {
                destination: i32_location(destination, context)?,
                path: path.clone(),
                flags: UsizeValue::Const(flags),
                mode: UsizeValue::Const(mode),
                failure_mode,
            }
        }
        (
            crate::intrinsics::IntrinsicId::ReadBytesRaw,
            Some(destination),
            [ScalarArgument::I32(fd), ScalarArgument::Slice(buffer)],
        ) => Instruction::ReadSlice {
            destination: usize_location(destination, context)?,
            fd: fd.clone(),
            buffer: buffer.clone(),
            failure_mode,
        },
        (
            crate::intrinsics::IntrinsicId::WriteTextRaw,
            None,
            [ScalarArgument::I32(fd), ScalarArgument::Str(text)],
        ) => {
            return Ok(vec![
                Instruction::WriteStr {
                    fd: fd.clone(),
                    text: text.clone(),
                },
                outcome_effect_check(failure_mode),
            ]);
        }
        (
            crate::intrinsics::IntrinsicId::WriteBytesRaw,
            None,
            [ScalarArgument::I32(fd), ScalarArgument::Slice(bytes)],
        ) => {
            return Ok(vec![
                Instruction::WriteSlice {
                    fd: fd.clone(),
                    bytes: bytes.clone(),
                },
                outcome_effect_check(failure_mode),
            ]);
        }
        _ => return Err(invalid_shape()),
    };
    Ok(vec![instruction])
}

fn outcome_effect_check(failure_mode: OutcomeFailureMode) -> Instruction {
    match failure_mode {
        OutcomeFailureMode::Propagate => Instruction::PropagateFailure,
        OutcomeFailureMode::Trap => Instruction::TrapOnFailure,
        OutcomeFailureMode::PropagateWithCleanup { .. }
        | OutcomeFailureMode::Handle { .. }
        | OutcomeFailureMode::Recover { .. }
        | OutcomeFailureMode::Catch { .. } => Instruction::CheckFailure { failure_mode },
    }
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
        super::call_abi::describe_type(callee_return_type),
    )))
}

fn lower_call_target(
    callee: &crate::mir::CallInstance,
    _resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    function_names: &super::context::FunctionNames,
    _root_source: SourceId,
) -> Result<(crate::ir::CallTarget, String), Vec<Diagnostic>> {
    let target = function_names
        .target_for_instance(callee, typed_hir)
        .ok_or_else(|| {
            invalid_mir_diagnostics(format!(
                "call target has no indexed runtime name: {callee:?}"
            ))
        })?
        .clone();
    let name = super::call_target_name(&target).to_string();
    Ok((target, name))
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
            super::call_abi::describe_type(return_type),
            super::call_abi::describe_type(callee_return_type),
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
        super::call_abi::describe_type(callee_return_type),
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
            ScalarArgument::Integer(
                kind,
                lower_integer_operand(&argument.operand, kind, context)?,
            )
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
        crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice) => {
            ScalarArgument::Slice(lower_slice_operand(&argument.operand, context)?)
        }
        crate::mir::ValueRepresentation::Unit | crate::mir::ValueRepresentation::Error => {
            return Err(invalid_mir_diagnostics(
                "logical error values cannot be passed as scalar call arguments",
            ));
        }
    })
}

fn lower_call_arguments(
    arguments: &[crate::mir::CallArgument],
    context: &BackendContext<'_>,
) -> Result<Vec<ScalarArgument>, Vec<Diagnostic>> {
    let mut lowered = Vec::new();
    for argument in arguments {
        if argument.representation == crate::mir::ValueRepresentation::Error {
            let (Operand::Copy(place) | Operand::Move(place)) = &argument.operand else {
                return Err(invalid_mir_diagnostics(
                    "logical error call argument is not a stored place",
                ));
            };
            let declaration = context
                .body
                .locals
                .get(place.local.index())
                .ok_or_else(|| {
                    invalid_mir_diagnostics("logical error call argument local is missing")
                })?;
            if place.projection.is_some()
                || declaration.representation != crate::mir::ValueRepresentation::Error
            {
                return Err(invalid_mir_diagnostics(
                    "logical error call argument has an invalid place",
                ));
            }
            let (code, message) = match declaration.storage {
                LocalStorage::Local => {
                    let base = machine_local_index(context.body, place.local);
                    (StrLocation::Local(base), StrLocation::Local(base + 2))
                }
                LocalStorage::Parameter { ordinal } => {
                    let Some(parameters::ParameterStorage::Error { abi_index }) =
                        context.parameters.get(ordinal)
                    else {
                        return Err(invalid_mir_diagnostics(
                            "logical error parameter has no matching ABI projection",
                        ));
                    };
                    (
                        StrLocation::Parameter(abi_index),
                        StrLocation::Parameter(abi_index + 2),
                    )
                }
                LocalStorage::Return => {
                    return Err(invalid_mir_diagnostics(
                        "logical error return storage cannot be a call argument",
                    ));
                }
            };
            lowered.push(ScalarArgument::Str(StrValue::Location(code)));
            lowered.push(ScalarArgument::Str(StrValue::Location(message)));
        } else {
            lowered.push(lower_call_argument(argument, context)?);
        }
    }
    Ok(lowered)
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
    let projection = place
        .projection
        .map(|projection| aggregate_borrow_projection(context.body, place.local, projection))
        .transpose()?;
    let access_bytes = u32::try_from(layout.size).map_err(|_| {
        invalid_mir_diagnostics("aggregate argument layout exceeds the addressable range")
    })?;
    let source = match projection {
        None => AggregateArgumentSource::Slot(slot_index),
        Some(AggregateBorrowProjection::Field { offset }) => {
            AggregateArgumentSource::SlotField { slot_index, offset }
        }
        Some(AggregateBorrowProjection::Index {
            base_offset,
            index,
            length,
            stride,
        }) => AggregateArgumentSource::SlotIndex {
            slot_index,
            base_offset,
            index: lower_direct_usize_index(&index, context)?,
            length,
            stride,
            access_bytes,
        },
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
    let local = &context.body.locals[place.local.index()];
    if local.representation == crate::mir::ValueRepresentation::Borrow {
        return Ok(crate::ir::AggregateLocation::Borrow(match local.storage {
            LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
                Some(parameters::ParameterStorage::Borrow { abi_index }) => {
                    UsizeLocation::Parameter(abi_index)
                }
                _ => {
                    return Err(invalid_mir_diagnostics(
                        "borrow MIR parameter has no matching ABI projection",
                    ));
                }
            },
            LocalStorage::Local => {
                UsizeLocation::Local(machine_local_index(context.body, place.local))
            }
            LocalStorage::Return => {
                return Err(invalid_mir_diagnostics(
                    "return borrow storage cannot address an aggregate",
                ));
            }
        }));
    }
    match local.storage {
        LocalStorage::Return => match context.return_type.success_type() {
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
        if let Statement::EnterAllocationContext { override_, .. } = statement {
            instructions.extend(lower_allocation_override_enter(*override_, context)?);
            continue;
        }
        if let Statement::ExitAllocationContext { override_ } = statement {
            instructions.push(lower_allocation_override_exit(*override_, context)?);
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
                destination: usize_location(&Place::local(declaration.destination), context)?,
                source: lower_borrow_source(declaration.source, context)?,
            });
            continue;
        }
        if matches!(statement, Statement::EndLoan { .. }) {
            continue;
        }
        if let Statement::Intrinsic {
            intrinsic,
            arguments,
            type_arguments,
            ..
        } = statement
        {
            instructions.extend(lower_intrinsic_effect(
                *intrinsic,
                arguments,
                type_arguments,
                context,
            )?);
            continue;
        }
        if let Statement::DropAtPointer {
            pointer,
            offset,
            ty,
            plan,
            ..
        } = statement
        {
            instructions.extend(drops::lower_pointer_drop(
                context,
                lower_usize_operand(pointer, context)?,
                lower_usize_operand(offset, context)?,
                *ty,
                *plan,
            )?);
            continue;
        }
        let Statement::Assign {
            destination, value, ..
        } = statement
        else {
            unreachable!("all MIR statement kinds handled above");
        };
        if destination.projection.is_none()
            && (storage::is_inlined_view_cast(body, destination.local)
                || storage::is_inlined_identity_intrinsic(body, destination.local))
        {
            continue;
        }
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
                let (offset, leaf_abi) = enum_variant_leaf_projection(
                    enum_,
                    abi_variant,
                    signature.payload.len(),
                    &leaf.path,
                )?;
                match leaf.representation {
                    crate::mir::ValueRepresentation::Scalar(scalar) => {
                        if !aggregate_projection::abi_matches_scalar(leaf_abi, scalar) {
                            return Err(invalid_mir_diagnostics(
                                "variant MIR leaf scalar does not match its ABI projection",
                            ));
                        }
                        instructions.push(aggregate_projection::store_scalar(
                            &AggregateRange {
                                location,
                                offset,
                                index: None,
                            },
                            0,
                            scalar,
                            &leaf.operand,
                            context,
                        )?);
                    }
                    crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) => {
                        if !matches!(leaf_abi, crate::abi::AbiType::StrView) {
                            return Err(invalid_mir_diagnostics(
                                "variant MIR string leaf does not match its ABI projection",
                            ));
                        }
                        let value = lower_str_operand(&leaf.operand, context)?;
                        let StrValue::Location(source) = value else {
                            return Err(invalid_mir_diagnostics(
                                "variant MIR string leaf must be materialized",
                            ));
                        };
                        instructions.push(Instruction::StoreAggregateUsize {
                            destination: location,
                            offset,
                            value: UsizeValue::StrPointer(source),
                        });
                        instructions.push(Instruction::StoreAggregateUsize {
                            destination: location,
                            offset: offset + crate::abi::ABI_WORD_SIZE as u32,
                            value: UsizeValue::StrLen(source),
                        });
                    }
                    crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice) => {
                        if !matches!(leaf_abi, crate::abi::AbiType::SliceView) {
                            return Err(invalid_mir_diagnostics(
                                "variant MIR slice leaf does not match its ABI projection",
                            ));
                        }
                        let value = lower_slice_operand(&leaf.operand, context)?;
                        let SliceValue::Location(source) = value else {
                            return Err(invalid_mir_diagnostics(
                                "variant MIR slice leaf must be materialized",
                            ));
                        };
                        instructions.push(Instruction::StoreAggregateUsize {
                            destination: location,
                            offset,
                            value: UsizeValue::SlicePointer(source),
                        });
                        instructions.push(Instruction::StoreAggregateUsize {
                            destination: location,
                            offset: offset + crate::abi::ABI_WORD_SIZE as u32,
                            value: UsizeValue::SliceLen(source),
                        });
                    }
                    crate::mir::ValueRepresentation::Borrow => {
                        if !matches!(leaf_abi, crate::abi::AbiType::Borrow) {
                            return Err(invalid_mir_diagnostics(
                                "variant MIR borrow leaf does not match its ABI projection",
                            ));
                        }
                        instructions.push(Instruction::StoreAggregateUsize {
                            destination: location,
                            offset,
                            value: lower_stored_borrow_pointer(&leaf.operand, context)?,
                        });
                    }
                    crate::mir::ValueRepresentation::Aggregate => {
                        let (Operand::Copy(source) | Operand::Move(source)) = &leaf.operand else {
                            return Err(invalid_mir_diagnostics(
                                "variant MIR aggregate leaf is not stored in a place",
                            ));
                        };
                        let layout = crate::abi::layout_of(leaf_abi)
                            .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
                        instructions.push(Instruction::CopyAggregateRange {
                            destination: location,
                            destination_offset: offset,
                            source: aggregate_location(source, context)?,
                            source_offset: 0,
                            layout,
                        });
                    }
                    crate::mir::ValueRepresentation::Unit
                    | crate::mir::ValueRepresentation::Error => {
                        return Err(invalid_mir_diagnostics(
                            "variant MIR leaf has no aggregate ABI representation",
                        ));
                    }
                }
            }
            continue;
        }
        if matches!(
            value,
            Rvalue::OutcomeSuccess { .. } | Rvalue::OutcomeNone | Rvalue::OutcomeFailure { .. }
        ) {
            lower_stored_outcome_construction(context, destination, value, &mut instructions)?;
            continue;
        }
        let destination_representation = destination
            .projection
            .and_then(|projection| body.projections.get(projection.index()))
            .map_or(
                body.locals[destination.local.index()].representation,
                |path| path.representation,
            );
        if destination_representation == crate::mir::ValueRepresentation::Error {
            let (destination_code, destination_message) =
                error_place_locations(*destination, context)?;
            let (code, message) = match value {
                Rvalue::Error { code, message } => (
                    lower_str_operand(code, context)?,
                    lower_str_operand(message, context)?,
                ),
                Rvalue::Use(Operand::Copy(source) | Operand::Move(source)) => {
                    let (code, message) = error_place_locations(*source, context)?;
                    (StrValue::Location(code), StrValue::Location(message))
                }
                _ => {
                    return Err(invalid_mir_diagnostics(
                        "logical error assignment has an invalid MIR value",
                    ));
                }
            };
            instructions.push(Instruction::SetStr {
                destination: destination_code,
                value: code,
            });
            instructions.push(Instruction::SetStr {
                destination: destination_message,
                value: message,
            });
            continue;
        }
        if let Rvalue::Intrinsic {
            intrinsic,
            arguments,
            type_arguments,
            ..
        } = value
        {
            if destination_representation == crate::mir::ValueRepresentation::Aggregate {
                reserve_aggregate_destination(context, destination, &mut instructions)?;
            }
            instructions.extend(lower_intrinsic_assignment(
                destination,
                *intrinsic,
                arguments,
                type_arguments,
                context,
            )?);
            continue;
        }
        if destination_representation == crate::mir::ValueRepresentation::Aggregate {
            let Rvalue::Use(Operand::Copy(source) | Operand::Move(source)) = value else {
                return Err(invalid_mir_diagnostics(
                    "aggregate MIR assignment requires a stored source place",
                ));
            };
            let destination_ty = destination
                .projection
                .and_then(|projection| body.projections.get(projection.index()))
                .map_or(body.locals[destination.local.index()].ty, |path| path.ty);
            let layout = aggregate_local_abi_value(destination_ty, context)?.layout;
            let destination_view_index = view_index_projection(*destination, context)?;
            let source_view_index = view_index_projection(*source, context)?;
            match (destination_view_index, source_view_index) {
                (Some((destination, index)), None) if source.projection.is_none() => {
                    instructions.push(Instruction::CopyAggregateToSliceElement {
                        destination,
                        index,
                        source: aggregate_location(source, context)?,
                        layout,
                    });
                    continue;
                }
                (None, Some((source, index))) if destination.projection.is_none() => {
                    instructions.push(Instruction::CopySliceElementToAggregate {
                        destination: aggregate_location(destination, context)?,
                        source,
                        index,
                        layout,
                    });
                    continue;
                }
                (Some(_), Some(_)) => {
                    return Err(invalid_mir_diagnostics(
                        "slice-element aggregate assignment requires explicit staging",
                    ));
                }
                _ => {}
            }
            let destination_pointer = destination
                .projection
                .map(|projection| {
                    dereferenced_pointer(body, destination.local, projection, context)
                })
                .transpose()?
                .flatten();
            let source_pointer = source
                .projection
                .map(|projection| dereferenced_pointer(body, source.local, projection, context))
                .transpose()?
                .flatten();
            match (destination_pointer, source_pointer) {
                (Some((pointer, offset)), None) if source.projection.is_none() => {
                    instructions.push(Instruction::CopyAggregateToPointer {
                        pointer,
                        offset: UsizeValue::Const(u64::from(offset)),
                        source: aggregate_location(source, context)?,
                        layout,
                    });
                    continue;
                }
                (None, Some((pointer, offset))) if destination.projection.is_none() => {
                    instructions.push(Instruction::CopyPointerToAggregate {
                        destination: aggregate_location(destination, context)?,
                        pointer,
                        offset: UsizeValue::Const(u64::from(offset)),
                        layout,
                    });
                    continue;
                }
                (Some(_), Some(_)) => {
                    return Err(invalid_mir_diagnostics(
                        "aggregate pointer-to-pointer assignment requires explicit staging",
                    ));
                }
                _ => {}
            }
            if destination.projection.is_some() || source.projection.is_some() {
                let destination_range = aggregate_range(*destination, 0, context)?;
                let source_range = aggregate_range(*source, 0, context)?;
                if destination_range.index.is_none() && source_range.index.is_none() {
                    instructions.push(Instruction::CopyAggregateRange {
                        destination: destination_range.location,
                        destination_offset: destination_range.offset,
                        source: source_range.location,
                        source_offset: source_range.offset,
                        layout,
                    });
                } else {
                    instructions.push(Instruction::CopyAggregateProjected {
                        destination: destination_range,
                        source: source_range,
                        layout,
                    });
                }
            } else {
                instructions.push(Instruction::CopyAggregate {
                    destination: aggregate_location(destination, context)?,
                    source: aggregate_location(source, context)?,
                    layout,
                });
            }
            continue;
        }
        if let (crate::mir::ValueRepresentation::Borrow, Some(projection)) =
            (destination_representation, destination.projection)
        {
            let Rvalue::Use(operand) = value else {
                return Err(invalid_mir_diagnostics(
                    "borrow field assignment requires a direct MIR operand",
                ));
            };
            instructions.push(store_projected_aggregate_usize(
                destination.local,
                projection,
                0,
                lower_stored_borrow_pointer(operand, context)?,
                context,
            )?);
            continue;
        }
        if destination_representation == crate::mir::ValueRepresentation::Borrow {
            let Rvalue::Use(operand) = value else {
                return Err(invalid_mir_diagnostics(
                    "borrow assignment requires a direct MIR operand",
                ));
            };
            let (Operand::Copy(source) | Operand::Move(source)) = operand else {
                return Err(invalid_mir_diagnostics(
                    "borrow assignment requires a stored MIR place",
                ));
            };
            let destination = usize_location(destination, context)?;
            if let Some(projection) = source.projection {
                instructions.push(load_projected_aggregate_usize(
                    destination,
                    source.local,
                    projection,
                    0,
                    context,
                )?);
            } else {
                instructions.push(Instruction::SetUsize {
                    destination,
                    value: lower_stored_borrow_pointer(operand, context)?,
                });
            }
            continue;
        }
        if destination_representation
            == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str)
        {
            let operand = match value {
                Rvalue::Use(operand)
                | Rvalue::ViewCast {
                    source: operand, ..
                } => operand,
                _ => {
                    return Err(invalid_mir_diagnostics(
                        "string-view assignment requires a direct MIR operand",
                    ));
                }
            };
            if destination.projection.is_none()
                && let Operand::Copy(source) | Operand::Move(source) = operand
                && let Some(source_projection) = source.projection
                && matches!(
                    context.body.projections[source_projection.index()].element,
                    crate::mir::ProjectionElement::Field { .. }
                        | crate::mir::ProjectionElement::Index { .. }
                )
            {
                let (pointer, len) = str_word_locations(str_location(destination, context)?)?;
                instructions.push(load_projected_aggregate_usize(
                    pointer,
                    source.local,
                    source_projection,
                    0,
                    context,
                )?);
                instructions.push(load_projected_aggregate_usize(
                    len,
                    source.local,
                    source_projection,
                    crate::abi::ABI_WORD_SIZE as u32,
                    context,
                )?);
                continue;
            }
            let value = lower_str_operand(operand, context)?;
            if let Some((destination, index)) = view_index_projection(*destination, context)? {
                let index = match index {
                    crate::ir::SliceElementIndex::Const(value) => UsizeValue::Const(value),
                    crate::ir::SliceElementIndex::Location(location) => {
                        UsizeValue::Location(location)
                    }
                };
                instructions.push(Instruction::StoreStrToSliceIndex {
                    destination,
                    index,
                    value,
                });
                continue;
            }
            if let Some(projection) = destination.projection {
                let StrValue::Location(source) = value else {
                    return Err(invalid_mir_diagnostics(
                        "projected string-view assignment must materialize its source",
                    ));
                };
                instructions.push(store_projected_aggregate_usize(
                    destination.local,
                    projection,
                    0,
                    UsizeValue::StrPointer(source),
                    context,
                )?);
                instructions.push(store_projected_aggregate_usize(
                    destination.local,
                    projection,
                    crate::abi::ABI_WORD_SIZE as u32,
                    UsizeValue::StrLen(source),
                    context,
                )?);
            } else {
                instructions.push(Instruction::SetStr {
                    destination: str_location(destination, context)?,
                    value,
                });
            }
            continue;
        }
        if destination_representation
            == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice)
        {
            let operand = match value {
                Rvalue::Use(operand)
                | Rvalue::ViewCast {
                    source: operand, ..
                } => operand,
                _ => {
                    return Err(invalid_mir_diagnostics(
                        "slice-view assignment requires a direct MIR operand",
                    ));
                }
            };
            if destination.projection.is_none()
                && let Operand::Copy(source) | Operand::Move(source) = operand
                && let Some(source_projection) = source.projection
            {
                let (pointer, len) = slice_word_locations(slice_location(destination, context)?)?;
                instructions.push(load_projected_aggregate_usize(
                    pointer,
                    source.local,
                    source_projection,
                    0,
                    context,
                )?);
                instructions.push(load_projected_aggregate_usize(
                    len,
                    source.local,
                    source_projection,
                    crate::abi::ABI_WORD_SIZE as u32,
                    context,
                )?);
                continue;
            }
            let value = lower_slice_operand(operand, context)?;
            if let Some(projection) = destination.projection {
                let SliceValue::Location(source) = value else {
                    return Err(invalid_mir_diagnostics(
                        "projected slice-view assignment must materialize its source",
                    ));
                };
                instructions.push(store_projected_aggregate_usize(
                    destination.local,
                    projection,
                    0,
                    UsizeValue::SlicePointer(source),
                    context,
                )?);
                instructions.push(store_projected_aggregate_usize(
                    destination.local,
                    projection,
                    crate::abi::ABI_WORD_SIZE as u32,
                    UsizeValue::SliceLen(source),
                    context,
                )?);
            } else {
                instructions.push(Instruction::SetSlice {
                    destination: slice_location(destination, context)?,
                    value,
                });
            }
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
        let destination_scalar = local_scalar(body, destination.local).map_err(|_| {
            invalid_mir_diagnostics(format!(
                "scalar MIR assignment targets non-scalar place {destination:?}: {value:?}"
            ))
        })?;
        match destination_scalar {
            ScalarType::I32 => {
                let destination = i32_location(destination, context)?;
                match value {
                    Rvalue::OutcomeSuccess { .. }
                    | Rvalue::OutcomeNone
                    | Rvalue::OutcomeFailure { .. }
                    | Rvalue::Variant { .. }
                    | Rvalue::Discriminant { .. }
                    | Rvalue::ViewCompare { .. }
                    | Rvalue::Error { .. }
                    | Rvalue::Intrinsic { .. } => {
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
                    } => {
                        if let Operand::Constant(constant) = operand {
                            let magnitude = i64::try_from(constant.value).map_err(|_| {
                                invalid_mir_diagnostics(
                                    "negated i32 constant is outside its runtime representation",
                                )
                            })?;
                            let value = i32::try_from(-magnitude).map_err(|_| {
                                invalid_mir_diagnostics(
                                    "negated i32 constant is outside its runtime representation",
                                )
                            })?;
                            instructions.push(Instruction::SetI32 {
                                destination,
                                value: I32Value::Const(value),
                            });
                        } else {
                            instructions.push(Instruction::I32Binary {
                                operator: IntegerBinaryOperator::Subtract,
                                destination,
                                left: I32Value::Const(0),
                                right: lower_i32_operand(operand, context)?,
                            });
                        }
                    }
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
                    } => instructions.push(integer_projection::i32_binary_instruction(
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
                    Rvalue::ViewCast { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "i32 scalar route received a view coercion",
                        ));
                    }
                }
            }
            ScalarType::U8 => {
                let destination = u8_location(destination, context)?;
                match value {
                    Rvalue::OutcomeSuccess { .. }
                    | Rvalue::OutcomeNone
                    | Rvalue::OutcomeFailure { .. }
                    | Rvalue::Variant { .. }
                    | Rvalue::ViewCompare { .. }
                    | Rvalue::Error { .. }
                    | Rvalue::Intrinsic { .. } => {
                        unreachable!("aggregate rvalue handled above")
                    }
                    Rvalue::Discriminant { source, .. } => {
                        let (Operand::Copy(source) | Operand::Move(source)) = source else {
                            return Err(invalid_mir_diagnostics(
                                "enum discriminant source is not a stored aggregate",
                            ));
                        };
                        instructions.push(Instruction::LoadAggregateU8 {
                            destination,
                            source: aggregate_location(source, context)?,
                            offset: 0,
                        });
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
                    } => instructions.push(integer_projection::u8_binary_instruction(
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
                    Rvalue::ViewCast { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "u8 scalar route received a view coercion",
                        ));
                    }
                }
            }
            ScalarType::Usize => {
                let destination = usize_location(destination, context)?;
                match value {
                    Rvalue::OutcomeSuccess { .. }
                    | Rvalue::OutcomeNone
                    | Rvalue::OutcomeFailure { .. }
                    | Rvalue::Variant { .. }
                    | Rvalue::Discriminant { .. }
                    | Rvalue::ViewCompare { .. }
                    | Rvalue::Error { .. }
                    | Rvalue::Intrinsic { .. } => {
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
                    } => instructions.push(integer_projection::usize_binary_instruction(
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
                    Rvalue::ViewCast { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "usize scalar route received a view coercion",
                        ));
                    }
                }
            }
            ScalarType::Integer(kind) => {
                let destination = integer_location(destination, kind, context)?;
                match value {
                    Rvalue::OutcomeSuccess { .. }
                    | Rvalue::OutcomeNone
                    | Rvalue::OutcomeFailure { .. }
                    | Rvalue::Variant { .. }
                    | Rvalue::Discriminant { .. }
                    | Rvalue::ViewCompare { .. }
                    | Rvalue::Error { .. }
                    | Rvalue::Intrinsic { .. } => {
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
                    } if kind.is_signed() => {
                        if let Operand::Constant(constant) = operand {
                            let magnitude = u64::try_from(constant.value).map_err(|_| {
                                invalid_mir_diagnostics(
                                    "negated integer constant is outside its runtime representation",
                                )
                            })?;
                            instructions.push(Instruction::SetUsize {
                                destination,
                                value: UsizeValue::Const(kind.negated_magnitude_word(magnitude)),
                            });
                        } else {
                            instructions.push(Instruction::IntegerBinary {
                                kind,
                                operator: IntegerBinaryOperator::Subtract,
                                destination,
                                left: UsizeValue::Const(0),
                                right: lower_integer_operand(operand, kind, context)?,
                            });
                        }
                    }
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
                        operator: integer_projection::binary_operator(*operator),
                        destination,
                        left: lower_integer_operand(left, kind, context)?,
                        right: lower_integer_operand(right, kind, context)?,
                    }),
                    Rvalue::Compare { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "integer scalar route received a comparison result",
                        ));
                    }
                    Rvalue::ViewCast { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "integer scalar route received a view coercion",
                        ));
                    }
                }
            }
            ScalarType::Bool => {
                let destination = bool_location(destination, context)?;
                match value {
                    Rvalue::OutcomeSuccess { .. }
                    | Rvalue::OutcomeNone
                    | Rvalue::OutcomeFailure { .. }
                    | Rvalue::Variant { .. }
                    | Rvalue::Discriminant { .. }
                    | Rvalue::Error { .. }
                    | Rvalue::Intrinsic { .. } => {
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
                    Rvalue::ViewCompare {
                        operator,
                        left,
                        right,
                        kind: crate::mir::ViewKind::Str,
                        ..
                    } => {
                        let operator = match operator {
                            ComparisonOperator::Equal => crate::ir::BoolComparisonOperator::Equal,
                            ComparisonOperator::NotEqual => {
                                crate::ir::BoolComparisonOperator::NotEqual
                            }
                            _ => {
                                return Err(invalid_mir_diagnostics(
                                    "string views support only equality comparisons",
                                ));
                            }
                        };
                        instructions.push(Instruction::SetBool {
                            destination,
                            value: BoolValue::StrComparison {
                                operator,
                                left: lower_str_operand(left, context)?,
                                right: lower_str_operand(right, context)?,
                            },
                        });
                    }
                    Rvalue::ViewCompare { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "boolean scalar route received an unsupported view comparison",
                        ));
                    }
                    Rvalue::ViewCast { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "boolean scalar route received a view coercion",
                        ));
                    }
                }
            }
        }
    }
    Ok(instructions)
}

fn outcome_storage_for_ty(
    ty: crate::semantic::TyId,
    context: &BackendContext<'_>,
) -> Result<crate::outcomes::storage::OutcomeStorageLayout, Vec<Diagnostic>> {
    Ok(context.types.outcome(ty)?.storage)
}

fn lower_stored_outcome_construction(
    context: &BackendContext<'_>,
    destination: &Place,
    value: &Rvalue,
    instructions: &mut Vec<Instruction>,
) -> Result<(), Vec<Diagnostic>> {
    let destination_ty = destination
        .projection
        .and_then(|projection| context.body.projections.get(projection.index()))
        .map_or(context.body.locals[destination.local.index()].ty, |path| {
            path.ty
        });
    let storage = outcome_storage_for_ty(destination_ty, context)?;
    reserve_aggregate_destination(context, &Place::local(destination.local), instructions)?;
    let destination_range = aggregate_range(*destination, 0, context)?;
    let checked_offset = |offset: u64, role: &str| {
        u32::try_from(offset).ok().ok_or_else(|| {
            invalid_mir_diagnostics(format!("stored outcome {role} offset is invalid"))
        })
    };
    let store_success_prefix = |through: usize, instructions: &mut Vec<Instruction>| {
        for layer in storage.layers.iter().take(through) {
            instructions.push(aggregate_projection::store_usize(
                &destination_range,
                checked_offset(layer.tag_offset, "tag")?,
                UsizeValue::Const(0),
            )?);
        }
        Ok::<(), Vec<Diagnostic>>(())
    };
    match value {
        Rvalue::OutcomeNone => {
            let layer_index = storage
                .layers
                .iter()
                .position(|layer| layer.layer == crate::outcomes::OutcomeLayer::Optional)
                .ok_or_else(|| {
                    invalid_mir_diagnostics("none assigned to a non-optional outcome")
                })?;
            store_success_prefix(layer_index, instructions)?;
            instructions.push(aggregate_projection::store_usize(
                &destination_range,
                checked_offset(storage.layers[layer_index].tag_offset, "tag")?,
                UsizeValue::Const(1),
            )?);
        }
        Rvalue::OutcomeFailure { code, message } => {
            let layer_index = storage
                .layers
                .iter()
                .position(|layer| layer.layer == crate::outcomes::OutcomeLayer::Fallible)
                .ok_or_else(|| {
                    invalid_mir_diagnostics("failure assigned to a non-fallible outcome")
                })?;
            store_success_prefix(layer_index, instructions)?;
            let layer = storage.layers[layer_index];
            instructions.push(aggregate_projection::store_usize(
                &destination_range,
                checked_offset(layer.tag_offset, "tag")?,
                UsizeValue::Const(1),
            )?);
            let error_offset = checked_offset(
                layer.failure_offset.ok_or_else(|| {
                    invalid_mir_diagnostics("fallible outcome has no failure storage")
                })?,
                "failure",
            )?;
            let code = lower_str_operand(code, context)?;
            let message = lower_str_operand(message, context)?;
            let StrValue::Location(code) = code else {
                return Err(invalid_mir_diagnostics(
                    "stored failure code was not materialized",
                ));
            };
            let StrValue::Location(message) = message else {
                return Err(invalid_mir_diagnostics(
                    "stored failure message was not materialized",
                ));
            };
            for (offset, value) in [
                (error_offset, UsizeValue::StrPointer(code)),
                (
                    error_offset + crate::abi::ABI_WORD_SIZE as u32,
                    UsizeValue::StrLen(code),
                ),
                (
                    error_offset + 2 * crate::abi::ABI_WORD_SIZE as u32,
                    UsizeValue::StrPointer(message),
                ),
                (
                    error_offset + 3 * crate::abi::ABI_WORD_SIZE as u32,
                    UsizeValue::StrLen(message),
                ),
            ] {
                instructions.push(aggregate_projection::store_usize(
                    &destination_range,
                    offset,
                    value,
                )?);
            }
        }
        Rvalue::OutcomeSuccess { value } => {
            store_success_prefix(storage.layers.len(), instructions)?;
            let offset = checked_offset(storage.payload_offset, "payload")?;
            match value.representation {
                crate::mir::ValueRepresentation::Scalar(scalar) => {
                    instructions.push(aggregate_projection::store_scalar(
                        &destination_range,
                        offset,
                        scalar,
                        &value.operand,
                        context,
                    )?)
                }
                crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) => {
                    let StrValue::Location(source) = lower_str_operand(&value.operand, context)?
                    else {
                        return Err(invalid_mir_diagnostics(
                            "stored outcome string was not materialized",
                        ));
                    };
                    instructions.push(aggregate_projection::store_usize(
                        &destination_range,
                        offset,
                        UsizeValue::StrPointer(source),
                    )?);
                    instructions.push(aggregate_projection::store_usize(
                        &destination_range,
                        offset + crate::abi::ABI_WORD_SIZE as u32,
                        UsizeValue::StrLen(source),
                    )?);
                }
                crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice) => {
                    let SliceValue::Location(source) =
                        lower_slice_operand(&value.operand, context)?
                    else {
                        return Err(invalid_mir_diagnostics(
                            "stored outcome slice was not materialized",
                        ));
                    };
                    instructions.push(aggregate_projection::store_usize(
                        &destination_range,
                        offset,
                        UsizeValue::SlicePointer(source),
                    )?);
                    instructions.push(aggregate_projection::store_usize(
                        &destination_range,
                        offset + crate::abi::ABI_WORD_SIZE as u32,
                        UsizeValue::SliceLen(source),
                    )?);
                }
                crate::mir::ValueRepresentation::Borrow => {
                    instructions.push(aggregate_projection::store_usize(
                        &destination_range,
                        offset,
                        lower_stored_borrow_pointer(&value.operand, context)?,
                    )?)
                }
                crate::mir::ValueRepresentation::Aggregate => {
                    let (Operand::Copy(source) | Operand::Move(source)) = &value.operand else {
                        return Err(invalid_mir_diagnostics(
                            "stored outcome aggregate payload has no place",
                        ));
                    };
                    let destination = aggregate_range(*destination, offset, context)?;
                    let source = aggregate_range(*source, 0, context)?;
                    let layout = aggregate_local_abi_value(value.ty, context)?.layout;
                    if destination.index.is_none() && source.index.is_none() {
                        instructions.push(Instruction::CopyAggregateRange {
                            destination: destination.location,
                            destination_offset: destination.offset,
                            source: source.location,
                            source_offset: source.offset,
                            layout,
                        });
                    } else {
                        instructions.push(Instruction::CopyAggregateProjected {
                            destination,
                            source,
                            layout,
                        });
                    }
                }
                crate::mir::ValueRepresentation::Unit => {}
                crate::mir::ValueRepresentation::Error => {
                    return Err(invalid_mir_diagnostics(
                        "logical Error cannot be an outcome success payload",
                    ));
                }
            }
        }
        _ => unreachable!("caller filters stored outcome construction rvalues"),
    }
    Ok(())
}

fn lower_stored_borrow_pointer(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<UsizeValue, Vec<Diagnostic>> {
    let (Operand::Copy(place) | Operand::Move(place)) = operand else {
        return Err(invalid_mir_diagnostics(
            "borrow field value is not a stored MIR place",
        ));
    };
    if place.projection.is_some() {
        return Err(invalid_mir_diagnostics(
            "projected borrow values require an explicit borrow local",
        ));
    }
    let local = &context.body.locals[place.local.index()];
    if local.representation != crate::mir::ValueRepresentation::Borrow {
        return Err(invalid_mir_diagnostics(
            "borrow field value has a non-borrow representation",
        ));
    }
    Ok(UsizeValue::Location(match local.storage {
        LocalStorage::Local => UsizeLocation::Local(machine_local_index(context.body, place.local)),
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::Borrow { abi_index }) => {
                UsizeLocation::Parameter(abi_index)
            }
            _ => {
                return Err(invalid_mir_diagnostics(
                    "borrow MIR parameter has no matching ABI projection",
                ));
            }
        },
        LocalStorage::Return => {
            return Err(invalid_mir_diagnostics(
                "return storage cannot initialize a borrow field",
            ));
        }
    }))
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

fn lower_allocation_override_enter(
    id: crate::mir::AllocationOverrideId,
    context: &BackendContext<'_>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let override_ = context
        .body
        .allocation_overrides
        .get(id.index())
        .ok_or_else(|| invalid_mir_diagnostics("MIR allocation override is missing"))?;
    let ty = override_.allocator.projection.map_or(
        context.body.locals[override_.allocator.local.index()].ty,
        |projection| context.body.projections[projection.index()].ty,
    );
    let (state_offset, kind_offset) = allocator_field_offsets(ty, context)?;
    let source = aggregate_location(&override_.allocator, context)?;
    let parent_state = usize_location(&Place::local(override_.parent_state), context)?;
    let parent_kind = usize_location(&Place::local(override_.parent_kind), context)?;
    let selected_state = usize_location(&Place::local(override_.selected_state), context)?;
    let selected_kind = usize_location(&Place::local(override_.selected_kind), context)?;
    Ok(vec![
        Instruction::SetUsize {
            destination: parent_state,
            value: UsizeValue::CurrentAllocationState,
        },
        Instruction::SetUsize {
            destination: parent_kind,
            value: UsizeValue::CurrentAllocationKind,
        },
        Instruction::LoadAggregateUsize {
            destination: selected_state,
            source,
            offset: state_offset,
        },
        Instruction::LoadAggregateUsize {
            destination: selected_kind,
            source,
            offset: kind_offset,
        },
        Instruction::SetCurrentAllocationContext {
            state: UsizeValue::Location(selected_state),
            kind: UsizeValue::Location(selected_kind),
        },
    ])
}

fn lower_allocation_override_exit(
    id: crate::mir::AllocationOverrideId,
    context: &BackendContext<'_>,
) -> Result<Instruction, Vec<Diagnostic>> {
    let override_ = context
        .body
        .allocation_overrides
        .get(id.index())
        .ok_or_else(|| invalid_mir_diagnostics("MIR allocation override is missing"))?;
    Ok(Instruction::SetCurrentAllocationContext {
        state: UsizeValue::Location(usize_location(
            &Place::local(override_.parent_state),
            context,
        )?),
        kind: UsizeValue::Location(usize_location(
            &Place::local(override_.parent_kind),
            context,
        )?),
    })
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
    declared_payload_count: usize,
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
        payload if declared_payload_count == 1 && *payload_index == 0 => (0, payload),
        crate::abi::AbiType::Struct(fields) if declared_payload_count == fields.len() => {
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

fn lower_borrow_source(
    place: Place,
    context: &BackendContext<'_>,
) -> Result<crate::ir::BorrowSource, Vec<Diagnostic>> {
    if place.projection.is_none()
        && let Some(source) = storage::inlined_borrow_source(context.body, place.local)
    {
        return lower_borrow_source(source, context);
    }
    if let Some(projection) = place.projection {
        let contract = context
            .body
            .projections
            .get(projection.index())
            .filter(|projection| projection.base == place.local)
            .ok_or_else(|| invalid_mir_diagnostics("borrow projection is missing"))?;
        if let crate::mir::ProjectionElement::ViewIndex { index } = &contract.element {
            if contract.parent.is_some()
                || context.body.locals[place.local.index()].representation
                    != crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice)
            {
                return Err(invalid_mir_diagnostics(
                    "slice-view index borrow has an invalid projection base",
                ));
            }
            let element = match contract.representation {
                crate::mir::ValueRepresentation::Scalar(ScalarType::I32) => {
                    crate::ir::SliceElementAddressKind::I32
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::U8) => {
                    crate::ir::SliceElementAddressKind::U8
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::Usize) => {
                    crate::ir::SliceElementAddressKind::Usize
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::Integer(kind)) => {
                    crate::ir::SliceElementAddressKind::Integer(kind)
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::Bool) => {
                    crate::ir::SliceElementAddressKind::Bool
                }
                crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) => {
                    crate::ir::SliceElementAddressKind::Str
                }
                crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice)
                | crate::mir::ValueRepresentation::Borrow
                | crate::mir::ValueRepresentation::Aggregate => {
                    let type_expr =
                        context
                            .typed_hir
                            .type_expr_by_id(contract.ty)
                            .ok_or_else(|| {
                                invalid_mir_diagnostics(
                                    "slice-view index borrow element type is missing",
                                )
                            })?;
                    let abi = context
                        .types
                        .abi_value_for_type_expr(type_expr)
                        .map_err(|_| {
                            invalid_mir_diagnostics(
                                "slice-view index borrow element ABI is unavailable",
                            )
                        })?;
                    let stride = crate::abi::layout_of(&abi.ty)
                        .ok()
                        .and_then(|layout| u32::try_from(layout.size).ok())
                        .filter(|stride| *stride != 0)
                        .ok_or_else(|| {
                            invalid_mir_diagnostics(
                                "slice-view index borrow element stride is invalid",
                            )
                        })?;
                    crate::ir::SliceElementAddressKind::Aggregate { stride }
                }
                _ => {
                    return Err(invalid_mir_diagnostics(
                        "slice-view index borrow has an unsupported element representation",
                    ));
                }
            };
            let index = match lower_direct_usize_index(index, context)? {
                UsizeValue::Const(value) => crate::ir::SliceElementIndex::Const(value),
                UsizeValue::Location(location) => crate::ir::SliceElementIndex::Location(location),
                _ => unreachable!("direct index validation accepts only constants and places"),
            };
            return Ok(crate::ir::BorrowSource::SliceIndex {
                source: slice_location(&Place::local(place.local), context)?,
                index,
                element,
            });
        }
        if let Some((pointer, offset)) =
            dereferenced_pointer(context.body, place.local, projection, context)?
        {
            let UsizeValue::Location(pointer) = pointer else {
                return Err(invalid_mir_diagnostics(
                    "dereferenced MIR borrow is not backed by a pointer location",
                ));
            };
            return Ok(crate::ir::BorrowSource::BorrowLocalField { pointer, offset });
        }
        let projection = aggregate_borrow_projection(context.body, place.local, projection)?;
        let local = &context.body.locals[place.local.index()];
        if let AggregateBorrowProjection::Index {
            base_offset,
            index,
            length,
            stride,
        } = projection
        {
            if !matches!(
                local.representation,
                crate::mir::ValueRepresentation::Aggregate
                    | crate::mir::ValueRepresentation::Borrow
            ) {
                return Err(invalid_mir_diagnostics(
                    "indexed MIR loan base is not aggregate or borrowed storage",
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
        crate::mir::ValueRepresentation::Unit | crate::mir::ValueRepresentation::View(_) => {
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

fn error_place_locations(
    place: Place,
    context: &BackendContext<'_>,
) -> Result<(StrLocation, StrLocation), Vec<Diagnostic>> {
    if place.projection.is_some() {
        return Err(invalid_mir_diagnostics(
            "logical error storage cannot be projected as another error",
        ));
    }
    let declaration = context
        .body
        .locals
        .get(place.local.index())
        .ok_or_else(|| {
            invalid_mir_diagnostics("logical error storage refers to a missing local")
        })?;
    if declaration.representation != crate::mir::ValueRepresentation::Error {
        return Err(invalid_mir_diagnostics(
            "logical error storage has a non-error representation",
        ));
    }
    match declaration.storage {
        LocalStorage::Local => {
            let base = machine_local_index(context.body, place.local);
            Ok((StrLocation::Local(base), StrLocation::Local(base + 2)))
        }
        LocalStorage::Parameter { ordinal } => {
            let Some(parameters::ParameterStorage::Error { abi_index }) =
                context.parameters.get(ordinal)
            else {
                return Err(invalid_mir_diagnostics(
                    "logical error parameter has no matching ABI projection",
                ));
            };
            Ok((
                StrLocation::Parameter(abi_index),
                StrLocation::Parameter(abi_index + 2),
            ))
        }
        LocalStorage::Return => Err(invalid_mir_diagnostics(
            "logical error return storage is not supported",
        )),
    }
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
    if matches!(
        elements.first(),
        Some(crate::mir::ProjectionElement::Dereference)
    ) && body
        .locals
        .get(base.index())
        .is_some_and(|local| local.representation == crate::mir::ValueRepresentation::Borrow)
    {
        elements.remove(0);
    }

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
            crate::mir::ProjectionElement::ViewIndex { .. } => {
                return Err(invalid_mir_diagnostics(
                    "slice-view index cannot participate in an aggregate projection",
                ));
            }
            crate::mir::ProjectionElement::Dereference => {
                return Err(invalid_mir_diagnostics(
                    "dereference requires pointer-backed MIR projection",
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
    if let crate::mir::ProjectionElement::ViewIndex { index } = &contract.element {
        if contract.parent.is_some()
            || context.body.locals[destination.local.index()].representation
                != crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice)
        {
            return Err(invalid_mir_diagnostics(
                "slice-view index store has an invalid projection base",
            ));
        }
        let destination = slice_location(&Place::local(destination.local), context)?;
        let index = lower_direct_usize_index(index, context)?;
        return Ok(match scalar {
            ScalarType::I32 => Instruction::StoreI32ToSliceIndex {
                destination,
                index,
                value: lower_i32_operand(operand, context)?,
            },
            ScalarType::U8 => Instruction::StoreU8ToSliceIndex {
                destination,
                index,
                value: lower_u8_operand(operand, context)?,
            },
            ScalarType::Usize => Instruction::StoreUsizeToSliceIndex {
                destination,
                index,
                value: lower_usize_operand(operand, context)?,
            },
            ScalarType::Integer(kind) => Instruction::StoreIntegerToSliceIndex {
                kind,
                destination,
                index,
                value: lower_integer_operand(operand, kind, context)?,
            },
            ScalarType::Bool => Instruction::StoreBoolToSliceIndex {
                destination,
                index,
                value: lower_bool_operand(operand, context)?,
            },
        });
    }
    if let Some((pointer, offset)) =
        dereferenced_pointer(context.body, destination.local, projection_id, context)?
    {
        return Ok(match scalar {
            ScalarType::I32 => Instruction::StoreI32ToPointer {
                pointer,
                offset: UsizeValue::Const(u64::from(offset)),
                value: lower_i32_operand(operand, context)?,
            },
            ScalarType::U8 => Instruction::StoreU8ToPointer {
                pointer,
                offset: UsizeValue::Const(u64::from(offset)),
                value: lower_u8_operand(operand, context)?,
            },
            ScalarType::Usize => Instruction::StoreUsizeToPointer {
                pointer,
                offset: UsizeValue::Const(u64::from(offset)),
                value: lower_usize_operand(operand, context)?,
            },
            ScalarType::Integer(kind) => Instruction::StoreIntegerToPointer {
                kind,
                pointer,
                offset: UsizeValue::Const(u64::from(offset)),
                value: lower_integer_operand(operand, kind, context)?,
            },
            ScalarType::Bool => Instruction::StoreBoolToPointer {
                pointer,
                offset: UsizeValue::Const(u64::from(offset)),
                value: lower_bool_operand(operand, context)?,
            },
        });
    }
    let location = aggregate_location(&Place::local(destination.local), context)?;
    let range = match aggregate_borrow_projection(context.body, destination.local, projection_id)? {
        AggregateBorrowProjection::Field { offset } => AggregateRange {
            location,
            offset,
            index: None,
        },
        AggregateBorrowProjection::Index {
            base_offset,
            index,
            length,
            stride,
        } => AggregateRange {
            location,
            offset: base_offset,
            index: Some(AggregateIndex {
                value: lower_direct_usize_index(&index, context)?,
                length,
                stride,
            }),
        },
    };
    aggregate_projection::store_scalar(&range, 0, scalar, operand, context)
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
    let projection_contract = context
        .body
        .projections
        .get(projection.index())
        .filter(|projection| projection.base == place.local)
        .ok_or_else(|| invalid_mir_diagnostics("scalar load projection is missing"))?;
    if let crate::mir::ProjectionElement::ViewIndex { index } = &projection_contract.element {
        if projection_contract.parent.is_none()
            && context.body.locals[place.local.index()].representation
                == crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str)
            && projection_contract.representation
                == crate::mir::ValueRepresentation::Scalar(crate::mir::ScalarType::U8)
        {
            let ScalarDestination::U8(destination) = destination else {
                return Err(invalid_mir_diagnostics(
                    "string-view index load does not target a u8 value",
                ));
            };
            return Ok(Some(Instruction::SetU8 {
                destination,
                value: U8Value::StrIndex {
                    source: str_location(&Place::local(place.local), context)?,
                    index: lower_direct_usize_index(index, context)?,
                },
            }));
        }
        if projection_contract.parent.is_some()
            || context.body.locals[place.local.index()].representation
                != crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice)
        {
            return Err(invalid_mir_diagnostics(
                "slice-view index load has an invalid projection base",
            ));
        }
        let source = slice_location(&Place::local(place.local), context)?;
        let index = lower_direct_usize_index(index, context)?;
        return Ok(Some(match destination {
            ScalarDestination::I32(destination) => Instruction::SetI32 {
                destination,
                value: I32Value::SliceIndex { source, index },
            },
            ScalarDestination::U8(destination) => Instruction::SetU8 {
                destination,
                value: U8Value::SliceIndex { source, index },
            },
            ScalarDestination::Usize(destination) => Instruction::SetUsize {
                destination,
                value: UsizeValue::SliceIndex {
                    source,
                    index: Box::new(index),
                },
            },
            ScalarDestination::Integer(kind, destination) => Instruction::SetUsize {
                destination,
                value: UsizeValue::IntegerSliceIndex {
                    kind,
                    source,
                    index: Box::new(index),
                },
            },
            ScalarDestination::Bool(destination) => Instruction::SetBool {
                destination,
                value: BoolValue::SliceIndex { source, index },
            },
        }));
    }
    if let Some((pointer, offset)) =
        dereferenced_pointer(context.body, place.local, projection, context)?
    {
        let offset = UsizeValue::Const(u64::from(offset));
        return Ok(Some(match destination {
            ScalarDestination::I32(destination) => Instruction::LoadI32FromPointer {
                destination,
                pointer,
                offset,
            },
            ScalarDestination::U8(destination) => Instruction::LoadU8FromPointer {
                destination,
                pointer,
                offset,
            },
            ScalarDestination::Usize(destination) => Instruction::LoadUsizeFromPointer {
                destination,
                pointer,
                offset,
            },
            ScalarDestination::Integer(kind, destination) => Instruction::LoadIntegerFromPointer {
                kind,
                destination,
                pointer,
                offset,
            },
            ScalarDestination::Bool(destination) => Instruction::LoadBoolFromPointer {
                destination,
                pointer,
                offset,
            },
        }));
    }
    let location = aggregate_location(&Place::local(place.local), context)?;
    let range = match aggregate_borrow_projection(context.body, place.local, projection)? {
        AggregateBorrowProjection::Field { offset } => AggregateRange {
            location,
            offset,
            index: None,
        },
        AggregateBorrowProjection::Index {
            base_offset,
            index,
            length,
            stride,
        } => AggregateRange {
            location,
            offset: base_offset,
            index: Some(AggregateIndex {
                value: lower_direct_usize_index(&index, context)?,
                length,
                stride,
            }),
        },
    };
    Ok(Some(aggregate_projection::load_scalar(destination, &range)))
}

fn dereferenced_pointer(
    body: &Body,
    base: LocalId,
    mut projection: crate::mir::ProjectionPathId,
    context: &BackendContext<'_>,
) -> Result<Option<(UsizeValue, u32)>, Vec<Diagnostic>> {
    let mut elements = Vec::new();
    loop {
        let path = body
            .projections
            .get(projection.index())
            .ok_or_else(|| invalid_mir_diagnostics("MIR dereference projection is missing"))?;
        if path.base != base {
            return Err(invalid_mir_diagnostics(
                "MIR dereference projection changed base local",
            ));
        }
        elements.push(path.element.clone());
        let Some(parent) = path.parent else {
            break;
        };
        projection = parent;
    }
    elements.reverse();
    if !matches!(
        elements.first(),
        Some(crate::mir::ProjectionElement::Dereference)
    ) {
        return Ok(None);
    }
    let mut offset = 0u32;
    for element in &elements[1..] {
        let crate::mir::ProjectionElement::Field {
            offset: field_offset,
        } = element
        else {
            // Dynamic indexes and other structured paths use the general
            // aggregate projection with `AggregateLocation::Borrow`. This
            // helper is only the direct constant-field fast path.
            return Ok(None);
        };
        offset = offset
            .checked_add(*field_offset)
            .ok_or_else(|| invalid_mir_diagnostics("dereference field offset overflowed"))?;
    }
    let pointer = lower_stored_borrow_pointer(&Operand::Copy(Place::local(base)), context)?;
    Ok(Some((pointer, offset)))
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

pub(super) fn view_index_projection(
    place: Place,
    context: &BackendContext<'_>,
) -> Result<Option<(SliceLocation, crate::ir::SliceElementIndex)>, Vec<Diagnostic>> {
    let Some(projection) = place.projection else {
        return Ok(None);
    };
    let contract = context
        .body
        .projections
        .get(projection.index())
        .filter(|contract| contract.base == place.local)
        .ok_or_else(|| invalid_mir_diagnostics("slice-view projection is missing"))?;
    let crate::mir::ProjectionElement::ViewIndex { index } = &contract.element else {
        return Ok(None);
    };
    if contract.parent.is_some()
        || context.body.locals[place.local.index()].representation
            != crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice)
    {
        return Err(invalid_mir_diagnostics(
            "slice-view aggregate projection has an invalid base",
        ));
    }
    let index = match lower_direct_usize_index(index, context)? {
        UsizeValue::Const(value) => crate::ir::SliceElementIndex::Const(value),
        UsizeValue::Location(location) => crate::ir::SliceElementIndex::Location(location),
        _ => unreachable!("direct index validation accepts only constants and places"),
    };
    Ok(Some((
        slice_location(&Place::local(place.local), context)?,
        index,
    )))
}

pub(super) fn temporary_aggregate_slot(context: &BackendContext<'_>) -> usize {
    context.parameters.first_local_aggregate_slot()
        + context
            .body
            .locals
            .iter()
            .filter(|local| {
                local.storage == LocalStorage::Local
                    && local.representation == crate::mir::ValueRepresentation::Aggregate
            })
            .count()
}

pub(super) fn aggregate_range(
    place: Place,
    additional_offset: u32,
    context: &BackendContext<'_>,
) -> Result<AggregateRange, Vec<Diagnostic>> {
    let location = aggregate_location(&Place::local(place.local), context)?;
    let projection = place
        .projection
        .map(|projection| aggregate_borrow_projection(context.body, place.local, projection))
        .transpose()?;
    match projection {
        None => Ok(AggregateRange {
            location,
            offset: additional_offset,
            index: None,
        }),
        Some(AggregateBorrowProjection::Field { offset }) => Ok(AggregateRange {
            location,
            offset: offset.checked_add(additional_offset).ok_or_else(|| {
                invalid_mir_diagnostics("projected aggregate range offset overflowed")
            })?,
            index: None,
        }),
        Some(AggregateBorrowProjection::Index {
            base_offset,
            index,
            length,
            stride,
        }) => Ok(AggregateRange {
            location,
            offset: base_offset.checked_add(additional_offset).ok_or_else(|| {
                invalid_mir_diagnostics("indexed aggregate range offset overflowed")
            })?,
            index: Some(AggregateIndex {
                value: lower_direct_usize_index(&index, context)?,
                length,
                stride,
            }),
        }),
    }
}

fn store_projected_aggregate_usize(
    base: LocalId,
    projection: crate::mir::ProjectionPathId,
    additional_offset: u32,
    value: UsizeValue,
    context: &BackendContext<'_>,
) -> Result<Instruction, Vec<Diagnostic>> {
    let range = aggregate_range(Place::projected(base, projection), 0, context)?;
    aggregate_projection::store_usize(&range, additional_offset, value)
}

fn load_projected_aggregate_usize(
    destination: UsizeLocation,
    base: LocalId,
    projection: crate::mir::ProjectionPathId,
    additional_offset: u32,
    context: &BackendContext<'_>,
) -> Result<Instruction, Vec<Diagnostic>> {
    if let Some((pointer, offset)) = dereferenced_pointer(context.body, base, projection, context)?
    {
        let offset = offset
            .checked_add(additional_offset)
            .ok_or_else(|| invalid_mir_diagnostics("dereferenced field offset overflowed"))?;
        return Ok(Instruction::LoadUsizeFromPointer {
            destination,
            pointer,
            offset: UsizeValue::Const(u64::from(offset)),
        });
    }
    let range = aggregate_range(Place::projected(base, projection), 0, context)?;
    aggregate_projection::load_usize(destination, &range, additional_offset)
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
            actual => Err(invalid_mir_diagnostics(format!(
                "i32 MIR parameter ordinal {ordinal} has ABI projection {actual:?}"
            ))),
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
        .ok_or_else(|| {
            invalid_mir_diagnostics(format!(
                "scalar MIR lowering received non-scalar local {local:?} with representation {:?} and type {:?}",
                body.locals[local.index()].representation,
                body.locals[local.index()].ty,
            ))
        })
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
            if place.projection.is_none()
                && let Some(source) =
                    storage::inlined_identity_intrinsic_source(context.body, place.local)
            {
                lower_usize_operand(source, context)
            } else {
                usize_location(place, context).map(UsizeValue::Location)
            }
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
            if let Some(projection) = place.projection
                && let Some(path) = context.body.projections.get(projection.index())
                && let crate::mir::ProjectionElement::ViewIndex { index } = &path.element
            {
                if path.parent.is_some()
                    || context.body.locals[place.local.index()].representation
                        != crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Slice)
                {
                    return Err(invalid_mir_diagnostics(
                        "string slice index has an invalid MIR base",
                    ));
                }
                return Ok(StrValue::SliceIndex {
                    source: slice_location(&Place::local(place.local), context)?,
                    index: lower_direct_usize_index(index, context)?,
                });
            }
            if place.projection.is_none()
                && let Some(source) = storage::inlined_view_cast_source(context.body, place.local)
            {
                lower_str_operand(source, context)
            } else {
                str_location(place, context).map(StrValue::Location)
            }
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
        if declaration.representation != crate::mir::ValueRepresentation::Error {
            return Err(invalid_mir_diagnostics(
                "error field projection is not backed by a logical error local",
            ));
        }
        let field_offset = match field {
            crate::builtin_types::BuiltinErrorField::Code => 0,
            crate::builtin_types::BuiltinErrorField::Message => 2,
        };
        return match declaration.storage {
            LocalStorage::Local => Ok(StrLocation::Local(
                machine_local_index(context.body, place.local) + field_offset,
            )),
            LocalStorage::Parameter { ordinal } => {
                let Some(parameters::ParameterStorage::Error { abi_index }) =
                    context.parameters.get(ordinal)
                else {
                    return Err(invalid_mir_diagnostics(
                        "logical error parameter has no matching ABI projection",
                    ));
                };
                Ok(StrLocation::Parameter(abi_index + field_offset))
            }
            LocalStorage::Return => Err(invalid_mir_diagnostics(
                "logical error return field cannot be projected",
            )),
        };
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

fn str_word_locations(
    location: StrLocation,
) -> Result<(UsizeLocation, UsizeLocation), Vec<Diagnostic>> {
    match location {
        StrLocation::Return => Err(invalid_mir_diagnostics(
            "a projected string view must be staged before return",
        )),
        StrLocation::Local(index) => {
            Ok((UsizeLocation::Local(index), UsizeLocation::Local(index + 1)))
        }
        StrLocation::Parameter(_) => Err(invalid_mir_diagnostics(
            "a string-view parameter cannot be an assignment destination",
        )),
    }
}

fn lower_intrinsic_assignment(
    destination: &Place,
    intrinsic: crate::intrinsics::IntrinsicId,
    arguments: &[crate::mir::CallArgument],
    type_arguments: &[crate::semantic::TyId],
    context: &BackendContext<'_>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let arguments = lower_call_arguments(arguments, context)?;
    let invalid = || invalid_mir_diagnostics("intrinsic MIR argument contract is invalid");
    let instruction = match (intrinsic, arguments.as_slice()) {
        (
            crate::intrinsics::IntrinsicId::TakeValueAtPtr,
            [
                ScalarArgument::Usize(pointer),
                ScalarArgument::Usize(offset),
            ],
        ) => {
            let destination_ty = destination
                .projection
                .and_then(|projection| context.body.projections.get(projection.index()))
                .map_or(context.body.locals[destination.local.index()].ty, |path| {
                    path.ty
                });
            match context.body.locals[destination.local.index()].representation {
                crate::mir::ValueRepresentation::Aggregate => {
                    let layout = aggregate_local_abi_value(destination_ty, context)?.layout;
                    Instruction::CopyPointerToAggregate {
                        destination: aggregate_location(destination, context)?,
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                        layout,
                    }
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::I32) => {
                    Instruction::LoadI32FromPointer {
                        destination: i32_location(destination, context)?,
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                    }
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::U8) => {
                    Instruction::LoadU8FromPointer {
                        destination: u8_location(destination, context)?,
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                    }
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::Usize) => {
                    Instruction::LoadUsizeFromPointer {
                        destination: usize_location(destination, context)?,
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                    }
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::Integer(kind)) => {
                    Instruction::LoadIntegerFromPointer {
                        kind,
                        destination: integer_location(destination, kind, context)?,
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                    }
                }
                crate::mir::ValueRepresentation::Scalar(ScalarType::Bool) => {
                    Instruction::LoadBoolFromPointer {
                        destination: bool_location(destination, context)?,
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                    }
                }
                crate::mir::ValueRepresentation::View(crate::mir::ViewKind::Str) => {
                    Instruction::LoadStrFromPointer {
                        destination: str_location(destination, context)?,
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                    }
                }
                _ => return Err(invalid()),
            }
        }
        (
            crate::intrinsics::IntrinsicId::Addr | crate::intrinsics::IntrinsicId::FromAddr,
            [ScalarArgument::Usize(value)],
        ) => Instruction::SetUsize {
            destination: usize_location(destination, context)?,
            value: value.clone(),
        },
        (
            crate::intrinsics::IntrinsicId::FromRef | crate::intrinsics::IntrinsicId::FromRefMut,
            [ScalarArgument::Borrow(value)],
        ) => Instruction::SetUsizeFromBorrow {
            destination: usize_location(destination, context)?,
            source: value.source,
        },
        (crate::intrinsics::IntrinsicId::PointeeSize, [ScalarArgument::Usize(_)]) => {
            let ty = *type_arguments.first().ok_or_else(invalid)?;
            Instruction::SetUsize {
                destination: usize_location(destination, context)?,
                value: UsizeValue::Const(aggregate_local_abi_value(ty, context)?.layout.size),
            }
        }
        (crate::intrinsics::IntrinsicId::PointeeAlign, [ScalarArgument::Usize(_)]) => {
            let ty = *type_arguments.first().ok_or_else(invalid)?;
            Instruction::SetUsize {
                destination: usize_location(destination, context)?,
                value: UsizeValue::Const(aggregate_local_abi_value(ty, context)?.layout.align),
            }
        }
        (crate::intrinsics::IntrinsicId::BytesFromStr, [ScalarArgument::Str(value)]) => {
            Instruction::SetSlice {
                destination: slice_location(destination, context)?,
                value: SliceValue::StrBytes(value.clone()),
            }
        }
        (crate::intrinsics::IntrinsicId::StrLenRaw, [ScalarArgument::Str(value)]) => {
            let StrValue::Location(value) = value else {
                return Err(invalid());
            };
            Instruction::SetUsize {
                destination: usize_location(destination, context)?,
                value: UsizeValue::StrLen(*value),
            }
        }
        (crate::intrinsics::IntrinsicId::SliceLenRaw, [ScalarArgument::Slice(value)]) => {
            let SliceValue::Location(value) = value else {
                return Err(invalid());
            };
            Instruction::SetUsize {
                destination: usize_location(destination, context)?,
                value: UsizeValue::SliceLen(*value),
            }
        }
        (crate::intrinsics::IntrinsicId::StrPtrAddrRaw, [ScalarArgument::Str(value)]) => {
            let StrValue::Location(value) = value else {
                return Err(invalid());
            };
            Instruction::SetUsize {
                destination: usize_location(destination, context)?,
                value: UsizeValue::StrPointer(*value),
            }
        }
        (crate::intrinsics::IntrinsicId::SlicePtrAddrRaw, [ScalarArgument::Slice(value)]) => {
            let SliceValue::Location(value) = value else {
                return Err(invalid());
            };
            Instruction::SetUsize {
                destination: usize_location(destination, context)?,
                value: UsizeValue::SlicePointer(*value),
            }
        }
        (crate::intrinsics::IntrinsicId::ArgCountRaw, []) => Instruction::SetUsize {
            destination: usize_location(destination, context)?,
            value: UsizeValue::ProcessArgCount,
        },
        (crate::intrinsics::IntrinsicId::EnvCountRaw, []) => Instruction::SetUsize {
            destination: usize_location(destination, context)?,
            value: UsizeValue::ProcessEnvironmentCount,
        },
        (crate::intrinsics::IntrinsicId::CurrentAllocatorState, []) => Instruction::SetUsize {
            destination: usize_location(destination, context)?,
            value: UsizeValue::CurrentAllocationState,
        },
        (crate::intrinsics::IntrinsicId::CurrentAllocatorKind, []) => Instruction::SetUsize {
            destination: usize_location(destination, context)?,
            value: UsizeValue::CurrentAllocationKind,
        },
        (crate::intrinsics::IntrinsicId::ArgRaw, [ScalarArgument::Usize(index)]) => {
            Instruction::SetStr {
                destination: str_location(destination, context)?,
                value: StrValue::ProcessArg {
                    index: index.clone(),
                },
            }
        }
        (crate::intrinsics::IntrinsicId::EnvNameRaw, [ScalarArgument::Usize(index)]) => {
            Instruction::SetStr {
                destination: str_location(destination, context)?,
                value: StrValue::ProcessEnvironmentName {
                    index: index.clone(),
                },
            }
        }
        (crate::intrinsics::IntrinsicId::EnvValueRaw, [ScalarArgument::Usize(index)]) => {
            Instruction::SetStr {
                destination: str_location(destination, context)?,
                value: StrValue::ProcessEnvironmentValue {
                    index: index.clone(),
                },
            }
        }
        (
            crate::intrinsics::IntrinsicId::StrFromRawParts,
            [ScalarArgument::Usize(pointer), ScalarArgument::Usize(len)],
        ) => Instruction::SetStrRawParts {
            destination: str_location(destination, context)?,
            pointer: pointer.clone(),
            len: len.clone(),
        },
        (
            crate::intrinsics::IntrinsicId::StrSubviewUnchecked,
            [
                ScalarArgument::Str(source),
                ScalarArgument::Usize(start),
                ScalarArgument::Usize(len),
            ],
        ) => Instruction::SetStrSubview {
            destination: str_location(destination, context)?,
            source: source.clone(),
            start: start.clone(),
            len: len.clone(),
        },
        (
            crate::intrinsics::IntrinsicId::SliceFromRawParts
            | crate::intrinsics::IntrinsicId::SliceFromRawPartsMut
            | crate::intrinsics::IntrinsicId::SliceFromRawPartsValue
            | crate::intrinsics::IntrinsicId::SliceFromRawPartsValueMut,
            [ScalarArgument::Usize(pointer), ScalarArgument::Usize(len)],
        ) => Instruction::SetSliceRawParts {
            destination: slice_location(destination, context)?,
            pointer: pointer.clone(),
            len: len.clone(),
        },
        (crate::intrinsics::IntrinsicId::Syscall(arity), arguments) => {
            let (ScalarArgument::Usize(number), arguments) =
                arguments.split_first().ok_or_else(invalid)?
            else {
                return Err(invalid());
            };
            if arguments.len() != usize::from(arity)
                || arguments
                    .iter()
                    .any(|argument| !matches!(argument, ScalarArgument::Usize(_)))
            {
                return Err(invalid());
            }
            Instruction::DarwinSyscall {
                destination: aggregate_location(destination, context)?,
                arity,
                number: number.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        let ScalarArgument::Usize(value) = argument else {
                            unreachable!("syscall arguments were validated above")
                        };
                        value.clone()
                    })
                    .collect(),
            }
        }
        _ => return Err(invalid()),
    };
    Ok(vec![instruction])
}

fn lower_intrinsic_effect(
    intrinsic: crate::intrinsics::IntrinsicId,
    arguments: &[crate::mir::CallArgument],
    type_arguments: &[crate::semantic::TyId],
    context: &BackendContext<'_>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let arguments = lower_call_arguments(arguments, context)?;
    let invalid = || invalid_mir_diagnostics("intrinsic MIR effect contract is invalid");
    Ok(match (intrinsic, arguments.as_slice()) {
        (crate::intrinsics::IntrinsicId::CloseFdRaw, [ScalarArgument::I32(fd)]) => {
            vec![Instruction::CloseFd { fd: fd.clone() }]
        }
        (
            crate::intrinsics::IntrinsicId::CopyStrToPtr,
            [
                ScalarArgument::Usize(pointer),
                ScalarArgument::Usize(offset),
                ScalarArgument::Str(text),
            ],
        ) => vec![Instruction::CopyStrToPointer {
            pointer: pointer.clone(),
            offset: offset.clone(),
            text: text.clone(),
        }],
        (
            crate::intrinsics::IntrinsicId::CopyPtrToPtr,
            [
                ScalarArgument::Usize(destination),
                ScalarArgument::Usize(source),
                ScalarArgument::Usize(byte_count),
            ],
        ) => vec![Instruction::CopyPointerBytes {
            destination: destination.clone(),
            source: source.clone(),
            byte_count: byte_count.clone(),
        }],
        (
            crate::intrinsics::IntrinsicId::StoreU8ToPtr,
            [
                ScalarArgument::Usize(pointer),
                ScalarArgument::Usize(offset),
                ScalarArgument::U8(value),
            ],
        ) => vec![Instruction::StoreU8ToPointer {
            pointer: pointer.clone(),
            offset: offset.clone(),
            value: value.clone(),
        }],
        (
            crate::intrinsics::IntrinsicId::StoreValueToPtr,
            [
                ScalarArgument::Usize(pointer),
                ScalarArgument::Usize(offset),
                value,
            ],
        ) => {
            let store = match value {
                ScalarArgument::I32(value) => Instruction::StoreI32ToPointer {
                    pointer: pointer.clone(),
                    offset: offset.clone(),
                    value: value.clone(),
                },
                ScalarArgument::U8(value) => Instruction::StoreU8ToPointer {
                    pointer: pointer.clone(),
                    offset: offset.clone(),
                    value: value.clone(),
                },
                ScalarArgument::Usize(value) => Instruction::StoreUsizeToPointer {
                    pointer: pointer.clone(),
                    offset: offset.clone(),
                    value: value.clone(),
                },
                ScalarArgument::Integer(kind, value) => Instruction::StoreIntegerToPointer {
                    kind: *kind,
                    pointer: pointer.clone(),
                    offset: offset.clone(),
                    value: value.clone(),
                },
                ScalarArgument::Bool(value) => Instruction::StoreBoolToPointer {
                    pointer: pointer.clone(),
                    offset: offset.clone(),
                    value: value.clone(),
                },
                ScalarArgument::Str(value) => Instruction::StoreStrToPointer {
                    pointer: pointer.clone(),
                    offset: offset.clone(),
                    value: value.clone(),
                },
                ScalarArgument::AggregateDirect(value) => {
                    let AggregateArgumentSource::Slot(source) = value.source else {
                        return Err(invalid());
                    };
                    Instruction::CopyAggregateToPointer {
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                        source: crate::ir::AggregateLocation::Slot(source),
                        layout: value.layout,
                    }
                }
                ScalarArgument::AggregateIndirect(value) => {
                    let AggregateArgumentSource::Slot(source) = value.source else {
                        return Err(invalid());
                    };
                    let ty = *type_arguments.first().ok_or_else(invalid)?;
                    Instruction::CopyAggregateToPointer {
                        pointer: pointer.clone(),
                        offset: offset.clone(),
                        source: crate::ir::AggregateLocation::Slot(source),
                        layout: aggregate_local_abi_value(ty, context)?.layout,
                    }
                }
                ScalarArgument::Borrow(value) => {
                    let temporary =
                        UsizeLocation::Local(storage::machine_local_count(context.body));
                    return Ok(vec![
                        Instruction::SetUsizeFromBorrow {
                            destination: temporary,
                            source: value.source,
                        },
                        Instruction::StoreUsizeToPointer {
                            pointer: pointer.clone(),
                            offset: offset.clone(),
                            value: UsizeValue::Location(temporary),
                        },
                    ]);
                }
                ScalarArgument::Slice(_) => return Err(invalid()),
            };
            vec![store]
        }
        _ => return Err(invalid()),
    })
}

fn lower_slice_operand(
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<SliceValue, Vec<Diagnostic>> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            if place.projection.is_none()
                && let Some(source) = storage::inlined_view_cast_source(context.body, place.local)
            {
                lower_slice_operand(source, context)
            } else {
                slice_location(place, context).map(SliceValue::Location)
            }
        }
        Operand::StaticStr { .. } | Operand::Constant(_) => Err(invalid_mir_diagnostics(
            "non-slice value used as a slice-view operand",
        )),
    }
}

fn slice_location(
    place: &Place,
    context: &BackendContext<'_>,
) -> Result<SliceLocation, Vec<Diagnostic>> {
    if place.projection.is_some() {
        return Err(invalid_mir_diagnostics(
            "projected slice-view storage is not yet supported",
        ));
    }
    match context.body.locals[place.local.index()].storage {
        LocalStorage::Return => Ok(SliceLocation::Return),
        LocalStorage::Parameter { ordinal } => match context.parameters.get(ordinal) {
            Some(parameters::ParameterStorage::Slice { abi_index }) => {
                Ok(SliceLocation::Parameter(abi_index))
            }
            _ => Err(invalid_mir_diagnostics(
                "slice-view MIR parameter has no matching ABI projection",
            )),
        },
        LocalStorage::Local => Ok(SliceLocation::Local(machine_local_index(
            context.body,
            place.local,
        ))),
    }
}

fn slice_word_locations(
    location: SliceLocation,
) -> Result<(UsizeLocation, UsizeLocation), Vec<Diagnostic>> {
    match location {
        SliceLocation::Return => Err(invalid_mir_diagnostics(
            "a projected slice view must be staged before return",
        )),
        SliceLocation::Local(index) => {
            Ok((UsizeLocation::Local(index), UsizeLocation::Local(index + 1)))
        }
        SliceLocation::Parameter(_) => Err(invalid_mir_diagnostics(
            "a slice-view parameter cannot be an assignment destination",
        )),
    }
}

fn invalid_mir_diagnostics(error: impl std::fmt::Debug) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8000",
        format!("compiler produced invalid MIR: {error:?}"),
    )]
}

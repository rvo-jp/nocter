//! MIR-to-machine-IR lowering. This module grows only after the corresponding
//! AST-driven lowering family has been removed from its production route.

use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolComparisonOperator, BoolLocation, BoolValue, I32ComparisonOperator, I32Location, I32Value,
    Instruction, OutcomeFailureMode, ScalarArgument, Type, UsizeLocation, UsizeValue,
};
use crate::mir::{
    BinaryOperator, Body, CallContinuation, ComparisonOperator, LocalId, LocalSource, Operand,
    Place, Rvalue, ScalarType, Statement, Terminator,
};
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::TypedHir;
use std::collections::HashSet;

mod control_flow;

pub(super) fn try_lower_scalar_body(
    cache: &crate::mir::BodyCache,
    body: &crate::ast::Block,
    parameters: &[crate::ast::Parameter],
    return_type: &Type,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    root_source: SourceId,
    sources: &SourceMap,
) -> Option<Result<Vec<Instruction>, Vec<Diagnostic>>> {
    let return_scalar = match return_type {
        Type::I32 => ScalarType::I32,
        Type::Usize => ScalarType::Usize,
        Type::Bool => ScalarType::Bool,
        _ => return None,
    };
    let body_id = resolved.semantic_db.body_at(body.span)?;
    let mir_body = cache.get_or_build(body_id, || {
        crate::mir::try_build_scalar_body(
            body,
            parameters,
            return_scalar,
            &resolved.semantic_db,
            resolved,
            typed_hir,
        )
    })?;
    Some(match mir_body {
        Ok(mir_body) => lower_scalar_body(
            &mir_body,
            return_type,
            resolved,
            function_name,
            function_signatures,
            function_names,
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
    function_name: &str,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    root_source: SourceId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    crate::mir::validate(body).map_err(invalid_mir_diagnostics)?;
    let mut instructions = Vec::new();
    let mut current = body.entry;
    let mut visited = HashSet::new();

    loop {
        if !visited.insert(current) {
            return Err(invalid_mir_diagnostics("control flow contains a cycle"));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(body, &block.statements)?);

        match &block.terminator {
            Terminator::Goto { target } => current = *target,
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                let then_join = control_flow::linear_branch_join(body, *then_target)
                    .map_err(invalid_mir_diagnostics)?;
                let else_join = control_flow::linear_branch_join(body, *else_target)
                    .map_err(invalid_mir_diagnostics)?;
                if then_join != else_join {
                    return Err(invalid_mir_diagnostics(
                        "scalar conditional branches must share one join block",
                    ));
                }
                instructions.push(Instruction::If {
                    condition: lower_bool_operand(condition, body)?,
                    then_instructions: lower_linear_branch(
                        body,
                        *then_target,
                        then_join,
                        resolved,
                        function_signatures,
                        function_names,
                        root_source,
                        &mut visited,
                    )?,
                    else_instructions: lower_linear_branch(
                        body,
                        *else_target,
                        else_join,
                        resolved,
                        function_signatures,
                        function_names,
                        root_source,
                        &mut visited,
                    )?,
                });
                current = then_join;
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
                    .map(|argument| lower_call_argument(argument, body))
                    .collect::<Result<Vec<_>, _>>()?;
                let fits_tail_call_abi = arguments
                    .iter()
                    .map(ScalarArgument::abi_word_count)
                    .sum::<usize>()
                    <= crate::abi::ARGUMENT_REGISTER_COUNT;

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
                        let destination_scalar = body.locals[destination.local.index()].scalar;
                        super::expressions::validate_known_call_success_return_passing(
                            function_signatures.success_return_passing(&call_target),
                            &callee_name,
                            &scalar_ir_type(destination_scalar),
                        )?;
                        let returns_directly = destination.local == body.return_local
                            && target_block.statements.is_empty()
                            && target_block.terminator == Terminator::Return;
                        if returns_directly {
                            validate_tail_call_return_type(
                                &call_target,
                                &callee_name,
                                function_name,
                                return_type,
                                function_signatures,
                            )?;
                        }
                        if returns_directly && fits_tail_call_abi {
                            instructions.push(Instruction::TailCall {
                                target: call_target,
                                arguments,
                            });
                            return Ok(instructions);
                        }
                        instructions.push(call_instruction(
                            destination_scalar,
                            destination,
                            call_target,
                            arguments,
                            body,
                        )?);
                        current = *target;
                    }
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                    } => {
                        let failure_block = &body.blocks[failure.index()];
                        if !failure_block.statements.is_empty()
                            || failure_block.terminator != Terminator::Trap
                        {
                            return Err(invalid_mir_diagnostics(
                                "trapping outcome call must have a dedicated trap block",
                            ));
                        }
                        let destination_scalar = body.locals[destination.local.index()].scalar;
                        validate_outcome_call_return_type(
                            &call_target,
                            &callee_name,
                            destination_scalar,
                            function_signatures,
                        )?;
                        instructions.push(outcome_call_instruction(
                            destination_scalar,
                            destination,
                            call_target,
                            arguments,
                            OutcomeFailureMode::Trap,
                            body,
                        )?);
                        visited.insert(*failure);
                        current = *success;
                    }
                }
            }
            Terminator::Trap => {
                instructions.push(Instruction::Trap);
                return Ok(instructions);
            }
            Terminator::Return => {
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
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
    let Type::Fallible(success) = return_type else {
        return Err(invalid_mir_diagnostics(format!(
            "outcome call to `{callee_name}` does not return a fallible value"
        )));
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

#[allow(clippy::too_many_arguments)]
fn lower_linear_branch(
    body: &Body,
    start: crate::mir::BasicBlockId,
    join: crate::mir::BasicBlockId,
    resolved: &ResolveOutput,
    function_signatures: &super::context::FunctionSignatures,
    function_names: &super::context::FunctionNames,
    root_source: SourceId,
    visited: &mut HashSet<crate::mir::BasicBlockId>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    let mut current = start;
    loop {
        if !visited.insert(current) {
            return Err(invalid_mir_diagnostics(
                "control-flow branch reuses an already lowered block",
            ));
        }
        let block = &body.blocks[current.index()];
        instructions.extend(lower_statements(body, &block.statements)?);
        match &block.terminator {
            Terminator::Goto { target } if *target == join => return Ok(instructions),
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
                let (call_target, callee_name) =
                    lower_call_target(*callee, resolved, function_names, root_source)?;
                let arguments = arguments
                    .iter()
                    .map(|argument| lower_call_argument(argument, body))
                    .collect::<Result<Vec<_>, _>>()?;
                let destination_scalar = body.locals[destination.local.index()].scalar;
                super::expressions::validate_known_call_success_return_passing(
                    function_signatures.success_return_passing(&call_target),
                    &callee_name,
                    &scalar_ir_type(destination_scalar),
                )?;
                instructions.push(call_instruction(
                    destination_scalar,
                    destination,
                    call_target,
                    arguments,
                    body,
                )?);
                current = *target;
            }
            _ => {
                return Err(invalid_mir_diagnostics(
                    "scalar conditional branch does not follow a linear path to its join",
                ));
            }
        }
    }
}

fn call_instruction(
    scalar: ScalarType,
    destination: &Place,
    target: crate::ir::CallTarget,
    arguments: Vec<ScalarArgument>,
    body: &Body,
) -> Result<Instruction, Vec<Diagnostic>> {
    Ok(match scalar {
        ScalarType::I32 => Instruction::CallI32 {
            destination: i32_location(destination, body)?,
            target,
            arguments,
        },
        ScalarType::Usize => Instruction::CallUsize {
            destination: usize_location(destination, body)?,
            target,
            arguments,
        },
        ScalarType::Bool => Instruction::CallBool {
            destination: bool_location(destination, body)?,
            target,
            arguments,
        },
    })
}

fn outcome_call_instruction(
    scalar: ScalarType,
    destination: &Place,
    target: crate::ir::CallTarget,
    arguments: Vec<ScalarArgument>,
    failure_mode: OutcomeFailureMode,
    body: &Body,
) -> Result<Instruction, Vec<Diagnostic>> {
    Ok(match scalar {
        ScalarType::I32 => Instruction::CallOutcomeI32 {
            destination: i32_location(destination, body)?,
            target,
            arguments,
            failure_mode,
        },
        ScalarType::Usize => Instruction::CallOutcomeUsize {
            destination: usize_location(destination, body)?,
            target,
            arguments,
            failure_mode,
        },
        ScalarType::Bool => Instruction::CallOutcomeBool {
            destination: bool_location(destination, body)?,
            target,
            arguments,
            failure_mode,
        },
    })
}

fn scalar_ir_type(scalar: ScalarType) -> Type {
    match scalar {
        ScalarType::I32 => Type::I32,
        ScalarType::Usize => Type::Usize,
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
    body: &Body,
) -> Result<ScalarArgument, Vec<Diagnostic>> {
    Ok(match argument.scalar {
        ScalarType::I32 => ScalarArgument::I32(lower_i32_operand(&argument.operand, body)?),
        ScalarType::Usize => ScalarArgument::Usize(lower_usize_operand(&argument.operand, body)?),
        ScalarType::Bool => ScalarArgument::Bool(lower_bool_operand(&argument.operand, body)?),
    })
}

fn lower_statements(
    body: &Body,
    statements: &[Statement],
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for statement in statements {
        let Statement::Assign {
            destination, value, ..
        } = statement;
        match body.locals[destination.local.index()].scalar {
            ScalarType::I32 => {
                let destination = i32_location(destination, body)?;
                match value {
                    Rvalue::Use(operand) => instructions.push(Instruction::SetI32 {
                        destination,
                        value: lower_i32_operand(operand, body)?,
                    }),
                    Rvalue::Binary {
                        operator,
                        left,
                        right,
                        ..
                    } => instructions.push(i32_binary_instruction(
                        *operator,
                        destination,
                        lower_i32_operand(left, body)?,
                        lower_i32_operand(right, body)?,
                    )),
                    Rvalue::Compare { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "i32 scalar route received a comparison result",
                        ));
                    }
                }
            }
            ScalarType::Usize => {
                let destination = usize_location(destination, body)?;
                match value {
                    Rvalue::Use(operand) => instructions.push(Instruction::SetUsize {
                        destination,
                        value: lower_usize_operand(operand, body)?,
                    }),
                    Rvalue::Binary {
                        operator,
                        left,
                        right,
                        ..
                    } => instructions.push(usize_binary_instruction(
                        *operator,
                        destination,
                        lower_usize_operand(left, body)?,
                        lower_usize_operand(right, body)?,
                    )),
                    Rvalue::Compare { .. } => {
                        return Err(invalid_mir_diagnostics(
                            "usize scalar route received a comparison result",
                        ));
                    }
                }
            }
            ScalarType::Bool => {
                let destination = bool_location(destination, body)?;
                match value {
                    Rvalue::Use(operand) => instructions.push(Instruction::SetBool {
                        destination,
                        value: lower_bool_operand(operand, body)?,
                    }),
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
                        value: lower_comparison(*operator, left, right, *operand_scalar, body)?,
                    }),
                }
            }
        }
    }
    Ok(instructions)
}

fn lower_comparison(
    operator: ComparisonOperator,
    left: &Operand,
    right: &Operand,
    operand_scalar: ScalarType,
    body: &Body,
) -> Result<BoolValue, Vec<Diagnostic>> {
    Ok(match operand_scalar {
        ScalarType::I32 => BoolValue::I32Comparison {
            operator: integer_comparison_operator(operator),
            left: lower_i32_operand(left, body)?,
            right: lower_i32_operand(right, body)?,
        },
        ScalarType::Usize => BoolValue::UsizeComparison {
            operator: integer_comparison_operator(operator),
            left: lower_usize_operand(left, body)?,
            right: lower_usize_operand(right, body)?,
        },
        ScalarType::Bool => BoolValue::BoolComparison {
            operator: bool_comparison_operator(operator)?,
            left: Box::new(lower_bool_operand(left, body)?),
            right: Box::new(lower_bool_operand(right, body)?),
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
    }
}

fn i32_location(place: &Place, body: &Body) -> Result<I32Location, Vec<Diagnostic>> {
    match &body.locals[place.local.index()].source {
        LocalSource::Return => Ok(I32Location::Return),
        LocalSource::Parameter { index, .. } => Ok(I32Location::Parameter(*index)),
        LocalSource::Binding(_) | LocalSource::Temporary(_) => {
            Ok(I32Location::Local(machine_local_index(body, place.local)))
        }
    }
}

fn usize_location(place: &Place, body: &Body) -> Result<UsizeLocation, Vec<Diagnostic>> {
    match &body.locals[place.local.index()].source {
        LocalSource::Return => Ok(UsizeLocation::Return),
        LocalSource::Parameter { index, .. } => Ok(UsizeLocation::Parameter(*index)),
        LocalSource::Binding(_) | LocalSource::Temporary(_) => {
            Ok(UsizeLocation::Local(machine_local_index(body, place.local)))
        }
    }
}

fn bool_location(place: &Place, body: &Body) -> Result<BoolLocation, Vec<Diagnostic>> {
    match &body.locals[place.local.index()].source {
        LocalSource::Return => Ok(BoolLocation::Return),
        LocalSource::Parameter { index, .. } => Ok(BoolLocation::Parameter(*index)),
        LocalSource::Binding(_) | LocalSource::Temporary(_) => {
            Ok(BoolLocation::Local(machine_local_index(body, place.local)))
        }
    }
}

fn machine_local_index(body: &Body, local: LocalId) -> usize {
    body.locals[..local.index()]
        .iter()
        .filter(|local| {
            matches!(
                local.source,
                LocalSource::Binding(_) | LocalSource::Temporary(_)
            )
        })
        .count()
}

fn lower_i32_operand(operand: &Operand, body: &Body) -> Result<I32Value, Vec<Diagnostic>> {
    match operand {
        Operand::Constant(constant) => {
            i32::try_from(constant.value)
                .map(I32Value::Const)
                .map_err(|_| {
                    invalid_mir_diagnostics("i32 constant is outside its runtime representation")
                })
        }
        Operand::Copy(place) => i32_location(place, body).map(I32Value::Location),
    }
}

fn lower_usize_operand(operand: &Operand, body: &Body) -> Result<UsizeValue, Vec<Diagnostic>> {
    match operand {
        Operand::Constant(constant) => u64::try_from(constant.value)
            .map(UsizeValue::Const)
            .map_err(|_| {
                invalid_mir_diagnostics("usize constant is outside its runtime representation")
            }),
        Operand::Copy(place) => usize_location(place, body).map(UsizeValue::Location),
    }
}

fn lower_bool_operand(operand: &Operand, body: &Body) -> Result<BoolValue, Vec<Diagnostic>> {
    match operand {
        Operand::Constant(constant) => match constant.value {
            0 => Ok(BoolValue::Const(false)),
            1 => Ok(BoolValue::Const(true)),
            _ => Err(invalid_mir_diagnostics(
                "bool constant is outside its runtime representation",
            )),
        },
        Operand::Copy(place) => bool_location(place, body).map(BoolValue::Location),
    }
}

fn invalid_mir_diagnostics(error: impl std::fmt::Debug) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8000",
        format!("compiler produced invalid MIR: {error:?}"),
    )]
}

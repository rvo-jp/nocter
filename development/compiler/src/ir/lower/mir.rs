//! MIR-to-machine-IR lowering. This module grows only after the corresponding
//! AST-driven lowering family has been removed from its production route.

use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolComparisonOperator, BoolLocation, BoolValue, I32ComparisonOperator, I32Location, I32Value,
    Instruction, Type, UsizeLocation, UsizeValue,
};
use crate::mir::{
    BinaryOperator, Body, ComparisonOperator, LocalId, LocalSource, Operand, Place, Rvalue,
    ScalarType, Statement, Terminator,
};
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::TypedHir;
use std::collections::HashSet;

pub(super) fn try_lower_scalar_body(
    cache: &crate::mir::BodyCache,
    body: &crate::ast::Block,
    parameters: &[crate::ast::Parameter],
    return_type: &Type,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
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
        Ok(mir_body) => lower_scalar_body(&mir_body)
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

fn lower_scalar_body(body: &Body) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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
                if !visited.insert(*then_target) || !visited.insert(*else_target) {
                    return Err(invalid_mir_diagnostics(
                        "control-flow branch reuses an already lowered block",
                    ));
                }
                let then_block = &body.blocks[then_target.index()];
                let else_block = &body.blocks[else_target.index()];
                let (
                    Terminator::Goto { target: then_join },
                    Terminator::Goto { target: else_join },
                ) = (&then_block.terminator, &else_block.terminator)
                else {
                    return Err(invalid_mir_diagnostics(
                        "scalar conditional branches must join explicitly",
                    ));
                };
                if then_join != else_join {
                    return Err(invalid_mir_diagnostics(
                        "scalar conditional branches must share one join block",
                    ));
                }
                instructions.push(Instruction::If {
                    condition: lower_bool_operand(condition, body)?,
                    then_instructions: lower_statements(body, &then_block.statements)?,
                    else_instructions: lower_statements(body, &else_block.statements)?,
                });
                current = *then_join;
            }
            Terminator::Return => {
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
        }
    }
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

//! MIR-to-machine-IR lowering. This module grows only after the corresponding
//! AST-driven lowering family has been removed from its production route.

use crate::diagnostics::Diagnostic;
use crate::ir::{
    BoolLocation, BoolValue, I32Location, I32Value, Instruction, Type, UsizeLocation, UsizeValue,
};
use crate::mir::{
    BinaryOperator, Body, LocalId, LocalSource, Operand, Place, Rvalue, ScalarType, Statement,
    Terminator,
};
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::TypedHir;
use std::collections::HashSet;

pub(super) fn try_lower_scalar_body(
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
    let mir_body = crate::mir::try_build_scalar_body(
        body,
        parameters,
        return_scalar,
        &resolved.semantic_db,
        resolved,
        typed_hir,
    )?;
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
        for statement in &block.statements {
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
                    }
                }
            }
        }

        match block.terminator {
            Terminator::Goto { target } => current = target,
            Terminator::Return => {
                instructions.push(Instruction::Return);
                return Ok(instructions);
            }
        }
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

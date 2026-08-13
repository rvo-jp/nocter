//! MIR-to-machine-IR lowering. This module grows only after the corresponding
//! AST-driven lowering family has been removed from its production route.

use crate::diagnostics::Diagnostic;
use crate::ir::{I32Location, I32Value, Instruction, Type, UsizeLocation, UsizeValue};
use crate::mir::{BinaryOperator, Body, Operand, Place, Rvalue, Statement, Terminator};
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::TypedHir;
use std::collections::HashSet;

pub(super) fn try_lower_scalar_body(
    body: &crate::ast::Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    sources: &SourceMap,
) -> Option<Result<Vec<Instruction>, Vec<Diagnostic>>> {
    if !matches!(return_type, Type::I32 | Type::Usize) {
        return None;
    }
    let mir_body =
        crate::mir::try_build_scalar_body(body, &resolved.semantic_db, resolved, typed_hir)?;
    Some(match mir_body {
        Ok(mir_body) => lower_scalar_body(&mir_body, return_type)
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

fn lower_scalar_body(body: &Body, return_type: &Type) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
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
            match return_type {
                Type::I32 => {
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
                Type::Usize => {
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
                _ => {
                    return Err(invalid_mir_diagnostics(
                        "scalar literal route received a non-scalar return type",
                    ));
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
    if place.local == body.return_local {
        Ok(I32Location::Return)
    } else {
        Ok(I32Location::Local(place.local.index() - 1))
    }
}

fn usize_location(place: &Place, body: &Body) -> Result<UsizeLocation, Vec<Diagnostic>> {
    if place.local == body.return_local {
        Ok(UsizeLocation::Return)
    } else {
        Ok(UsizeLocation::Local(place.local.index() - 1))
    }
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

fn invalid_mir_diagnostics(error: impl std::fmt::Debug) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8000",
        format!("compiler produced invalid MIR: {error:?}"),
    )]
}

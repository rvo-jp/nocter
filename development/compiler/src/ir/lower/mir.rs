//! MIR-to-machine-IR lowering. This module grows only after the corresponding
//! AST-driven lowering family has been removed from its production route.

use crate::diagnostics::Diagnostic;
use crate::ir::{I32Location, I32Value, Instruction, Type, UsizeLocation, UsizeValue};
use crate::mir::{Body, Operand, Place, Rvalue, Statement, Terminator};
use std::collections::HashSet;

pub(super) fn lower_scalar_body(
    body: &Body,
    return_type: &Type,
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
        for statement in &block.statements {
            let Statement::Assign {
                destination,
                value: Rvalue::Use(operand),
                ..
            } = statement;
            match return_type {
                Type::I32 => {
                    let value = lower_i32_operand(operand, body)?;
                    instructions.push(Instruction::SetI32 {
                        destination: i32_location(destination, body)?,
                        value,
                    });
                }
                Type::Usize => {
                    let value = lower_usize_operand(operand, body)?;
                    instructions.push(Instruction::SetUsize {
                        destination: usize_location(destination, body)?,
                        value,
                    });
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

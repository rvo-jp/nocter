//! MIR-to-machine-IR lowering. This module grows only after the corresponding
//! AST-driven lowering family has been removed from its production route.

use crate::diagnostics::Diagnostic;
use crate::ir::{I32Location, I32Value, Instruction, Type, UsizeLocation, UsizeValue};
use crate::mir::{Body, Operand, Rvalue, Statement, Terminator};
use std::collections::HashSet;

pub(super) fn lower_scalar_literal_body(
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
                value: Rvalue::Use(Operand::Constant(constant)),
                ..
            } = statement;
            if destination.local != body.return_local || constant.ty != body.return_type().unwrap()
            {
                return Err(invalid_mir_diagnostics(
                    "scalar literal route assigned a non-return place",
                ));
            }
            match return_type {
                Type::I32 => {
                    let value = i32::try_from(constant.value).map_err(|_| {
                        invalid_mir_diagnostics(
                            "i32 constant is outside its runtime representation",
                        )
                    })?;
                    instructions.push(Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: I32Value::Const(value),
                    });
                }
                Type::Usize => {
                    let value = u64::try_from(constant.value).map_err(|_| {
                        invalid_mir_diagnostics(
                            "usize constant is outside its runtime representation",
                        )
                    })?;
                    instructions.push(Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: UsizeValue::Const(value),
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

fn invalid_mir_diagnostics(error: impl std::fmt::Debug) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8000",
        format!("compiler produced invalid MIR: {error:?}"),
    )]
}

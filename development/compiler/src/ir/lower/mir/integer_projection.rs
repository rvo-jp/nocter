//! Projection of checked MIR integer operations into machine-IR families.
//!
//! The semantic operator is selected once and retained uniformly.  Separate
//! machine value/location families remain only where their overflow checks or
//! register representations differ.

use crate::ir::{
    I32Location, I32Value, Instruction, IntegerBinaryOperator, U8Location, U8Value, UsizeLocation,
    UsizeValue,
};
use crate::mir::BinaryOperator;

pub(super) fn i32_binary_instruction(
    operator: BinaryOperator,
    destination: I32Location,
    left: I32Value,
    right: I32Value,
) -> Instruction {
    Instruction::I32Binary {
        operator: binary_operator(operator),
        destination,
        left,
        right,
    }
}

pub(super) fn u8_binary_instruction(
    operator: BinaryOperator,
    destination: U8Location,
    left: U8Value,
    right: U8Value,
) -> Instruction {
    Instruction::U8Binary {
        operator: binary_operator(operator),
        destination,
        left,
        right,
    }
}

pub(super) fn usize_binary_instruction(
    operator: BinaryOperator,
    destination: UsizeLocation,
    left: UsizeValue,
    right: UsizeValue,
) -> Instruction {
    Instruction::UsizeBinary {
        operator: binary_operator(operator),
        destination,
        left,
        right,
    }
}

pub(super) fn binary_operator(operator: BinaryOperator) -> IntegerBinaryOperator {
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

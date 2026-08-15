//! Projection of checked aggregate ranges onto typed machine instructions.
//!
//! MIR owns field and index identity while `AggregateRange` owns the resolved
//! machine location, byte offset, and optional bounds-checked index.  This
//! module is the sole type-dispatch boundary that turns that range into a
//! scalar aggregate load or store instruction.

use super::*;

pub(super) fn abi_matches_scalar(abi: &crate::abi::AbiType, scalar: ScalarType) -> bool {
    match scalar {
        ScalarType::I32 => *abi == crate::abi::AbiType::I32,
        ScalarType::U8 => *abi == crate::abi::AbiType::U8,
        ScalarType::Usize => *abi == crate::abi::AbiType::Usize,
        ScalarType::Bool => *abi == crate::abi::AbiType::Bool,
        ScalarType::Integer(kind) => abi.integer_type() == Some(kind),
    }
}

pub(super) fn store_usize(
    range: &AggregateRange,
    additional_offset: u32,
    value: UsizeValue,
) -> Result<Instruction, Vec<Diagnostic>> {
    let offset = checked_offset(range.offset, additional_offset, "word")?;
    Ok(match &range.index {
        None => Instruction::StoreAggregateUsize {
            destination: range.location,
            offset,
            value,
        },
        Some(index) => Instruction::StoreAggregateUsizeIndexed {
            destination: range.location,
            base_offset: offset,
            index: index.value.clone(),
            length: index.length,
            stride: index.stride,
            value,
        },
    })
}

pub(super) fn load_usize(
    destination: UsizeLocation,
    range: &AggregateRange,
    additional_offset: u32,
) -> Result<Instruction, Vec<Diagnostic>> {
    let offset = checked_offset(range.offset, additional_offset, "word")?;
    Ok(match &range.index {
        None => Instruction::LoadAggregateUsize {
            destination,
            source: range.location,
            offset,
        },
        Some(index) => Instruction::LoadAggregateUsizeIndexed {
            destination,
            source: range.location,
            base_offset: offset,
            index: index.value.clone(),
            length: index.length,
            stride: index.stride,
        },
    })
}

pub(super) fn store_scalar(
    range: &AggregateRange,
    additional_offset: u32,
    scalar: ScalarType,
    operand: &Operand,
    context: &BackendContext<'_>,
) -> Result<Instruction, Vec<Diagnostic>> {
    let offset = checked_offset(range.offset, additional_offset, "scalar")?;
    Ok(match (&range.index, scalar) {
        (None, ScalarType::I32) => Instruction::StoreAggregateI32 {
            destination: range.location,
            offset,
            value: lower_i32_operand(operand, context)?,
        },
        (None, ScalarType::U8) => Instruction::StoreAggregateU8 {
            destination: range.location,
            offset,
            value: lower_u8_operand(operand, context)?,
        },
        (None, ScalarType::Usize) => Instruction::StoreAggregateUsize {
            destination: range.location,
            offset,
            value: lower_usize_operand(operand, context)?,
        },
        (None, ScalarType::Integer(kind)) => Instruction::StoreAggregateInteger {
            kind,
            destination: range.location,
            offset,
            value: lower_integer_operand(operand, kind, context)?,
        },
        (None, ScalarType::Bool) => Instruction::StoreAggregateBool {
            destination: range.location,
            offset,
            value: lower_bool_operand(operand, context)?,
        },
        (Some(index), ScalarType::I32) => Instruction::StoreAggregateI32Indexed {
            destination: range.location,
            base_offset: offset,
            index: index.value.clone(),
            length: index.length,
            stride: index.stride,
            value: lower_i32_operand(operand, context)?,
        },
        (Some(index), ScalarType::U8) => Instruction::StoreAggregateU8Indexed {
            destination: range.location,
            base_offset: offset,
            index: index.value.clone(),
            length: index.length,
            stride: index.stride,
            value: lower_u8_operand(operand, context)?,
        },
        (Some(index), ScalarType::Usize) => Instruction::StoreAggregateUsizeIndexed {
            destination: range.location,
            base_offset: offset,
            index: index.value.clone(),
            length: index.length,
            stride: index.stride,
            value: lower_usize_operand(operand, context)?,
        },
        (Some(index), ScalarType::Integer(kind)) => Instruction::StoreAggregateIntegerIndexed {
            kind,
            destination: range.location,
            base_offset: offset,
            index: index.value.clone(),
            length: index.length,
            stride: index.stride,
            value: lower_integer_operand(operand, kind, context)?,
        },
        (Some(index), ScalarType::Bool) => Instruction::StoreAggregateBoolIndexed {
            destination: range.location,
            base_offset: offset,
            index: index.value.clone(),
            length: index.length,
            stride: index.stride,
            value: lower_bool_operand(operand, context)?,
        },
    })
}

pub(super) fn load_scalar(destination: ScalarDestination, range: &AggregateRange) -> Instruction {
    match (&range.index, destination) {
        (None, ScalarDestination::I32(destination)) => Instruction::LoadAggregateI32 {
            destination,
            source: range.location,
            offset: range.offset,
        },
        (None, ScalarDestination::U8(destination)) => Instruction::LoadAggregateU8 {
            destination,
            source: range.location,
            offset: range.offset,
        },
        (None, ScalarDestination::Usize(destination)) => Instruction::LoadAggregateUsize {
            destination,
            source: range.location,
            offset: range.offset,
        },
        (None, ScalarDestination::Integer(kind, destination)) => {
            Instruction::LoadAggregateInteger {
                kind,
                destination,
                source: range.location,
                offset: range.offset,
            }
        }
        (None, ScalarDestination::Bool(destination)) => Instruction::LoadAggregateBool {
            destination,
            source: range.location,
            offset: range.offset,
        },
        (Some(index), ScalarDestination::I32(destination)) => {
            Instruction::LoadAggregateI32Indexed {
                destination,
                source: range.location,
                base_offset: range.offset,
                index: index.value.clone(),
                length: index.length,
                stride: index.stride,
            }
        }
        (Some(index), ScalarDestination::U8(destination)) => Instruction::LoadAggregateU8Indexed {
            destination,
            source: range.location,
            base_offset: range.offset,
            index: index.value.clone(),
            length: index.length,
            stride: index.stride,
        },
        (Some(index), ScalarDestination::Usize(destination)) => {
            Instruction::LoadAggregateUsizeIndexed {
                destination,
                source: range.location,
                base_offset: range.offset,
                index: index.value.clone(),
                length: index.length,
                stride: index.stride,
            }
        }
        (Some(index), ScalarDestination::Integer(kind, destination)) => {
            Instruction::LoadAggregateIntegerIndexed {
                kind,
                destination,
                source: range.location,
                base_offset: range.offset,
                index: index.value.clone(),
                length: index.length,
                stride: index.stride,
            }
        }
        (Some(index), ScalarDestination::Bool(destination)) => {
            Instruction::LoadAggregateBoolIndexed {
                destination,
                source: range.location,
                base_offset: range.offset,
                index: index.value.clone(),
                length: index.length,
                stride: index.stride,
            }
        }
    }
}

fn checked_offset(base: u32, additional: u32, kind: &str) -> Result<u32, Vec<Diagnostic>> {
    base.checked_add(additional).ok_or_else(|| {
        invalid_mir_diagnostics(format!("projected aggregate {kind} offset overflowed"))
    })
}

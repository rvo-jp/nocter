//! Loads and stores for compiler-owned closure environment fields.

use super::*;

pub(super) fn lower_i32_closure_capture_to_location(
    identifier: &IdentifierExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.closure_capture_field(&identifier.name) else {
        return Ok(None);
    };
    match &field.kind {
        AggregateFieldKind::I32 => Ok(Some(vec![Instruction::LoadAggregateI32 {
            destination,
            source: field.source,
            offset: field.offset,
        }])),
        AggregateFieldKind::Borrow {
            inner: Type::I32, ..
        } => {
            let pointer = temporaries.next_usize()?;
            Ok(Some(vec![
                Instruction::LoadAggregateUsize {
                    destination: pointer,
                    source: field.source,
                    offset: field.offset,
                },
                Instruction::LoadI32FromPointer {
                    destination,
                    pointer: UsizeValue::Location(pointer),
                    offset: UsizeValue::Const(0),
                },
            ]))
        }
        _ => Ok(None),
    }
}

pub(in crate::ir::lower) fn lower_i32_closure_capture_assignment(
    identifier: &IdentifierExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.closure_capture_field(&identifier.name) else {
        return Ok(None);
    };
    if !field.is_readwrite {
        return Ok(None);
    }
    if !matches!(
        &field.kind,
        AggregateFieldKind::I32
            | AggregateFieldKind::Borrow {
                is_readwrite: true,
                inner: Type::I32,
            }
    ) {
        return Ok(None);
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let value_location = temporaries.next_i32()?;
    let mut instructions = lower_i32_expression_to_location(value, value_location, context)?;
    match &field.kind {
        AggregateFieldKind::I32 => {
            instructions.push(Instruction::StoreAggregateI32 {
                destination: field.source,
                offset: field.offset,
                value: I32Value::Location(value_location),
            });
            Ok(Some(instructions))
        }
        AggregateFieldKind::Borrow {
            is_readwrite: true,
            inner: Type::I32,
        } => {
            let pointer = temporaries.next_usize()?;
            instructions.push(Instruction::LoadAggregateUsize {
                destination: pointer,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(Instruction::StoreI32ToPointer {
                pointer: UsizeValue::Location(pointer),
                offset: UsizeValue::Const(0),
                value: I32Value::Location(value_location),
            });
            Ok(Some(instructions))
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_u8_closure_capture_to_location(
    identifier: &IdentifierExpr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.closure_capture_field(&identifier.name) else {
        return Ok(None);
    };
    match &field.kind {
        AggregateFieldKind::U8 => Ok(Some(vec![Instruction::LoadAggregateU8 {
            destination,
            source: field.source,
            offset: field.offset,
        }])),
        AggregateFieldKind::Borrow {
            inner: Type::U8, ..
        } => {
            let pointer = temporaries.next_usize()?;
            Ok(Some(vec![
                Instruction::LoadAggregateUsize {
                    destination: pointer,
                    source: field.source,
                    offset: field.offset,
                },
                Instruction::LoadU8FromPointer {
                    destination,
                    pointer: UsizeValue::Location(pointer),
                    offset: UsizeValue::Const(0),
                },
            ]))
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_usize_closure_capture_to_location(
    identifier: &IdentifierExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.closure_capture_field(&identifier.name) else {
        return Ok(None);
    };
    match &field.kind {
        AggregateFieldKind::Usize => Ok(Some(vec![Instruction::LoadAggregateUsize {
            destination,
            source: field.source,
            offset: field.offset,
        }])),
        AggregateFieldKind::Borrow {
            inner: Type::Usize, ..
        } => {
            let pointer = temporaries.next_usize()?;
            Ok(Some(vec![
                Instruction::LoadAggregateUsize {
                    destination: pointer,
                    source: field.source,
                    offset: field.offset,
                },
                Instruction::LoadUsizeFromPointer {
                    destination,
                    pointer: UsizeValue::Location(pointer),
                    offset: UsizeValue::Const(0),
                },
            ]))
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_bool_closure_capture_to_location(
    identifier: &IdentifierExpr,
    destination: BoolLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.closure_capture_field(&identifier.name) else {
        return Ok(None);
    };
    match &field.kind {
        AggregateFieldKind::Bool => Ok(Some(vec![Instruction::LoadAggregateBool {
            destination,
            source: field.source,
            offset: field.offset,
        }])),
        AggregateFieldKind::Borrow {
            inner: Type::Bool, ..
        } => {
            let pointer = temporaries.next_usize()?;
            Ok(Some(vec![
                Instruction::LoadAggregateUsize {
                    destination: pointer,
                    source: field.source,
                    offset: field.offset,
                },
                Instruction::LoadBoolFromPointer {
                    destination,
                    pointer: UsizeValue::Location(pointer),
                    offset: UsizeValue::Const(0),
                },
            ]))
        }
        _ => Ok(None),
    }
}

pub(in crate::ir::lower) fn lower_u8_closure_capture_assignment(
    identifier: &IdentifierExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.closure_capture_field(&identifier.name) else {
        return Ok(None);
    };
    if !field.is_readwrite {
        return Ok(None);
    }
    if !matches!(
        &field.kind,
        AggregateFieldKind::U8
            | AggregateFieldKind::Borrow {
                is_readwrite: true,
                inner: Type::U8,
            }
    ) {
        return Ok(None);
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let value_location = temporaries.next_u8()?;
    let mut instructions = lower_u8_expression_to_location(value, value_location, context)?;
    match &field.kind {
        AggregateFieldKind::U8 => {
            instructions.push(Instruction::StoreAggregateU8 {
                destination: field.source,
                offset: field.offset,
                value: U8Value::Location(value_location),
            });
            Ok(Some(instructions))
        }
        AggregateFieldKind::Borrow {
            is_readwrite: true,
            inner: Type::U8,
        } => {
            let pointer = temporaries.next_usize()?;
            instructions.push(Instruction::LoadAggregateUsize {
                destination: pointer,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(Instruction::StoreU8ToPointer {
                pointer: UsizeValue::Location(pointer),
                offset: UsizeValue::Const(0),
                value: U8Value::Location(value_location),
            });
            Ok(Some(instructions))
        }
        _ => Ok(None),
    }
}

pub(in crate::ir::lower) fn lower_usize_closure_capture_assignment(
    identifier: &IdentifierExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.closure_capture_field(&identifier.name) else {
        return Ok(None);
    };
    if !field.is_readwrite {
        return Ok(None);
    }
    if !matches!(
        &field.kind,
        AggregateFieldKind::Usize
            | AggregateFieldKind::Borrow {
                is_readwrite: true,
                inner: Type::Usize,
            }
    ) {
        return Ok(None);
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let value_location = temporaries.next_usize()?;
    let mut instructions = lower_usize_expression_to_location(value, value_location, context)?;
    match &field.kind {
        AggregateFieldKind::Usize => {
            instructions.push(Instruction::StoreAggregateUsize {
                destination: field.source,
                offset: field.offset,
                value: UsizeValue::Location(value_location),
            });
            Ok(Some(instructions))
        }
        AggregateFieldKind::Borrow {
            is_readwrite: true,
            inner: Type::Usize,
        } => {
            let pointer = temporaries.next_usize()?;
            instructions.push(Instruction::LoadAggregateUsize {
                destination: pointer,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(Instruction::StoreUsizeToPointer {
                pointer: UsizeValue::Location(pointer),
                offset: UsizeValue::Const(0),
                value: UsizeValue::Location(value_location),
            });
            Ok(Some(instructions))
        }
        _ => Ok(None),
    }
}

pub(in crate::ir::lower) fn lower_bool_closure_capture_assignment(
    identifier: &IdentifierExpr,
    value: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.closure_capture_field(&identifier.name) else {
        return Ok(None);
    };
    if !field.is_readwrite {
        return Ok(None);
    }
    if !matches!(
        &field.kind,
        AggregateFieldKind::Bool
            | AggregateFieldKind::Borrow {
                is_readwrite: true,
                inner: Type::Bool,
            }
    ) {
        return Ok(None);
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let value_location = temporaries.next_bool()?;
    let mut instructions =
        lower_bool_expression_to_location(value, value_location, context, "E8008")?;
    match &field.kind {
        AggregateFieldKind::Bool => {
            instructions.push(Instruction::StoreAggregateBool {
                destination: field.source,
                offset: field.offset,
                value: BoolValue::Location(value_location),
            });
            Ok(Some(instructions))
        }
        AggregateFieldKind::Borrow {
            is_readwrite: true,
            inner: Type::Bool,
        } => {
            let pointer = temporaries.next_usize()?;
            instructions.push(Instruction::LoadAggregateUsize {
                destination: pointer,
                source: field.source,
                offset: field.offset,
            });
            instructions.push(Instruction::StoreBoolToPointer {
                pointer: UsizeValue::Location(pointer),
                offset: UsizeValue::Const(0),
                value: BoolValue::Location(value_location),
            });
            Ok(Some(instructions))
        }
        _ => Ok(None),
    }
}

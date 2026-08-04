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

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ir::lower) enum PointerTakeDestination {
    I32(I32Location),
    U8(U8Location),
    Usize(UsizeLocation),
    Bool(BoolLocation),
    Str(StrLocation),
    Aggregate {
        location: AggregateLocation,
        layout: ValueLayout,
    },
}

pub(in crate::ir::lower) fn primitive_take_value_at_ptr_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    context.primitive_name_for_call(call) == Some("take_value_at_ptr")
}

pub(in crate::ir::lower) fn lower_take_value_at_ptr_primitive_call(
    call: &CallExpr,
    destination: PointerTakeDestination,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(pointer_take_diagnostic(
            "`take_value_at_ptr` requires arguments `(pointer: *T, offset: usize)`",
        ));
    }
    let Some(_pointee_type) = context.function_call_type_substitution(call, "T") else {
        return Err(pointer_take_diagnostic(
            "`take_value_at_ptr` requires a concrete pointer element type",
        ));
    };
    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let offset = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(offset.instructions);

    let instruction = match destination {
        PointerTakeDestination::I32(destination) => Instruction::LoadI32FromPointer {
            destination,
            pointer,
            offset: offset.value,
        },
        PointerTakeDestination::U8(destination) => Instruction::LoadU8FromPointer {
            destination,
            pointer,
            offset: offset.value,
        },
        PointerTakeDestination::Usize(destination) => Instruction::LoadUsizeFromPointer {
            destination,
            pointer,
            offset: offset.value,
        },
        PointerTakeDestination::Bool(destination) => Instruction::LoadBoolFromPointer {
            destination,
            pointer,
            offset: offset.value,
        },
        PointerTakeDestination::Str(destination) => Instruction::LoadStrFromPointer {
            destination,
            pointer,
            offset: offset.value,
        },
        PointerTakeDestination::Aggregate { location, layout }
            if supported_aggregate_copy_layout(layout) =>
        {
            Instruction::CopyPointerToAggregate {
                destination: location,
                pointer,
                offset: offset.value,
                layout,
            }
        }
        PointerTakeDestination::Aggregate { .. } => {
            return Err(pointer_take_diagnostic(
                "`take_value_at_ptr` requires a non-empty aggregate layout",
            ));
        }
    };
    instructions.push(instruction);
    Ok(instructions)
}

fn pointer_take_diagnostic(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error("E8006", message.into())]
}

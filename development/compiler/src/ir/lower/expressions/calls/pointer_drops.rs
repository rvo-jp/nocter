use super::*;

pub(in crate::ir::lower::expressions) fn primitive_drop_value_at_ptr_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("drop_value_at_ptr")
    )
}

pub(in crate::ir::lower::expressions) fn lower_drop_value_at_ptr_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(pointer_drop_diagnostic(
            "`drop_value_at_ptr` requires arguments `(pointer: *T, offset: usize)`",
        ));
    }
    let Some(pointee_type) = context.function_call_type_substitution(call, "T") else {
        return Err(pointer_drop_diagnostic(
            "`drop_value_at_ptr` requires a concrete pointer element type",
        ));
    };

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let pointer = materialize_usize(pointer, &mut instructions, temporaries)?;
    let offset = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(offset.instructions);
    let offset = materialize_usize(offset.value, &mut instructions, temporaries)?;

    let Some(drop_kind) = context.aggregate_drop_for_type_expr(&pointee_type) else {
        return Ok(instructions);
    };
    if context.abi_value_for_type_expr(&pointee_type).is_none() {
        return Err(pointer_drop_diagnostic(
            "`drop_value_at_ptr` requires an element type with an ABI layout",
        ));
    }
    instructions.extend(lower_pointer_drop_kind(&drop_kind, pointer, offset, 0)?);
    Ok(instructions)
}

fn materialize_usize(
    value: UsizeValue,
    instructions: &mut Vec<Instruction>,
    temporaries: &mut TemporaryAllocator,
) -> Result<UsizeLocation, Vec<Diagnostic>> {
    if let UsizeValue::Location(location) = value {
        return Ok(location);
    }
    let destination = temporaries.next_usize()?;
    instructions.push(Instruction::SetUsize { destination, value });
    Ok(destination)
}

fn lower_pointer_drop_kind(
    drop_kind: &AggregateDrop,
    pointer: UsizeLocation,
    offset: UsizeLocation,
    field_offset: u32,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match drop_kind {
        AggregateDrop::Direct(drop_glue) => Ok(vec![pointer_drop_call(
            drop_glue.target.clone(),
            pointer,
            offset,
            field_offset,
        )]),
        AggregateDrop::Struct(drop_) => {
            let mut instructions = Vec::new();
            if let Some(direct) = &drop_.direct {
                instructions.push(pointer_drop_call(
                    direct.target.clone(),
                    pointer,
                    offset,
                    field_offset,
                ));
            }
            for field in drop_.fields.iter().rev() {
                let nested_offset = field_offset.checked_add(field.offset).ok_or_else(|| {
                    pointer_drop_diagnostic("pointer drop field offset overflows")
                })?;
                instructions.extend(lower_pointer_drop_kind(
                    field.drop_kind.as_ref(),
                    pointer,
                    offset,
                    nested_offset,
                )?);
            }
            Ok(instructions)
        }
        AggregateDrop::Array(drop_) => {
            let mut instructions = Vec::new();
            for index in (0..drop_.length).rev() {
                let nested_offset = index
                    .checked_mul(drop_.stride)
                    .and_then(|value| value.checked_add(u64::from(field_offset)))
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| {
                        pointer_drop_diagnostic("pointer drop array offset overflows")
                    })?;
                instructions.extend(lower_pointer_drop_kind(
                    drop_.element_drop_kind.as_ref(),
                    pointer,
                    offset,
                    nested_offset,
                )?);
            }
            Ok(instructions)
        }
        AggregateDrop::PayloadEnum(_) => Err(pointer_drop_diagnostic(
            "`drop_value_at_ptr` does not yet support payload enum elements",
        )),
        AggregateDrop::Outcome(_) => Err(pointer_drop_diagnostic(
            "`drop_value_at_ptr` does not support outcome elements",
        )),
    }
}

fn pointer_drop_call(
    target: CallTarget,
    pointer: UsizeLocation,
    offset: UsizeLocation,
    field_offset: u32,
) -> Instruction {
    Instruction::CallVoid {
        target,
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::PointerOffset {
                pointer,
                offset,
                field_offset,
            },
        })],
    }
}

fn pointer_drop_diagnostic(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error("E8006", message.into())]
}

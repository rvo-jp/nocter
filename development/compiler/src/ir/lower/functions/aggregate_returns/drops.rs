use super::*;

pub(in crate::ir::lower::functions) fn lower_direct_aggregate_drop_instruction(
    name: &str,
    slot_index: usize,
    layout: ValueLayout,
    drop_glue: &crate::ir::lower::context::DropGlue,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(&drop_glue.target) else {
        return Err(unsupported_drop_statement_diagnostic(name));
    };
    if parameter_types.len() != 1 || !drop_parameter_matches_local(&parameter_types[0], layout) {
        return Err(unsupported_drop_statement_diagnostic(name));
    }

    Ok(Instruction::CallVoid {
        target: drop_glue.target.clone(),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(slot_index),
        })],
    })
}

pub(in crate::ir::lower::functions) fn lower_payload_enum_drop_instructions(
    name: &str,
    slot_index: usize,
    drop_: &PayloadEnumDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let tag = temporaries.next_u8()?;
    let mut instructions = vec![Instruction::LoadAggregateU8 {
        destination: tag,
        source: AggregateLocation::Slot(slot_index),
        offset: 0,
    }];
    for variant in drop_.variants.iter().rev() {
        instructions.push(lower_payload_enum_drop_variant_if(
            name, slot_index, tag, variant, context,
        )?);
    }
    Ok(instructions)
}

pub(in crate::ir::lower::functions) fn lower_payload_enum_drop_variant_if(
    name: &str,
    slot_index: usize,
    tag: U8Location,
    variant: &PayloadEnumDropVariant,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let mut then_instructions = Vec::new();
    for field in variant.fields.iter().rev() {
        then_instructions.push(lower_payload_enum_drop_field(
            name, slot_index, field, context,
        )?);
    }

    Ok(Instruction::If {
        condition: BoolValue::I32Comparison {
            operator: I32ComparisonOperator::Equal,
            left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(tag))),
            right: I32Value::U8ZeroExtend(Box::new(U8Value::Const(variant.tag))),
        },
        then_instructions,
        else_instructions: Vec::new(),
    })
}

pub(in crate::ir::lower::functions) fn lower_payload_enum_drop_field(
    name: &str,
    slot_index: usize,
    field: &PayloadEnumDropField,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(&field.drop_glue.target) else {
        return Err(unsupported_drop_statement_diagnostic(name));
    };
    if parameter_types.len() != 1
        || !drop_parameter_matches_local(&parameter_types[0], field.payload_layout)
    {
        return Err(unsupported_drop_statement_diagnostic(name));
    }

    Ok(Instruction::CallVoid {
        target: field.drop_glue.target.clone(),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlotField {
                slot_index,
                offset: field.payload_offset,
            },
        })],
    })
}

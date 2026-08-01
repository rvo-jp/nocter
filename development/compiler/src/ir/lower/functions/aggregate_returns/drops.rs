use super::*;

pub(in crate::ir::lower) fn lower_aggregate_drop_instructions_at_location(
    name: &str,
    location: AggregateLocation,
    offset: u32,
    layout: ValueLayout,
    drop_kind: &AggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_drop_at_offset(name, location, offset, true, layout, drop_kind, context)
}

pub(in crate::ir::lower::functions) fn lower_aggregate_drop_instructions_at_root_location(
    name: &str,
    location: AggregateLocation,
    layout: ValueLayout,
    drop_kind: &AggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_drop_at_offset(name, location, 0, false, layout, drop_kind, context)
}

fn lower_direct_aggregate_drop_instruction_at_location(
    name: &str,
    location: AggregateLocation,
    offset: u32,
    is_field: bool,
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
            source: aggregate_borrow_source_at_offset(location, offset, is_field)
                .ok_or_else(|| unsupported_drop_statement_diagnostic(name))?,
        })],
    })
}

fn lower_struct_drop_instructions_at_location(
    name: &str,
    location: AggregateLocation,
    base_offset: u32,
    is_field: bool,
    layout: ValueLayout,
    drop_: &StructDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    if let Some(direct) = &drop_.direct {
        instructions.push(lower_direct_aggregate_drop_instruction_at_location(
            name,
            location,
            base_offset,
            is_field,
            layout,
            direct,
            context,
        )?);
    }
    for field in drop_.fields.iter().rev() {
        instructions.extend(lower_struct_drop_field(
            name,
            location,
            base_offset,
            field,
            context,
        )?);
    }
    Ok(instructions)
}

fn lower_array_drop_instructions_at_location(
    name: &str,
    location: AggregateLocation,
    base_offset: u32,
    drop_: &ArrayDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for index in (0..drop_.length).rev() {
        let offset = index
            .checked_mul(drop_.stride)
            .and_then(|offset| u64::from(base_offset).checked_add(offset))
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or_else(|| unsupported_drop_statement_diagnostic(name))?;
        instructions.extend(lower_aggregate_drop_at_offset(
            name,
            location,
            offset,
            true,
            drop_.element_layout,
            drop_.element_drop_kind.as_ref(),
            context,
        )?);
    }
    Ok(instructions)
}

fn lower_struct_drop_field(
    name: &str,
    location: AggregateLocation,
    base_offset: u32,
    field: &StructDropField,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let offset = base_offset
        .checked_add(field.offset)
        .ok_or_else(|| unsupported_drop_statement_diagnostic(name))?;
    lower_aggregate_drop_at_offset(
        name,
        location,
        offset,
        true,
        field.layout,
        field.drop_kind.as_ref(),
        context,
    )
}

fn lower_aggregate_drop_at_offset(
    name: &str,
    location: AggregateLocation,
    offset: u32,
    is_field: bool,
    layout: ValueLayout,
    drop_kind: &AggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match drop_kind {
        AggregateDrop::Direct(drop_glue) => {
            Ok(vec![lower_direct_aggregate_drop_instruction_at_location(
                name, location, offset, is_field, layout, drop_glue, context,
            )?])
        }
        AggregateDrop::Struct(drop_) => lower_struct_drop_instructions_at_location(
            name, location, offset, is_field, layout, drop_, context,
        ),
        AggregateDrop::Array(drop_) => {
            lower_array_drop_instructions_at_location(name, location, offset, drop_, context)
        }
        AggregateDrop::PayloadEnum(drop_) => {
            lower_payload_enum_drop_instructions_at_location(name, location, offset, drop_, context)
        }
    }
}

fn lower_payload_enum_drop_instructions_at_location(
    name: &str,
    location: AggregateLocation,
    base_offset: u32,
    drop_: &PayloadEnumDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let tag = temporaries.next_u8()?;
    let mut instructions = vec![Instruction::LoadAggregateU8 {
        destination: tag,
        source: location,
        offset: base_offset,
    }];
    for variant in drop_.variants.iter().rev() {
        instructions.push(lower_payload_enum_drop_variant_if(
            name,
            location,
            base_offset,
            tag,
            variant,
            context,
        )?);
    }
    Ok(instructions)
}

fn lower_payload_enum_drop_variant_if(
    name: &str,
    location: AggregateLocation,
    base_offset: u32,
    tag: U8Location,
    variant: &PayloadEnumDropVariant,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let mut then_instructions = Vec::new();
    for field in variant.fields.iter().rev() {
        then_instructions.extend(lower_payload_enum_drop_field(
            name,
            location,
            base_offset,
            field,
            context,
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

fn lower_payload_enum_drop_field(
    name: &str,
    location: AggregateLocation,
    base_offset: u32,
    field: &PayloadEnumDropField,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let offset = base_offset
        .checked_add(field.payload_offset)
        .ok_or_else(|| unsupported_drop_statement_diagnostic(name))?;
    lower_aggregate_drop_at_offset(
        name,
        location,
        offset,
        true,
        field.payload_layout,
        field.drop_kind.as_ref(),
        context,
    )
}

fn aggregate_borrow_source_at_offset(
    location: AggregateLocation,
    offset: u32,
    is_field: bool,
) -> Option<BorrowSource> {
    match (location, is_field) {
        (AggregateLocation::Slot(slot_index), false) => {
            Some(BorrowSource::AggregateSlot(slot_index))
        }
        (AggregateLocation::Slot(slot_index), true) => {
            Some(BorrowSource::AggregateSlotField { slot_index, offset })
        }
        (AggregateLocation::Parameter(parameter_index), false) => {
            Some(BorrowSource::AggregateParameter(parameter_index))
        }
        (AggregateLocation::Parameter(parameter_index), true) => {
            Some(BorrowSource::AggregateParameterField {
                parameter_index,
                offset,
            })
        }
        (AggregateLocation::Return, _)
        | (AggregateLocation::DirectReturn, _)
        | (AggregateLocation::DirectParameter { .. }, _) => None,
    }
}

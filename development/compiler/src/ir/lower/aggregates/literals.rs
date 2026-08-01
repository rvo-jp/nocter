use super::field_values::{
    lower_aggregate_struct_fields_to_location, validate_direct_aggregate_field_store,
};
use super::*;

pub(in crate::ir::lower) fn lower_aggregate_struct_literal_to_location(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_struct_literal_to_location_at_offset(
        literal,
        expected_layout,
        destination,
        0,
        diagnostic_code,
        subject,
        resolved,
        context,
    )
}

pub(in crate::ir::lower) fn lower_payload_enum_constructor_to_location(
    expression: &Expr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((member, arguments)) = payload_enum_constructor_member_and_arguments(expression)
    else {
        return Ok(None);
    };
    let actual_layout = layout_of(expected_type).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    if actual_layout != expected_layout {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let AbiType::Enum(enum_) = expected_type else {
        return Ok(None);
    };
    let Some(variant) = enum_
        .variants
        .iter()
        .find(|variant| variant.name == member.member)
    else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;

    let mut temporaries = TemporaryAllocator::new(context)?;
    let mut instructions = vec![Instruction::StoreAggregateU8 {
        destination,
        offset: 0,
        value: U8Value::Const(variant.tag),
    }];
    instructions.extend(lower_payload_enum_payload_to_location(
        variant.payload.as_ref(),
        arguments,
        destination,
        enum_.payload_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        &mut temporaries,
    )?);
    Ok(Some(instructions))
}

pub(in crate::ir::lower) fn payload_enum_constructor_member_and_arguments(
    expression: &Expr,
) -> Option<(&MemberExpr, &[Expr])> {
    match expression {
        Expr::Call(call) => {
            let Expr::Member(member) = call.callee.as_ref() else {
                return None;
            };
            Some((member, call.arguments.as_slice()))
        }
        Expr::Member(member) => Some((member, &[])),
        Expr::Group(group) => payload_enum_constructor_member_and_arguments(&group.expression),
        _ => None,
    }
}

fn lower_payload_enum_payload_to_location(
    payload_type: Option<&AbiType>,
    arguments: &[Expr],
    destination: AggregateLocation,
    payload_offset: u64,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(payload_type) = payload_type else {
        if arguments.is_empty() {
            return Ok(Vec::new());
        }
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };

    let base_offset = u32::try_from(payload_offset).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    if arguments.len() == 1 {
        return lower_aggregate_field_to_location(
            payload_type,
            &arguments[0],
            destination,
            base_offset,
            diagnostic_code,
            subject,
            resolved,
            context,
            temporaries,
        );
    }

    let AbiType::Struct(fields) = payload_type else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if fields.len() != arguments.len() {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let layout = layout_struct(fields).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    let mut instructions = Vec::new();
    for ((field, field_layout), argument) in fields
        .iter()
        .zip(layout.fields.iter())
        .zip(arguments.iter())
    {
        let field_offset = payload_offset
            .checked_add(field_layout.offset)
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        instructions.extend(lower_aggregate_field_to_location(
            &field.ty,
            argument,
            destination,
            field_offset,
            diagnostic_code,
            subject,
            resolved,
            context,
            temporaries,
        )?);
    }
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_aggregate_array_literal_to_location(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_aggregate_array_literal_to_location_at_offset_with_temporaries(
        literal,
        expected_type,
        expected_layout,
        destination,
        0,
        diagnostic_code,
        subject,
        resolved,
        context,
        &mut temporaries,
    )
}

pub(in crate::ir::lower) fn lower_aggregate_array_literal_to_location_at_offset(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_aggregate_array_literal_to_location_at_offset_with_temporaries(
        literal,
        expected_type,
        expected_layout,
        destination,
        base_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        &mut temporaries,
    )
}

pub(in crate::ir::lower) fn lower_aggregate_array_literal_to_location_with_temporaries(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_array_literal_to_location_at_offset_with_temporaries(
        literal,
        expected_type,
        expected_layout,
        destination,
        0,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
    )
}

pub(super) fn lower_aggregate_array_literal_to_location_at_offset_with_temporaries(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_array_literal_to_location_with_progress(
        literal,
        expected_type,
        expected_layout,
        destination,
        base_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
        None,
    )
}

pub(in crate::ir::lower) fn lower_aggregate_array_literal_to_location_with_progress(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    progress: Option<ArrayInitializationProgress>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let actual_layout = layout_of(expected_type).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    if actual_layout != expected_layout {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let AbiType::Array { element, length } = expected_type else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    if u64::try_from(literal.elements.len()).ok() != Some(*length) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let stride = array_element_stride(element).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    let mut instructions = Vec::new();
    for (index, element_expr) in literal.elements.iter().enumerate() {
        let element_offset = u64::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(stride))
            .and_then(|offset| u64::from(base_offset).checked_add(offset))
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        instructions.extend(lower_aggregate_field_to_location(
            element,
            element_expr,
            destination,
            element_offset,
            diagnostic_code,
            subject,
            resolved,
            context,
            temporaries,
        )?);
        if let Some(progress) = progress {
            let initialized_count = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or_else(|| {
                    unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                })?;
            instructions.push(progress.complete_element(initialized_count));
        }
    }
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_aggregate_struct_literal_to_location_with_temporaries(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
        literal,
        expected_layout,
        destination,
        0,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
        None,
    )
}

pub(in crate::ir::lower) fn lower_aggregate_struct_literal_to_location_with_progress(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    progress: &StructInitializationProgress,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
        literal,
        expected_layout,
        destination,
        0,
        diagnostic_code,
        subject,
        resolved,
        context,
        &mut temporaries,
        Some(progress),
    )
}

pub(in crate::ir::lower) fn lower_aggregate_struct_literal_to_location_at_offset(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
        literal,
        expected_layout,
        destination,
        base_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        &mut temporaries,
        None,
    )
}

fn lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
    literal: &StructLiteralExpr,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    progress: Option<&StructInitializationProgress>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let value = abi_value_from_type_expr(&literal.ty, resolved).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    if value.layout != expected_layout {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let AbiType::Struct(fields) = value.ty else {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    };
    lower_aggregate_struct_fields_to_location(
        &fields,
        literal,
        destination,
        base_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
        progress,
    )
}

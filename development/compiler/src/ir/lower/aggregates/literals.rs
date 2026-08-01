use super::field_values::{
    lower_aggregate_struct_fields_to_location, validate_direct_aggregate_field_store,
};
use super::*;
use crate::ir::lower::context::DropObligation;

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
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_payload_enum_constructor_to_location_with_progress(
        expression,
        expected_type,
        expected_layout,
        destination,
        diagnostic_code,
        subject,
        resolved,
        context,
        &mut temporaries,
        None,
    )
}

pub(in crate::ir::lower) fn lower_payload_enum_constructor_to_location_with_progress(
    expression: &Expr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    progress: Option<&PayloadInitializationProgress>,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    lower_payload_enum_constructor_to_location_at_offset_with_progress(
        expression,
        expected_type,
        expected_layout,
        destination,
        0,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
        progress,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::ir::lower) fn lower_payload_enum_constructor_to_location_at_offset_with_progress(
    expression: &Expr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    destination: AggregateLocation,
    base_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    progress: Option<&PayloadInitializationProgress>,
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
    if progress.is_some_and(|progress| progress.tag() != variant.tag) {
        return Err(unsupported_aggregate_struct_literal_diagnostic(
            diagnostic_code,
            subject,
        ));
    }
    validate_direct_aggregate_field_store(destination, diagnostic_code, subject)?;

    let mut instructions = vec![Instruction::StoreAggregateU8 {
        destination,
        offset: base_offset,
        value: U8Value::Const(variant.tag),
    }];
    instructions.extend(lower_payload_enum_payload_to_location(
        variant.payload.as_ref(),
        arguments,
        destination,
        base_offset,
        enum_.payload_offset,
        diagnostic_code,
        subject,
        resolved,
        context,
        temporaries,
        progress,
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
    base_offset: u32,
    payload_offset: u64,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    progress: Option<&PayloadInitializationProgress>,
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

    let payload_offset = u32::try_from(payload_offset).map_err(|_error| {
        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
    })?;
    if arguments.len() == 1 {
        let destination_offset = base_offset.checked_add(payload_offset).ok_or_else(|| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        return lower_payload_field_to_location(
            payload_type,
            &arguments[0],
            destination,
            payload_offset,
            destination_offset,
            diagnostic_code,
            subject,
            resolved,
            context,
            temporaries,
            progress,
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
        let progress_offset = u64::from(payload_offset)
            .checked_add(field_layout.offset)
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        let destination_offset = base_offset.checked_add(progress_offset).ok_or_else(|| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        instructions.extend(lower_payload_field_to_location(
            &field.ty,
            argument,
            destination,
            progress_offset,
            destination_offset,
            diagnostic_code,
            subject,
            resolved,
            context,
            temporaries,
            progress,
        )?);
    }
    Ok(instructions)
}

#[allow(clippy::too_many_arguments)]
fn lower_payload_field_to_location(
    field_type: &AbiType,
    expression: &Expr,
    destination: AggregateLocation,
    progress_offset: u32,
    destination_offset: u32,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    progress: Option<&PayloadInitializationProgress>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions =
        if let (Some(array_progress), AbiType::Array { .. }, Expr::ArrayLiteral(literal)) = (
            progress.and_then(|progress| progress.array_field_progress(progress_offset)),
            field_type,
            unwrap_aggregate_literal_group(expression),
        ) {
            let layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
            lower_aggregate_array_literal_to_location_with_progress(
                literal,
                field_type,
                layout,
                destination,
                destination_offset,
                diagnostic_code,
                subject,
                resolved,
                context,
                temporaries,
                Some(&array_progress),
            )?
        } else if let (Some(struct_progress), AbiType::Struct(_), Expr::StructLiteral(literal)) = (
            progress.and_then(|progress| progress.struct_field_progress(progress_offset)),
            field_type,
            unwrap_aggregate_literal_group(expression),
        ) {
            let layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
            lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
                literal,
                layout,
                destination,
                destination_offset,
                diagnostic_code,
                subject,
                resolved,
                context,
                temporaries,
                Some(&struct_progress),
            )?
        } else if let (Some(payload_progress), AbiType::Enum(_)) = (
            progress.and_then(|progress| progress.payload_field_progress(progress_offset)),
            field_type,
        ) {
            let layout = layout_of(field_type).map_err(|_error| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
            lower_payload_enum_constructor_to_location_at_offset_with_progress(
                expression,
                field_type,
                layout,
                destination,
                destination_offset,
                diagnostic_code,
                subject,
                resolved,
                context,
                temporaries,
                Some(&payload_progress),
            )?
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?
        } else {
            lower_aggregate_field_to_location(
                field_type,
                expression,
                destination,
                destination_offset,
                diagnostic_code,
                subject,
                resolved,
                context,
                temporaries,
            )?
        };
    if let Some(completed) = progress.and_then(|progress| progress.complete_field(progress_offset))
    {
        instructions.push(completed);
    }
    Ok(instructions)
}

fn unwrap_aggregate_literal_group(mut expression: &Expr) -> &Expr {
    while let Expr::Group(group) = expression {
        expression = &group.expression;
    }
    expression
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
    progress: Option<&ArrayInitializationProgress>,
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
        let element_index = u64::try_from(index).map_err(|_error| {
            unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
        })?;
        let element_offset = element_index
            .checked_mul(stride)
            .and_then(|offset| u64::from(base_offset).checked_add(offset))
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or_else(|| {
                unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
            })?;
        let partial = progress.and_then(|progress| progress.element_obligation(element_index));
        let lowered = match (
            partial,
            element.as_ref(),
            unwrap_aggregate_literal_group(element_expr),
        ) {
            (
                Some(DropObligation::ArrayPrefix {
                    initialized,
                    elements,
                }),
                AbiType::Array { .. },
                Expr::ArrayLiteral(literal),
            ) => {
                let nested_progress =
                    ArrayInitializationProgress::from_drop_state(*initialized, elements.clone());
                lower_aggregate_array_literal_to_location_with_progress(
                    literal,
                    element,
                    layout_of(element).map_err(|_error| {
                        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                    })?,
                    destination,
                    element_offset,
                    diagnostic_code,
                    subject,
                    resolved,
                    context,
                    temporaries,
                    Some(&nested_progress),
                )?
            }
            (
                Some(DropObligation::StructFields { fields }),
                AbiType::Struct(_),
                Expr::StructLiteral(literal),
            ) => {
                let nested_progress =
                    StructInitializationProgress::from_drop_states(fields.clone());
                lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
                    literal,
                    layout_of(element).map_err(|_error| {
                        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                    })?,
                    destination,
                    element_offset,
                    diagnostic_code,
                    subject,
                    resolved,
                    context,
                    temporaries,
                    Some(&nested_progress),
                )?
            }
            (Some(DropObligation::PayloadFields { tag, fields }), AbiType::Enum(_), _) => {
                let nested_progress =
                    PayloadInitializationProgress::from_drop_states(*tag, fields.clone());
                lower_payload_enum_constructor_to_location_at_offset_with_progress(
                    element_expr,
                    element,
                    layout_of(element).map_err(|_error| {
                        unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                    })?,
                    destination,
                    element_offset,
                    diagnostic_code,
                    subject,
                    resolved,
                    context,
                    temporaries,
                    Some(&nested_progress),
                )?
                .ok_or_else(|| {
                    unsupported_aggregate_struct_literal_diagnostic(diagnostic_code, subject)
                })?
            }
            _ => lower_aggregate_field_to_location(
                element,
                element_expr,
                destination,
                element_offset,
                diagnostic_code,
                subject,
                resolved,
                context,
                temporaries,
            )?,
        };
        instructions.extend(lowered);
        if let Some(progress) = progress {
            let initialized_count = element_index.checked_add(1).ok_or_else(|| {
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
        Some(&progress),
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

pub(in crate::ir::lower) fn lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
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

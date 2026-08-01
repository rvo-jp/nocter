use super::*;

pub(super) struct FixedArrayAccessMetadata {
    pub(super) instructions: Vec<Instruction>,
    pub(super) source: AggregateLocation,
    pub(super) base_offset: u32,
    pub(super) length: u64,
    pub(super) stride: u32,
    pub(super) element: AbiType,
    pub(super) is_readwrite: bool,
}

pub(super) fn fixed_array_access_metadata(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic> + Copy,
) -> Result<Option<FixedArrayAccessMetadata>, Vec<Diagnostic>> {
    match unwrap_group(&expression.object) {
        Expr::Identifier(identifier) => {
            let Some(local) = context.aggregate_local(&identifier.name) else {
                return Ok(None);
            };
            let Some(ty) = context.local_binding_type_expr_for_identifier(identifier) else {
                return Ok(None);
            };
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_diagnostic());
            };
            let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
                context.resolved_source(source)
            })
            .map_err(|_error| unsupported_diagnostic())?;
            let AbiType::Array { element, length } = &value.ty else {
                return Ok(None);
            };
            if value.layout != local.layout {
                return Err(unsupported_diagnostic());
            }
            let stride =
                array_element_stride(element).map_err(|_error| unsupported_diagnostic())?;
            let stride = u32::try_from(stride).map_err(|_error| unsupported_diagnostic())?;
            Ok(Some(FixedArrayAccessMetadata {
                instructions: Vec::new(),
                source: AggregateLocation::Slot(local.slot_index),
                base_offset: 0,
                length: *length,
                stride,
                element: element.as_ref().clone(),
                is_readwrite: true,
            }))
        }
        Expr::Member(_) => {
            let Some(access) =
                lower_aggregate_member_field_access(&expression.object, context, temporaries)?
            else {
                return Ok(None);
            };
            let AggregateFieldKind::Array {
                element,
                length,
                stride,
                ..
            } = access.kind
            else {
                return Ok(None);
            };
            Ok(Some(FixedArrayAccessMetadata {
                instructions: access.instructions,
                source: access.source,
                base_offset: access.offset,
                length,
                stride,
                element,
                is_readwrite: access.is_readwrite,
            }))
        }
        _ => Ok(None),
    }
}

pub(super) fn fixed_array_constant_index_value(expression: &Expr) -> Option<u128> {
    match unwrap_group(expression) {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value),
        _ => None,
    }
}

pub(super) fn lower_i32_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    if let Some(lowered) =
        lower_fixed_array_i32_index_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    if let Some(lowered) =
        lower_fixed_array_i32_indexed_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    lower_i32_slice_index_expression_to_value(expression, context, temporaries)
}

pub(super) fn lower_fixed_array_i32_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredI32Value>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_access(
        expression,
        context,
        temporaries,
        unsupported_i32_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::I32 {
        return Ok(None);
    }
    let mut instructions = access.instructions;
    if access.out_of_bounds {
        instructions.push(Instruction::Trap);
        return Ok(Some(LoweredI32Value {
            instructions,
            value: I32Value::Const(0),
        }));
    }

    let temporary = temporaries.next_i32()?;
    instructions.push(Instruction::LoadAggregateI32 {
        destination: temporary,
        source: access.source,
        offset: access.offset,
    });
    Ok(Some(LoweredI32Value {
        instructions,
        value: I32Value::Location(temporary),
    }))
}

pub(super) fn lower_fixed_array_i32_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredI32Value>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_indexed_access(
        expression,
        context,
        temporaries,
        unsupported_i32_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::I32 {
        return Ok(None);
    }

    let temporary = temporaries.next_i32()?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateI32Indexed {
        destination: temporary,
        source: access.source,
        base_offset: access.base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredI32Value {
        instructions,
        value: I32Value::Location(temporary),
    }))
}

pub(super) fn lower_u8_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredU8Value, Vec<Diagnostic>> {
    if let Some(lowered) =
        lower_fixed_array_u8_index_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    if let Some(lowered) =
        lower_fixed_array_u8_indexed_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }

    let source =
        lower_byte_collection_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;

    match source {
        LoweredByteCollectionValue::Str(source) => {
            let mut instructions = source.instructions;
            instructions.extend(index.instructions);
            let value = match source.value {
                StrValue::StaticBytes(bytes) => U8Value::StaticStrIndex {
                    bytes,
                    index: index.value,
                },
                StrValue::Location(source) => U8Value::StrIndex {
                    source,
                    index: index.value,
                },
                value @ (StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. }) => {
                    let source = temporaries.next_str()?;
                    instructions.push(Instruction::SetStr {
                        destination: source,
                        value,
                    });
                    U8Value::StrIndex {
                        source,
                        index: index.value,
                    }
                }
            };
            Ok(LoweredU8Value {
                instructions,
                value,
            })
        }
        LoweredByteCollectionValue::Slice(source) => {
            let mut instructions = source.instructions;
            let value = match source.value {
                SliceValue::Location(source) => U8Value::SliceIndex {
                    source,
                    index: index.value,
                },
                SliceValue::StrBytes(StrValue::StaticBytes(bytes)) => U8Value::StaticStrIndex {
                    bytes,
                    index: index.value,
                },
                SliceValue::StrBytes(StrValue::Location(source)) => U8Value::StrIndex {
                    source,
                    index: index.value,
                },
                SliceValue::StrBytes(
                    value @ (StrValue::ProcessArg { .. } | StrValue::SliceIndex { .. }),
                ) => {
                    let source = temporaries.next_str()?;
                    instructions.push(Instruction::SetStr {
                        destination: source,
                        value,
                    });
                    U8Value::StrIndex {
                        source,
                        index: index.value,
                    }
                }
            };
            instructions.extend(index.instructions);
            Ok(LoweredU8Value {
                instructions,
                value,
            })
        }
    }
}

pub(super) fn lower_fixed_array_u8_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredU8Value>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_access(
        expression,
        context,
        temporaries,
        unsupported_u8_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::U8 {
        return Ok(None);
    }
    let mut instructions = access.instructions;
    if access.out_of_bounds {
        instructions.push(Instruction::Trap);
        return Ok(Some(LoweredU8Value {
            instructions,
            value: U8Value::Const(0),
        }));
    }

    let temporary = temporaries.next_u8()?;
    instructions.push(Instruction::LoadAggregateU8 {
        destination: temporary,
        source: access.source,
        offset: access.offset,
    });
    Ok(Some(LoweredU8Value {
        instructions,
        value: U8Value::Location(temporary),
    }))
}

pub(super) fn lower_fixed_array_u8_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredU8Value>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_indexed_access(
        expression,
        context,
        temporaries,
        unsupported_u8_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::U8 {
        return Ok(None);
    }

    let temporary = temporaries.next_u8()?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateU8Indexed {
        destination: temporary,
        source: access.source,
        base_offset: access.base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredU8Value {
        instructions,
        value: U8Value::Location(temporary),
    }))
}

pub(super) fn lower_usize_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    if let Some(lowered) =
        lower_fixed_array_usize_index_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    if let Some(lowered) =
        lower_fixed_array_usize_indexed_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    lower_usize_slice_index_expression_to_value(expression, context, temporaries)
}

pub(super) fn lower_fixed_array_usize_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredUsizeValue>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_access(
        expression,
        context,
        temporaries,
        unsupported_usize_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::Usize {
        return Ok(None);
    }
    let mut instructions = access.instructions;
    if access.out_of_bounds {
        instructions.push(Instruction::Trap);
        return Ok(Some(LoweredUsizeValue {
            instructions,
            value: UsizeValue::Const(0),
        }));
    }

    let temporary = temporaries.next_usize()?;
    instructions.push(Instruction::LoadAggregateUsize {
        destination: temporary,
        source: access.source,
        offset: access.offset,
    });
    Ok(Some(LoweredUsizeValue {
        instructions,
        value: UsizeValue::Location(temporary),
    }))
}

pub(super) fn lower_fixed_array_usize_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredUsizeValue>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_indexed_access(
        expression,
        context,
        temporaries,
        unsupported_usize_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::Usize {
        return Ok(None);
    }

    let temporary = temporaries.next_usize()?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateUsizeIndexed {
        destination: temporary,
        source: access.source,
        base_offset: access.base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredUsizeValue {
        instructions,
        value: UsizeValue::Location(temporary),
    }))
}

pub(super) fn lower_usize_slice_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_usize_expression_diagnostic());
    };

    Ok(LoweredUsizeValue {
        instructions,
        value: UsizeValue::SliceIndex {
            source,
            index: Box::new(index.value),
        },
    })
}

pub(super) fn lower_i32_slice_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredI32Value, Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_i32_expression_diagnostic());
    };

    Ok(LoweredI32Value {
        instructions,
        value: I32Value::SliceIndex {
            source,
            index: index.value,
        },
    })
}

pub(super) fn lower_bool_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    if let Some(lowered) = lower_fixed_array_bool_index_expression_to_value(
        expression,
        context,
        diagnostic_code,
        temporaries,
    )? {
        return Ok(lowered);
    }
    if let Some(lowered) = lower_fixed_array_bool_indexed_expression_to_value(
        expression,
        context,
        diagnostic_code,
        temporaries,
    )? {
        return Ok(lowered);
    }
    lower_bool_slice_index_expression_to_value(expression, context, diagnostic_code, temporaries)
}

pub(super) fn lower_fixed_array_bool_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredBoolValue>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_access(expression, context, temporaries, || {
        unsupported_bool_expression_diagnostic(diagnostic_code)
    })?
    else {
        return Ok(None);
    };
    if access.element != AbiType::Bool {
        return Ok(None);
    }
    let mut instructions = access.instructions;
    if access.out_of_bounds {
        instructions.push(Instruction::Trap);
        return Ok(Some(LoweredBoolValue {
            instructions,
            value: BoolValue::Const(false),
        }));
    }

    let temporary = temporaries.next_bool()?;
    instructions.push(Instruction::LoadAggregateBool {
        destination: temporary,
        source: access.source,
        offset: access.offset,
    });
    Ok(Some(LoweredBoolValue {
        instructions,
        value: BoolValue::Location(temporary),
    }))
}

pub(super) fn lower_fixed_array_bool_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredBoolValue>, Vec<Diagnostic>> {
    let Some(access) =
        fixed_array_element_indexed_access(expression, context, temporaries, || {
            unsupported_bool_expression_diagnostic(diagnostic_code)
        })?
    else {
        return Ok(None);
    };
    if access.element != AbiType::Bool {
        return Ok(None);
    }

    let temporary = temporaries.next_bool()?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateBoolIndexed {
        destination: temporary,
        source: access.source,
        base_offset: access.base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredBoolValue {
        instructions,
        value: BoolValue::Location(temporary),
    }))
}

pub(super) fn lower_bool_slice_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_bool_expression_diagnostic(diagnostic_code));
    };

    Ok(LoweredBoolValue {
        instructions,
        value: BoolValue::SliceIndex {
            source,
            index: index.value,
        },
    })
}

pub(super) fn lower_str_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    if let Some(lowered) =
        lower_fixed_array_str_index_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    if let Some(lowered) =
        lower_fixed_array_str_indexed_expression_to_value(expression, context, temporaries)?
    {
        return Ok(lowered);
    }
    lower_str_slice_index_expression_to_value(expression, context, temporaries)
}

pub(super) fn lower_fixed_array_str_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredStrValue>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_access(
        expression,
        context,
        temporaries,
        unsupported_str_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::StrView {
        return Ok(None);
    }
    let mut instructions = access.instructions;
    if access.out_of_bounds {
        instructions.push(Instruction::Trap);
        return Ok(Some(LoweredStrValue {
            instructions,
            value: StrValue::StaticBytes(Vec::new()),
        }));
    }

    let temporary = temporaries.next_str()?;
    let StrLocation::Local(index) = temporary else {
        unreachable!("temporary str locations are local pairs");
    };
    let len_index = index
        .checked_add(1)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let len_offset = access
        .offset
        .checked_add(8)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    instructions.push(Instruction::LoadAggregateUsize {
        destination: UsizeLocation::Local(index),
        source: access.source,
        offset: access.offset,
    });
    instructions.push(Instruction::LoadAggregateUsize {
        destination: UsizeLocation::Local(len_index),
        source: access.source,
        offset: len_offset,
    });
    Ok(Some(LoweredStrValue {
        instructions,
        value: StrValue::Location(temporary),
    }))
}

pub(super) fn lower_fixed_array_str_indexed_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredStrValue>, Vec<Diagnostic>> {
    let Some(access) = fixed_array_element_indexed_access(
        expression,
        context,
        temporaries,
        unsupported_str_expression_diagnostic,
    )?
    else {
        return Ok(None);
    };
    if access.element != AbiType::StrView {
        return Ok(None);
    }

    let temporary = temporaries.next_str()?;
    let StrLocation::Local(index) = temporary else {
        unreachable!("temporary str locations are local pairs");
    };
    let len_index = index
        .checked_add(1)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let len_base_offset = access
        .base_offset
        .checked_add(8)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
    let mut instructions = access.index_instructions;
    instructions.push(Instruction::LoadAggregateUsizeIndexed {
        destination: UsizeLocation::Local(index),
        source: access.source,
        base_offset: access.base_offset,
        index: access.index.clone(),
        length: access.length,
        stride: access.stride,
    });
    instructions.push(Instruction::LoadAggregateUsizeIndexed {
        destination: UsizeLocation::Local(len_index),
        source: access.source,
        base_offset: len_base_offset,
        index: access.index,
        length: access.length,
        stride: access.stride,
    });
    Ok(Some(LoweredStrValue {
        instructions,
        value: StrValue::Location(temporary),
    }))
}

pub(super) fn lower_str_slice_index_expression_to_value(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_str_expression_diagnostic());
    };

    Ok(LoweredStrValue {
        instructions,
        value: StrValue::SliceIndex {
            source,
            index: index.value,
        },
    })
}

use super::*;

pub(super) enum ByteCollectionKind {
    Str,
    Slice,
}

pub(super) enum LoweredByteCollectionValue {
    Str(LoweredStrValue),
    Slice(LoweredSliceValue),
}

pub(super) fn lower_byte_collection_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredByteCollectionValue, Vec<Diagnostic>> {
    match byte_collection_expression_kind(expression, context) {
        Some(ByteCollectionKind::Str) => {
            lower_str_expression_to_value(expression, context, temporaries)
                .map(LoweredByteCollectionValue::Str)
        }
        Some(ByteCollectionKind::Slice) => {
            lower_slice_expression_to_value(expression, context, temporaries)
                .map(LoweredByteCollectionValue::Slice)
        }
        None => Err(unsupported_u8_expression_diagnostic()),
    }
}

pub(super) fn byte_collection_expression_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ByteCollectionKind> {
    match expression {
        Expr::StringLiteral(_) => Some(ByteCollectionKind::Str),
        Expr::Identifier(identifier) => {
            if context.str_location(&identifier.name).is_some() {
                Some(ByteCollectionKind::Str)
            } else if context.slice_location(&identifier.name).is_some() {
                Some(ByteCollectionKind::Slice)
            } else {
                None
            }
        }
        Expr::Call(call) => byte_collection_call_kind(call, context),
        Expr::Member(_) => match aggregate_member_field_kind(expression, context)
            .ok()
            .flatten()?
        {
            AggregateFieldKind::Str => Some(ByteCollectionKind::Str),
            AggregateFieldKind::Slice(_) => Some(ByteCollectionKind::Slice),
            _ => None,
        },
        Expr::Propagate(propagation) => {
            fallible_byte_collection_expression_kind(&propagation.expression, context)
        }
        Expr::Force(force) => fallible_byte_collection_expression_kind(&force.expression, context),
        Expr::Catch(catch) => fallible_byte_collection_expression_kind(&catch.expression, context),
        Expr::Group(group) => byte_collection_expression_kind(&group.expression, context),
        _ => None,
    }
}

pub(super) fn byte_collection_call_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<ByteCollectionKind> {
    if primitive_arg_raw_call(call, context) || primitive_env_entry_raw_call(call, context) {
        return Some(ByteCollectionKind::Str);
    }
    if primitive_str_from_raw_parts_call(call, context) {
        return Some(ByteCollectionKind::Str);
    }
    if primitive_bytes_from_str_call(call, context)
        || primitive_slice_from_raw_parts_call(call, context)
    {
        return Some(ByteCollectionKind::Slice);
    }

    let (target, _call_name) = context.direct_call_target_and_name(call)?;
    byte_collection_kind_from_type(context.call_return_type(&target)?)
}

pub(super) fn fallible_byte_collection_expression_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<ByteCollectionKind> {
    let Expr::Call(call) = unwrap_group(expression) else {
        return None;
    };
    let (target, _call_name) = context.direct_call_target_and_name(call)?;
    let Type::Fallible(success) = context.call_return_type(&target)? else {
        return None;
    };
    byte_collection_kind_from_type(success)
}

pub(super) fn byte_collection_kind_from_type(ty: &Type) -> Option<ByteCollectionKind> {
    match ty {
        Type::Str => Some(ByteCollectionKind::Str),
        Type::Slice { .. } => Some(ByteCollectionKind::Slice),
        _ => None,
    }
}

pub(super) fn lower_builtin_len_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Option<Result<LoweredUsizeValue, Vec<Diagnostic>>> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    if member.member != "len" || !call.arguments.is_empty() {
        return None;
    }
    if let Expr::Identifier(identifier) = member.object.as_ref()
        && let Some(pack) = context.literal_pack(&identifier.name)
    {
        return Some(Ok(LoweredUsizeValue {
            instructions: Vec::new(),
            value: UsizeValue::Const(pack.element_names.len() as u64),
        }));
    }
    byte_collection_expression_kind(&member.object, context)?;

    Some(lower_byte_collection_len_expression_to_value(
        &member.object,
        context,
        temporaries,
    ))
}

pub(super) fn lower_builtin_is_empty_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Option<Result<LoweredBoolValue, Vec<Diagnostic>>> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    if member.member != "is_empty" || !call.arguments.is_empty() {
        return None;
    }
    byte_collection_expression_kind(&member.object, context)?;

    Some(
        lower_byte_collection_len_expression_to_value(&member.object, context, temporaries).map(
            |source| LoweredBoolValue {
                instructions: source.instructions,
                value: BoolValue::UsizeComparison {
                    operator: I32ComparisonOperator::Equal,
                    left: source.value,
                    right: UsizeValue::Const(0),
                },
            },
        ),
    )
}

pub(super) fn lower_byte_collection_len_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    match lower_byte_collection_expression_to_value(expression, context, temporaries)? {
        LoweredByteCollectionValue::Str(source) => {
            let mut instructions = source.instructions;
            let value = match source.value {
                StrValue::StaticBytes(bytes) => UsizeValue::Const(bytes.len() as u64),
                StrValue::Location(location) => UsizeValue::StrLen(location),
                value @ (StrValue::ProcessArg { .. }
                | StrValue::ProcessEnvironmentName { .. }
                | StrValue::ProcessEnvironmentValue { .. }
                | StrValue::SliceIndex { .. }) => {
                    let temporary = temporaries.next_str()?;
                    instructions.push(Instruction::SetStr {
                        destination: temporary,
                        value,
                    });
                    UsizeValue::StrLen(temporary)
                }
            };
            Ok(LoweredUsizeValue {
                instructions,
                value,
            })
        }
        LoweredByteCollectionValue::Slice(source) => {
            let mut instructions = source.instructions;
            let value = match source.value {
                SliceValue::Location(location) => UsizeValue::SliceLen(location),
                SliceValue::StrBytes(StrValue::StaticBytes(bytes)) => {
                    UsizeValue::Const(bytes.len() as u64)
                }
                SliceValue::StrBytes(StrValue::Location(location)) => UsizeValue::StrLen(location),
                SliceValue::StrBytes(
                    value @ (StrValue::ProcessArg { .. }
                    | StrValue::ProcessEnvironmentName { .. }
                    | StrValue::ProcessEnvironmentValue { .. }
                    | StrValue::SliceIndex { .. }),
                ) => {
                    let temporary = temporaries.next_str()?;
                    instructions.push(Instruction::SetStr {
                        destination: temporary,
                        value,
                    });
                    UsizeValue::StrLen(temporary)
                }
            };
            Ok(LoweredUsizeValue {
                instructions,
                value,
            })
        }
    }
}

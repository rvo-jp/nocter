use super::*;

pub(super) fn lower_aggregate_i32_field_to_location(
    expression: &Expr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::I32)
        .ok_or_else(unsupported_i32_expression_diagnostic)?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateI32 {
        destination,
        source: access.source,
        offset: access.offset,
    });
    Ok(instructions)
}

pub(super) fn lower_aggregate_u8_field_to_location(
    expression: &Expr,
    destination: U8Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::U8)
        .ok_or_else(unsupported_u8_expression_diagnostic)?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateU8 {
        destination,
        source: access.source,
        offset: access.offset,
    });
    Ok(instructions)
}

pub(super) fn lower_aggregate_usize_field_to_location(
    expression: &Expr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::Usize)
        .ok_or_else(unsupported_usize_expression_diagnostic)?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateUsize {
        destination,
        source: access.source,
        offset: access.offset,
    });
    Ok(instructions)
}

pub(super) fn lower_aggregate_bool_field_to_location(
    expression: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::Bool)
        .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code))?;
    let mut instructions = access.instructions;
    instructions.push(Instruction::LoadAggregateBool {
        destination,
        source: access.source,
        offset: access.offset,
    });
    Ok(instructions)
}

pub(super) fn lower_aggregate_str_field_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| access.kind == AggregateFieldKind::Str)
        .ok_or_else(unsupported_str_expression_diagnostic)?;
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
    let mut instructions = access.instructions;
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
    Ok(LoweredStrValue {
        instructions,
        value: StrValue::Location(temporary),
    })
}

pub(super) fn lower_aggregate_slice_field_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(expression, context, temporaries)?
        .filter(|access| matches!(access.kind, AggregateFieldKind::Slice(_)))
        .ok_or_else(unsupported_slice_expression_diagnostic)?;
    let temporary = temporaries.next_slice()?;
    let SliceLocation::Local(index) = temporary else {
        unreachable!("temporary slice locations are local pairs");
    };
    let len_index = index
        .checked_add(1)
        .ok_or_else(unsupported_slice_expression_diagnostic)?;
    let len_offset = access
        .offset
        .checked_add(8)
        .ok_or_else(unsupported_slice_expression_diagnostic)?;
    let mut instructions = access.instructions;
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
    Ok(LoweredSliceValue {
        instructions,
        value: SliceValue::Location(temporary),
    })
}

pub(super) struct AggregateMemberAccess<'a> {
    pub(super) root: AggregateMemberRoot<'a>,
    pub(super) field_path: String,
}

pub(super) enum AggregateMemberRoot<'a> {
    Identifier(&'a str),
    Call(&'a CallExpr),
    FallibleCall(&'a CallExpr, FallibleFailureMode),
    OptionalCall(&'a crate::ast::OtherwiseExpr),
}

pub(super) fn aggregate_member_access<'a>(
    expression: &'a Expr,
    context: &LoweringContext,
) -> Result<Option<AggregateMemberAccess<'a>>, Vec<Diagnostic>> {
    let Expr::Member(member) = unwrap_group(expression) else {
        return Ok(None);
    };
    let Some((root, mut fields)) = aggregate_member_root_and_path(&member.object, context)? else {
        return Ok(None);
    };
    fields.push(member.member.as_str());
    Ok(Some(AggregateMemberAccess {
        root,
        field_path: fields.join("."),
    }))
}

pub(super) fn aggregate_member_root_and_path<'a>(
    expression: &'a Expr,
    context: &LoweringContext,
) -> Result<Option<(AggregateMemberRoot<'a>, Vec<&'a str>)>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Ok(Some((
            AggregateMemberRoot::Identifier(&identifier.name),
            Vec::new(),
        ))),
        Expr::Call(call) => Ok(Some((AggregateMemberRoot::Call(call), Vec::new()))),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberRoot::FallibleCall(
                    call,
                    propagating_outcome_mode(&propagation.expression, context)?,
                ),
                Vec::new(),
            )))
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberRoot::FallibleCall(call, FallibleFailureMode::Trap),
                Vec::new(),
            )))
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberRoot::FallibleCall(
                    call,
                    lower_catch_failure_mode(catch, context, 0)?,
                ),
                Vec::new(),
            )))
        }
        Expr::Otherwise(otherwise) => Ok(Some((
            AggregateMemberRoot::OptionalCall(otherwise),
            Vec::new(),
        ))),
        Expr::Member(member) => {
            let Some((root, mut fields)) = aggregate_member_root_and_path(&member.object, context)?
            else {
                return Ok(None);
            };
            fields.push(member.member.as_str());
            Ok(Some((root, fields)))
        }
        _ => Ok(None),
    }
}

pub(super) fn aggregate_member_field_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Option<AggregateFieldKind>, Vec<Diagnostic>> {
    let Expr::Member(member) = unwrap_group(expression) else {
        return Ok(None);
    };
    aggregate_member_field_kind_from_member(member, context)
}

pub(super) fn aggregate_call_member_field_kind(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<AggregateFieldKind> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        if !supported_aggregate_copy_layout(layout) {
            return None;
        }
        return aggregate_call_field(call, member_name, context).map(|field| field.kind);
    }

    let (target, _) = context.direct_call_target_and_name(call)?;
    let layout = aggregate_type_layout(context.call_return_type(&target)?)?;
    if !supported_aggregate_copy_layout(layout) {
        return None;
    }
    aggregate_call_field(call, member_name, context).map(|field| field.kind)
}

pub(super) fn aggregate_fallible_call_member_field_kind(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<AggregateFieldKind> {
    let (target, _) = context.direct_call_target_and_name(call)?;
    let Type::Fallible(success_type) = context.call_return_type(&target)? else {
        return None;
    };
    let layout = aggregate_type_layout(success_type.as_ref())?;
    if !supported_aggregate_copy_layout(layout) {
        return None;
    }
    aggregate_call_field(call, member_name, context).map(|field| field.kind)
}

pub(super) fn aggregate_optional_otherwise_member_field_kind(
    otherwise: &crate::ast::OtherwiseExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<AggregateFieldKind> {
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return None;
    };
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    if !return_type_expr_is_top_level_optional_with_resolver(&return_type, resolved, |source| {
        context.resolved_source(source)
    }) {
        return None;
    }
    let (target, _) = context.direct_call_target_and_name(call)?;
    let Type::Fallible(success_type) = context.call_return_type(&target)? else {
        return None;
    };
    let layout = aggregate_type_layout(success_type.as_ref())?;
    if !supported_aggregate_copy_layout(layout) {
        return None;
    }
    aggregate_call_field(call, member_name, context).map(|field| field.kind)
}

pub(super) fn lower_aggregate_call_member_field_access(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredAggregateFieldAccess>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        if !supported_aggregate_copy_layout(layout) {
            return Ok(None);
        }
        let Some(field) = aggregate_call_field(call, member_name, context) else {
            return Ok(None);
        };

        let slot_index = temporaries.next_aggregate_slot();
        let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
        let Some(mut syscall_instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            AggregateLocation::Slot(slot_index),
            layout,
            context,
            temporaries,
        )?
        else {
            return Ok(None);
        };
        instructions.append(&mut syscall_instructions);

        return Ok(Some(LoweredAggregateFieldAccess {
            instructions,
            source: AggregateLocation::Slot(slot_index),
            offset: field.offset,
            kind: field.kind,
            is_readwrite: false,
            is_copy: true,
        }));
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(layout) = aggregate_type_layout(&return_type) else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(layout) {
        return Ok(None);
    }
    let Some(field) = aggregate_call_field(call, member_name, context) else {
        return Ok(None);
    };

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.append(&mut argument_instructions);
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
    );

    Ok(Some(LoweredAggregateFieldAccess {
        instructions,
        source: AggregateLocation::Slot(slot_index),
        offset: field.offset,
        kind: field.kind,
        is_readwrite: false,
        is_copy: true,
    }))
}

pub(super) fn lower_aggregate_optional_otherwise_member_field_access(
    otherwise: &crate::ast::OtherwiseExpr,
    member_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredAggregateFieldAccess>, Vec<Diagnostic>> {
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Ok(None);
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return Ok(None);
    };
    let Some(expected_abi) =
        top_level_optional_success_abi_value_with_resolver(&return_type, resolved, |source| {
            context.resolved_source(source)
        })
    else {
        return Ok(None);
    };
    if !matches!(expected_abi.ty, AbiType::Struct(_) | AbiType::Array { .. })
        || !supported_aggregate_copy_layout(expected_abi.layout)
    {
        return Ok(None);
    }
    let Some(field) = aggregate_call_field(call, member_name, context) else {
        return Ok(None);
    };

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_abi.layout,
    }];
    instructions.extend(lower_aggregate_optional_otherwise_to_location(
        AggregateLocation::Slot(slot_index),
        0,
        expected_abi.layout,
        Some(&expected_abi.ty),
        otherwise,
        context,
        unsupported_aggregate_member_field_access_diagnostic,
    )?);

    Ok(Some(LoweredAggregateFieldAccess {
        instructions,
        source: AggregateLocation::Slot(slot_index),
        offset: field.offset,
        kind: field.kind,
        is_readwrite: false,
        is_copy: true,
    }))
}

pub(super) fn macos_syscall_primitive_call(call: &CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some(
            "syscall0"
                | "syscall1"
                | "syscall2"
                | "syscall3"
                | "syscall4"
                | "syscall5"
                | "syscall6"
        )
    )
}

pub(super) fn lower_aggregate_fallible_call_member_field_access(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Option<LoweredAggregateFieldAccess>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(layout) {
        return Ok(None);
    }
    let Some(field) = aggregate_call_field(call, member_name, context) else {
        return Ok(None);
    };

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );

    Ok(Some(LoweredAggregateFieldAccess {
        instructions,
        source: AggregateLocation::Slot(slot_index),
        offset: field.offset,
        kind: field.kind,
        is_readwrite: false,
        is_copy: true,
    }))
}

use super::*;

pub(super) fn lower_aggregate_struct_literal_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::StructLiteral(literal) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower aggregate struct literal bindings without resolved type information",
        ));
    };

    let value = abi_value_from_type_expr(&literal.ty, resolved).map_err(|_error| {
        unsupported_binding_diagnostic(
            "IR v0 can only lower local aggregate bindings whose initializer has an ABI layout",
        )
    })?;
    validate_aggregate_binding_layout(value.layout)?;

    let is_copy = type_expr_is_copy_struct(&literal.ty, resolved);
    let drop_kind = context.aggregate_drop_for_type_expr(&literal.ty);
    let fields =
        aggregate_fields_from_type_expr(&literal.ty, root_source, resolved).unwrap_or_default();
    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        value.layout,
        is_copy,
        drop_kind.clone(),
        fields,
    );
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    let progress = match (&value.ty, drop_kind.as_ref()) {
        (AbiType::Struct(fields), Some(drop_kind)) => Some(StructInitializationProgress::new(
            fields, literal, drop_kind, context,
        )?),
        _ => None,
    };
    if let Some(progress) = &progress {
        if !context
            .mark_aggregate_local_struct_fields(statement.name.as_str(), progress.drop_states())
        {
            return Err(unsupported_binding_diagnostic(
                "IR v0 cannot establish struct field initialization state",
            ));
        }
        instructions.extend(progress.initialize());
        instructions.extend(lower_aggregate_struct_literal_to_location_with_progress(
            literal,
            value.layout,
            AggregateLocation::Slot(slot_index),
            "E8008",
            "local bindings",
            resolved,
            context,
            progress,
        )?);
        context.mark_aggregate_local_initialized(statement.name.as_str());
    } else {
        instructions.extend(lower_aggregate_struct_literal_to_location(
            literal,
            value.layout,
            AggregateLocation::Slot(slot_index),
            "E8008",
            "local bindings",
            resolved,
            context,
        )?);
    }
    Ok(Some(instructions))
}

pub(super) fn lower_aggregate_array_literal_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::ArrayLiteral(literal) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower fixed array literal bindings without resolved type information",
        ));
    };
    let Some(ty) = context
        .binding_type_expr(statement.name_span)
        .or_else(|| statement.ty.clone())
    else {
        return Ok(None);
    };

    let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| {
        unsupported_binding_diagnostic(
            "IR v0 can only lower fixed array literal bindings whose type has an ABI layout",
        )
    })?;
    if !matches!(&value.ty, AbiType::Array { .. }) {
        return Ok(None);
    }

    let is_copy = type_expr_is_copy_aggregate_value_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    });
    let drop_kind = context.aggregate_drop_for_type_expr(&ty);
    let tracks_initialization = matches!(&drop_kind, Some(AggregateDrop::Array(_)))
        && array_literal_requires_runtime_progress(literal);
    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        value.layout,
        is_copy,
        drop_kind.clone(),
        Vec::new(),
    );
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    let progress = if tracks_initialization {
        let AbiType::Array { element, .. } = &value.ty else {
            return Ok(None);
        };
        let Some(drop_kind) = drop_kind.as_ref() else {
            return Ok(None);
        };
        let initialized = context.reserve_drop_state_usize_local()?;
        let progress = ArrayInitializationProgress::with_allocator(
            literal,
            element,
            drop_kind,
            initialized,
            context,
        )?;
        if !context.mark_aggregate_local_array_prefix(
            statement.name.as_str(),
            progress.location(),
            progress.element_states(),
        ) {
            return Err(unsupported_binding_diagnostic(
                "IR v0 cannot establish fixed array initialization state",
            ));
        }
        instructions.extend(progress.initialize());
        Some(progress)
    } else {
        None
    };
    let mut temporaries = TemporaryAllocator::new(context)?;
    instructions.extend(lower_aggregate_array_literal_to_location_with_progress(
        literal,
        &value.ty,
        value.layout,
        AggregateLocation::Slot(slot_index),
        0,
        "E8008",
        "local bindings",
        resolved,
        context,
        &mut temporaries,
        progress.as_ref(),
    )?);
    context.mark_aggregate_local_initialized(statement.name.as_str());
    Ok(Some(instructions))
}

pub(super) fn lower_payload_enum_constructor_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if payload_enum_constructor_member_and_arguments(&statement.initializer).is_none() {
        return Ok(None);
    }
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower payload enum bindings without resolved type information",
        ));
    };
    let Some(ty) = context
        .binding_type_expr(statement.name_span)
        .or_else(|| statement.ty.clone())
    else {
        return Ok(None);
    };
    let Ok(value) = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    }) else {
        return Ok(None);
    };
    if !matches!(value.ty, AbiType::Enum(_)) {
        return Ok(None);
    }

    let drop_kind = context.aggregate_drop_for_type_expr(&ty);
    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        value.layout,
        false,
        drop_kind.clone(),
        Vec::new(),
    );
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    let progress = match (&value.ty, drop_kind.as_ref()) {
        (AbiType::Enum(enum_), Some(drop_kind @ AggregateDrop::PayloadEnum(_))) => {
            Some(PayloadInitializationProgress::with_allocator(
                &statement.initializer,
                enum_,
                drop_kind,
                context,
            )?)
        }
        _ => None,
    };
    let mut temporaries = TemporaryAllocator::new(context)?;
    let constructor = if let Some(progress) = &progress {
        if !context.mark_aggregate_local_payload_fields(
            statement.name.as_str(),
            progress.tag(),
            progress.drop_states(),
        ) {
            return Err(unsupported_binding_diagnostic(
                "IR v0 cannot establish payload field initialization state",
            ));
        }
        instructions.extend(progress.initialize());
        lower_payload_enum_constructor_to_location_with_progress(
            &statement.initializer,
            &value.ty,
            value.layout,
            AggregateLocation::Slot(slot_index),
            "E8008",
            "local bindings",
            resolved,
            context,
            &mut temporaries,
            Some(progress),
        )?
    } else {
        lower_payload_enum_constructor_to_location(
            &statement.initializer,
            &value.ty,
            value.layout,
            AggregateLocation::Slot(slot_index),
            "E8008",
            "local bindings",
            resolved,
            context,
        )?
    };
    let Some(mut constructor_instructions) = constructor else {
        return Ok(None);
    };
    instructions.append(&mut constructor_instructions);
    context.mark_aggregate_local_initialized(statement.name.as_str());
    Ok(Some(instructions))
}

pub(super) fn lower_aggregate_call_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match unwrap_group(&statement.initializer) {
        Expr::Call(call) => lower_aggregate_normal_call_binding(statement, call, context),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Ok(None);
            };
            lower_aggregate_fallible_call_binding(
                statement,
                call,
                propagating_failure_mode(context)?,
                context,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Ok(None);
            };
            lower_aggregate_fallible_call_binding(
                statement,
                call,
                FallibleFailureMode::Trap,
                context,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Ok(None);
            };
            lower_aggregate_fallible_call_binding(
                statement,
                call,
                lower_catch_failure_mode(catch, context, 0)?,
                context,
            )
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_aggregate_normal_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        validate_aggregate_binding_layout(layout)?;
        let is_copy = call_success_type_is_copy_aggregate_value(call, context);
        let drop_kind = call_success_aggregate_drop(call, context);
        let fields = call_success_aggregate_fields(call, context);
        let slot_index = context.define_aggregate_local(
            statement.name.clone(),
            layout,
            is_copy,
            drop_kind,
            fields,
        );
        let mut temporaries = TemporaryAllocator::new(context)?;
        let Some(mut syscall_instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            AggregateLocation::Slot(slot_index),
            layout,
            context,
            &mut temporaries,
        )?
        else {
            return Ok(None);
        };
        let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
        instructions.append(&mut syscall_instructions);
        return Ok(Some(instructions));
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

    let is_copy = call_success_type_is_copy_aggregate_value(call, context);
    let drop_kind = call_success_aggregate_drop(call, context);
    let fields = call_success_aggregate_fields(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, drop_kind, fields);
    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    instructions.insert(0, Instruction::ReserveAggregateSlot { slot_index, layout });
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
    );
    Ok(Some(instructions))
}

pub(super) fn lower_aggregate_fallible_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    failure_mode: FallibleFailureMode,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };

    let Some(Type::Fallible(success)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(layout) = aggregate_type_layout(success.as_ref()) else {
        return Ok(None);
    };

    let is_copy = call_success_type_is_copy_aggregate_value(call, context);
    let drop_kind = call_success_aggregate_drop(call, context);
    let fields = call_success_aggregate_fields(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, drop_kind, fields);
    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    instructions.insert(0, Instruction::ReserveAggregateSlot { slot_index, layout });
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok(Some(instructions))
}

pub(super) fn lower_aggregate_copy_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some(source) = context.aggregate_local(&identifier.name) else {
        return Ok(None);
    };
    if !source.is_copy {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate copy bindings from copy aggregate locals",
        ));
    }
    let Some(fields) = context.aggregate_local_fields(&identifier.name) else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower aggregate copy bindings without aggregate field metadata",
        ));
    };

    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        source.layout,
        source.is_copy,
        source.drop_kind.clone(),
        fields,
    );
    Ok(Some(vec![
        Instruction::ReserveAggregateSlot {
            slot_index,
            layout: source.layout,
        },
        Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(slot_index),
            source: AggregateLocation::Slot(source.slot_index),
            layout: source.layout,
        },
    ]))
}

pub(super) fn lower_aggregate_move_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Unary(unary) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    if unary.operator != UnaryOperator::Move {
        return Ok(None);
    }
    let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate move bindings from `move name` initializers",
        ));
    };
    let Some(source) = context.aggregate_local(&identifier.name) else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(source.layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate move bindings for supported aggregate layouts",
        ));
    }
    let Some(fields) = context.aggregate_local_fields(&identifier.name) else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower aggregate move bindings without aggregate field metadata",
        ));
    };

    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        source.layout,
        source.is_copy,
        source.drop_kind.clone(),
        fields,
    );
    Ok(Some(vec![
        Instruction::ReserveAggregateSlot {
            slot_index,
            layout: source.layout,
        },
        Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(slot_index),
            source: AggregateLocation::Slot(source.slot_index),
            layout: source.layout,
        },
    ]))
}

pub(super) fn lower_aggregate_member_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Member(member) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };

    match aggregate_member_binding_path(member, context)? {
        Some((AggregateMemberBindingRoot::Identifier(identifier_name), field_path)) => {
            lower_aggregate_local_member_binding(statement, identifier_name, &field_path, context)
        }
        Some((AggregateMemberBindingRoot::Call(call), field_path)) => {
            lower_aggregate_call_member_binding(statement, call, &field_path, context)
        }
        Some((AggregateMemberBindingRoot::FallibleCall(call, failure_mode), field_path)) => {
            lower_aggregate_fallible_call_member_binding(
                statement,
                call,
                &field_path,
                failure_mode,
                context,
            )
        }
        Some((AggregateMemberBindingRoot::OptionalCall(otherwise), field_path)) => {
            lower_aggregate_optional_otherwise_member_binding(
                statement,
                otherwise,
                &field_path,
                context,
            )
        }
        None => Ok(None),
    }
}

pub(super) fn lower_aggregate_local_member_binding(
    statement: &BindingStmt,
    identifier_name: &str,
    field_path: &str,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(field) = context.aggregate_field(identifier_name, field_path) else {
        return Ok(None);
    };
    let source = field.source;
    let source_offset = field.offset;
    let is_copy = field.is_copy;
    let Some((layout, fields)) = field.kind.copy_aggregate_layout_and_fields() else {
        return Ok(None);
    };
    if !is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from copy aggregate fields",
        ));
    }

    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, None, fields);
    Ok(Some(vec![
        Instruction::ReserveAggregateSlot { slot_index, layout },
        Instruction::CopyAggregateRange {
            destination: AggregateLocation::Slot(slot_index),
            destination_offset: 0,
            source,
            source_offset,
            layout,
        },
    ]))
}

pub(super) fn lower_aggregate_call_member_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    field_path: &str,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(return_type) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(source_layout) = aggregate_type_layout(&return_type) else {
        return Ok(None);
    };
    let Some(field) = aggregate_call_field(call, field_path, context) else {
        return Ok(None);
    };
    let source_offset = field.offset;
    let is_copy = field.is_copy;
    let Some((layout, fields)) = field.kind.copy_aggregate_layout_and_fields() else {
        return Ok(None);
    };
    if !is_copy
        || !supported_aggregate_copy_layout(layout)
        || !supported_aggregate_copy_layout(source_layout)
    {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from copy aggregate fields",
        ));
    }

    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, None, fields);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![
        Instruction::ReserveAggregateSlot { slot_index, layout },
        Instruction::ReserveAggregateSlot {
            slot_index: source_slot,
            layout: source_layout,
        },
    ];
    let (mut argument_instructions, arguments) =
        lower_call_arguments_to_scalar_arguments_with_temporaries(
            call,
            &target,
            &call_name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        AggregateLocation::Slot(source_slot),
        target,
        arguments,
        source_layout,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(slot_index),
        destination_offset: 0,
        source: AggregateLocation::Slot(source_slot),
        source_offset,
        layout,
    });
    Ok(Some(instructions))
}

pub(super) fn lower_aggregate_fallible_call_member_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    field_path: &str,
    failure_mode: FallibleFailureMode,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(source_layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Ok(None);
    };
    let Some(field) = aggregate_call_field(call, field_path, context) else {
        return Ok(None);
    };
    let source_offset = field.offset;
    let is_copy = field.is_copy;
    let Some((layout, fields)) = field.kind.copy_aggregate_layout_and_fields() else {
        return Ok(None);
    };
    if !is_copy
        || !supported_aggregate_copy_layout(layout)
        || !supported_aggregate_copy_layout(source_layout)
    {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from copy fallible aggregate fields",
        ));
    }

    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, None, fields);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![
        Instruction::ReserveAggregateSlot { slot_index, layout },
        Instruction::ReserveAggregateSlot {
            slot_index: source_slot,
            layout: source_layout,
        },
    ];
    let (mut argument_instructions, arguments) =
        lower_call_arguments_to_scalar_arguments_with_temporaries(
            call,
            &target,
            &call_name,
            context,
            &mut temporaries,
        )?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
        AggregateLocation::Slot(source_slot),
        target,
        arguments,
        source_layout,
        failure_mode,
    );
    instructions.push(Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(slot_index),
        destination_offset: 0,
        source: AggregateLocation::Slot(source_slot),
        source_offset,
        layout,
    });
    Ok(Some(instructions))
}

pub(super) fn lower_aggregate_optional_otherwise_member_binding(
    statement: &BindingStmt,
    otherwise: &crate::ast::OtherwiseExpr,
    field_path: &str,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
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
    let Some(field) = aggregate_call_field(call, field_path, context) else {
        return Ok(None);
    };
    let source_offset = field.offset;
    let is_copy = field.is_copy;
    let Some((layout, fields)) = field.kind.copy_aggregate_layout_and_fields() else {
        return Ok(None);
    };
    if !is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate member bindings from copy optional aggregate fields",
        ));
    }

    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, None, fields);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let source_slot = temporaries.next_aggregate_slot();
    let mut instructions = vec![
        Instruction::ReserveAggregateSlot { slot_index, layout },
        Instruction::ReserveAggregateSlot {
            slot_index: source_slot,
            layout: expected_abi.layout,
        },
    ];
    instructions.extend(lower_aggregate_optional_otherwise_to_location(
        AggregateLocation::Slot(source_slot),
        0,
        expected_abi.layout,
        Some(&expected_abi.ty),
        otherwise,
        context,
        || {
            unsupported_binding_diagnostic(
                "IR v0 can only lower aggregate member bindings from copy optional aggregate fields",
            )
        },
    )?);
    instructions.push(Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(slot_index),
        destination_offset: 0,
        source: AggregateLocation::Slot(source_slot),
        source_offset,
        layout,
    });
    Ok(Some(instructions))
}

pub(super) fn lower_aggregate_slice_index_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Index(index) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Some(element) = copy_aggregate_slice_index_element(index, context) else {
        return Ok(None);
    };

    let drop_kind = element.drop_glue.map(AggregateDrop::Direct);
    let slot_index = context.define_aggregate_local(
        statement.name.clone(),
        element.layout,
        true,
        drop_kind,
        element.fields,
    );
    let mut temporaries = TemporaryAllocator::new(context)?;
    let lowered_slice = lower_slice_expression_to_value(&index.object, context, &mut temporaries)?;
    let SliceValue::Location(source) = lowered_slice.value else {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower aggregate slice index bindings from slice locations",
        ));
    };
    let (index_instructions, element_index) =
        lower_usize_expression_to_word_with_temporaries(&index.index, context, &mut temporaries)?;

    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: element.layout,
    }];
    instructions.extend(lowered_slice.instructions);
    instructions.extend(index_instructions);
    let element_index =
        materialize_slice_aggregate_index(&mut instructions, element_index, &mut temporaries)?;
    instructions.push(Instruction::CopySliceElementToAggregate {
        destination: AggregateLocation::Slot(slot_index),
        source,
        index: element_index,
        layout: element.layout,
    });
    Ok(Some(instructions))
}

pub(super) enum AggregateMemberBindingRoot<'a> {
    Identifier(&'a str),
    Call(&'a CallExpr),
    FallibleCall(&'a CallExpr, FallibleFailureMode),
    OptionalCall(&'a crate::ast::OtherwiseExpr),
}

pub(super) struct CopyAggregateSliceElement {
    pub(super) layout: ValueLayout,
    pub(super) fields: Vec<crate::ir::lower::context::AggregateField>,
    pub(super) drop_glue: Option<DropGlue>,
}

pub(super) fn copy_aggregate_slice_index_element(
    index: &IndexExpr,
    context: &LoweringContext,
) -> Option<CopyAggregateSliceElement> {
    let element_ty = slice_index_element_type_expr(index, context)?;
    let (root_source, resolved) = context.resolved_calls()?;
    if !type_expr_is_copy_struct_with_resolver(&element_ty, resolved, |source| {
        context.resolved_source(source)
    }) {
        return None;
    }

    let value = abi_value_from_type_expr_with_resolver(&element_ty, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;
    if !matches!(value.ty, AbiType::Struct(_)) || !supported_aggregate_copy_layout(value.layout) {
        return None;
    }
    let fields = aggregate_fields_from_type_expr_with_resolver(
        &element_ty,
        root_source,
        resolved,
        |source| context.resolved_source(source),
    )?;
    Some(CopyAggregateSliceElement {
        layout: value.layout,
        fields,
        drop_glue: context.drop_glue_for_type_expr(&element_ty),
    })
}

pub(super) fn aggregate_member_binding_path<'a>(
    member: &'a MemberExpr,
    context: &LoweringContext,
) -> Result<Option<(AggregateMemberBindingRoot<'a>, String)>, Vec<Diagnostic>> {
    let Some((root, mut fields)) = aggregate_member_binding_root_and_path(&member.object, context)?
    else {
        return Ok(None);
    };
    fields.push(member.member.as_str());
    Ok(Some((root, fields.join("."))))
}

pub(super) fn aggregate_member_binding_root_and_path<'a>(
    expression: &'a Expr,
    context: &LoweringContext,
) -> Result<Option<(AggregateMemberBindingRoot<'a>, Vec<&'a str>)>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Ok(Some((
            AggregateMemberBindingRoot::Identifier(&identifier.name),
            Vec::new(),
        ))),
        Expr::Call(call) => Ok(Some((AggregateMemberBindingRoot::Call(call), Vec::new()))),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberBindingRoot::FallibleCall(call, propagating_failure_mode(context)?),
                Vec::new(),
            )))
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberBindingRoot::FallibleCall(call, FallibleFailureMode::Trap),
                Vec::new(),
            )))
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Ok(None);
            };
            Ok(Some((
                AggregateMemberBindingRoot::FallibleCall(
                    call,
                    lower_catch_failure_mode(catch, context, 0)?,
                ),
                Vec::new(),
            )))
        }
        Expr::Otherwise(otherwise) => Ok(Some((
            AggregateMemberBindingRoot::OptionalCall(otherwise),
            Vec::new(),
        ))),
        Expr::Member(member) => {
            let Some((root, mut fields)) =
                aggregate_member_binding_root_and_path(&member.object, context)?
            else {
                return Ok(None);
            };
            fields.push(member.member.as_str());
            Ok(Some((root, fields)))
        }
        _ => Ok(None),
    }
}

pub(super) fn validate_aggregate_binding_layout(
    layout: crate::abi::ValueLayout,
) -> Result<(), Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower this aggregate binding with a non-empty ABI layout",
        ));
    }
    Ok(())
}

pub(super) fn call_success_type_is_copy_aggregate_value(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return false;
    };
    let Some(return_type) = context.call_value_type_expr(call) else {
        return false;
    };
    type_expr_is_copy_aggregate_value_with_resolver(&return_type, resolved, |source| {
        context.resolved_source(source)
    })
}

pub(super) fn call_success_aggregate_fields(
    call: &CallExpr,
    context: &LoweringContext,
) -> Vec<crate::ir::lower::context::AggregateField> {
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Vec::new();
    };
    let Some(return_type) = context.call_value_type_expr(call) else {
        return Vec::new();
    };
    aggregate_fields_from_type_expr(&return_type, root_source, resolved).unwrap_or_default()
}

pub(super) fn call_success_aggregate_drop(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<crate::ir::lower::context::AggregateDrop> {
    let return_type = context.call_value_type_expr(call)?;
    context.aggregate_drop_for_type_expr(&return_type)
}

use super::super::{
    LoweredUsizeValue, lower_byte_collection_len_expression_to_value,
    lower_byte_collection_pointer_expression_to_value,
};
use super::*;

pub(in crate::ir::lower) fn lower_macos_syscall_primitive_call_to_location(
    call: &CallExpr,
    destination: AggregateLocation,
    expected_layout: ValueLayout,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(arity) = macos_syscall_arity(call, context) else {
        return Ok(None);
    };
    if call.arguments.len() != arity + 1 {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "native lowering can only lower primitive `syscall{arity}` with {} `usize` arguments",
                arity + 1
            ),
        )]);
    }

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_macos_syscall_diagnostic(
            "missing resolved primitive signature",
        ));
    };
    let Some(signature) = resolved.call_signature_for_call(call) else {
        return Err(unsupported_macos_syscall_diagnostic(
            "missing resolved call signature",
        ));
    };
    let value =
        abi_value_from_type_expr_with_resolver(&signature.return_type, resolved, |source| {
            context.resolved_source(source)
        })
        .map_err(|_error| unsupported_macos_syscall_diagnostic("invalid return ABI layout"))?;
    if value.layout != expected_layout {
        return Err(unsupported_macos_syscall_diagnostic(
            "return layout does not match the destination aggregate",
        ));
    }

    let mut instructions = Vec::new();
    let mut words = Vec::with_capacity(call.arguments.len());
    for argument in &call.arguments {
        let lowered = lower_usize_expression_to_value(argument, context, temporaries)?;
        instructions.extend(lowered.instructions);
        words.push(lowered.value);
    }
    let mut words = words.into_iter();
    let number = words
        .next()
        .ok_or_else(|| unsupported_macos_syscall_diagnostic("missing syscall number argument"))?;
    let arguments = words.collect::<Vec<_>>();
    instructions.push(Instruction::DarwinSyscall {
        destination,
        arity: u8::try_from(arity).expect("macOS syscall arity fits in u8"),
        number,
        arguments,
    });
    Ok(Some(instructions))
}

pub(in crate::ir::lower) fn primitive_trap_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("trap" | "unreachable")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_exit_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("exit_raw" | "allocation_abort_raw")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_write_text_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("write_text_raw")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_open_read_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("open_read_raw" | "create_raw" | "append_raw")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_write_bytes_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("write_bytes_raw")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_read_bytes_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("read_bytes_raw")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_close_fd_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(context.primitive_name_for_call(call), Some("close_fd_raw"))
}

pub(in crate::ir::lower::expressions) fn primitive_bytes_from_str_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("bytes_from_str")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_view_len_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("str_len_raw" | "slice_len_raw")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_view_pointer_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("str_ptr_addr_raw" | "slice_ptr_addr_raw")
    )
}

pub(in crate::ir::lower::expressions) fn lower_view_pointer_primitive_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "view pointer primitives require exactly one view argument",
        ));
    }
    lower_byte_collection_pointer_expression_to_value(&call.arguments[0], context, temporaries)
}

pub(in crate::ir::lower::expressions) fn lower_view_len_primitive_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredUsizeValue, Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "view length primitives require exactly one view argument",
        ));
    }
    if let Expr::Identifier(identifier) = &call.arguments[0]
        && let Some(pack) = context.literal_pack(&identifier.name)
    {
        let fixed = pack
            .segments
            .iter()
            .filter(|segment| {
                matches!(
                    segment,
                    super::super::super::context::LiteralPackLoweringSegment::Value { .. }
                )
            })
            .count() as u64;
        let value = match &pack.runtime_length_name {
            Some(name) => context
                .usize_location(name)
                .map(UsizeValue::Location)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E8014",
                        "literal pack cached length is unavailable",
                    )]
                })?,
            None => UsizeValue::Const(fixed),
        };
        return Ok(LoweredUsizeValue {
            instructions: Vec::new(),
            value,
        });
    }
    lower_byte_collection_len_expression_to_value(&call.arguments[0], context, temporaries)
}

pub(in crate::ir::lower::expressions) fn primitive_addr_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(context.primitive_name_for_call(call), Some("addr"))
}

pub(in crate::ir::lower::expressions) fn primitive_from_ref_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("from_ref" | "from_ref_mut")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_pointee_layout_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("pointee_size" | "pointee_align")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_arg_count_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(context.primitive_name_for_call(call), Some("arg_count_raw"))
}

pub(in crate::ir::lower::expressions) fn primitive_env_count_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(context.primitive_name_for_call(call), Some("env_count_raw"))
}

pub(in crate::ir::lower::expressions) fn primitive_current_allocation_state_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("current_allocator_state")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_current_allocation_kind_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("current_allocator_kind")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_arg_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(context.primitive_name_for_call(call), Some("arg_raw"))
}

pub(in crate::ir::lower::expressions) fn primitive_env_entry_raw_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("env_name_raw" | "env_value_raw")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_copy_str_to_ptr_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("copy_str_to_ptr")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_copy_ptr_to_ptr_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("copy_ptr_to_ptr")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_store_u8_to_ptr_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("store_u8_to_ptr")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_store_value_to_ptr_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("store_value_to_ptr")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_str_from_raw_parts_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("str_from_raw_parts")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_str_subview_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some("str_subview_unchecked")
    )
}

pub(in crate::ir::lower::expressions) fn primitive_slice_from_raw_parts_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some(
            "slice_from_raw_parts"
                | "slice_from_raw_parts_mut"
                | "slice_from_raw_parts_value"
                | "slice_from_raw_parts_value_mut"
        )
    )
}

pub(in crate::ir::lower::expressions) fn lower_addr_primitive_call_to_word(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`addr` requires one pointer argument",
        ));
    }
    lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)
}

pub(in crate::ir::lower::expressions) fn lower_addr_primitive_call_to_location(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`addr` requires one pointer argument",
        ));
    }
    if let Expr::Call(pointer_call) = unwrap_group(&call.arguments[0])
        && primitive_from_ref_call(pointer_call, context)
    {
        return lower_from_ref_primitive_call_to_location(
            pointer_call,
            destination,
            context,
            temporaries,
        );
    }

    let (mut instructions, value) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    instructions.push(Instruction::SetUsize { destination, value });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_from_ref_primitive_call_to_location(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (mut instructions, source) =
        lower_from_ref_primitive_call_to_borrow_source(call, context, temporaries)?;
    instructions.push(Instruction::SetUsizeFromBorrow {
        destination,
        source,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_from_ref_primitive_call_to_word(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    let destination = temporaries.next_usize()?;
    let instructions =
        lower_from_ref_primitive_call_to_location(call, destination, context, temporaries)?;
    Ok((instructions, UsizeValue::Location(destination)))
}

fn lower_from_ref_primitive_call_to_borrow_source(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowSource), Vec<Diagnostic>> {
    let Some(primitive_name) = context.primitive_name_for_call(call) else {
        return Err(unsupported_pointer_primitive_diagnostic(
            "borrow-to-pointer conversion requires a pointer primitive",
        ));
    };
    let is_readwrite = match primitive_name {
        "from_ref" => false,
        "from_ref_mut" => true,
        _ => {
            return Err(unsupported_pointer_primitive_diagnostic(
                "borrow-to-pointer conversion requires `from_ref` or `from_ref_mut`",
            ));
        }
    };
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(format!(
            "`{primitive_name}` requires one borrow argument"
        )));
    }
    let Some(pointee_type) = context.function_call_type_substitution(call, "T") else {
        return Err(unsupported_pointer_primitive_diagnostic(format!(
            "`{primitive_name}` requires a concrete pointer element type"
        )));
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_pointer_primitive_diagnostic(format!(
            "`{primitive_name}` requires resolved type information"
        )));
    };
    let Some(inner) = borrow_inner_type_with_resolver(&pointee_type, resolved, |source| {
        context.resolved_source(source)
    }) else {
        return Err(unsupported_pointer_primitive_diagnostic(format!(
            "`{primitive_name}` requires a borrowable pointer element type"
        )));
    };
    let parameter_type = Type::Borrow {
        is_readwrite,
        inner: Box::new(inner),
    };
    let (instructions, argument) = lower_borrow_argument(
        &call.arguments[0],
        &parameter_type,
        primitive_name,
        context,
        temporaries,
    )?;
    Ok((instructions, argument.source))
}

pub(in crate::ir::lower::expressions) fn lower_pointee_layout_primitive_call_to_word(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    let primitive_name = context
        .primitive_name_for_call(call)
        .unwrap_or("pointee layout primitive");
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(format!(
            "`{primitive_name}` requires one pointer argument"
        )));
    }
    let (instructions, _pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let Some(pointee_type) = context.function_call_type_substitution(call, "T") else {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`{primitive_name}` requires a concrete pointer element type",
        ));
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`{primitive_name}` requires resolved type information",
        ));
    };
    let value = abi_value_from_type_expr_with_resolver(&pointee_type, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| {
        unsupported_pointer_primitive_diagnostic(
            "`{primitive_name}` requires a pointer element type with an ABI layout",
        )
    })?;
    let result = match primitive_name {
        "pointee_align" => value.layout.align,
        _ => value.layout.size,
    };
    Ok((instructions, UsizeValue::Const(result)))
}

pub(in crate::ir::lower::expressions) fn lower_arg_count_raw_primitive_call_to_word(
    call: &CallExpr,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    if !call.arguments.is_empty() {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`arg_count_raw` requires no arguments",
        ));
    }
    Ok((Vec::new(), UsizeValue::ProcessArgCount))
}

pub(in crate::ir::lower::expressions) fn lower_env_count_raw_primitive_call_to_word(
    call: &CallExpr,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    if !call.arguments.is_empty() {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`env_count_raw` requires no arguments",
        ));
    }
    Ok((Vec::new(), UsizeValue::ProcessEnvironmentCount))
}

pub(in crate::ir::lower::expressions) fn lower_arg_raw_primitive_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, StrValue), Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`arg_raw` requires one `usize` index argument",
        ));
    }

    let index = lower_usize_expression_to_value(&call.arguments[0], context, temporaries)?;
    Ok((
        index.instructions,
        StrValue::ProcessArg { index: index.value },
    ))
}

pub(in crate::ir::lower::expressions) fn lower_env_entry_raw_primitive_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, StrValue), Vec<Diagnostic>> {
    let primitive_name = context.primitive_name_for_call(call).ok_or_else(|| {
        unsupported_pointer_primitive_diagnostic("expected environment primitive")
    })?;
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(format!(
            "`{primitive_name}` requires one `usize` index argument"
        )));
    }

    let index = lower_usize_expression_to_value(&call.arguments[0], context, temporaries)?;
    let value = match primitive_name {
        "env_name_raw" => StrValue::ProcessEnvironmentName { index: index.value },
        "env_value_raw" => StrValue::ProcessEnvironmentValue { index: index.value },
        _ => {
            return Err(unsupported_pointer_primitive_diagnostic(
                "expected indexed environment primitive",
            ));
        }
    };
    Ok((index.instructions, value))
}

pub(in crate::ir::lower::expressions) fn lower_copy_str_to_ptr_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 3 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`copy_str_to_ptr` requires arguments `(destination: *u8, offset: usize, text: &str)`",
        ));
    }

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let offset = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(offset.instructions);
    let text = lower_str_expression_to_value(&call.arguments[2], context, temporaries)?;
    instructions.extend(text.instructions);
    instructions.push(Instruction::CopyStrToPointer {
        pointer,
        offset: offset.value,
        text: text.value,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_copy_ptr_to_ptr_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 3 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`copy_ptr_to_ptr` requires arguments `(destination: *u8, source: *u8, byte_count: usize)`",
        ));
    }

    let (mut instructions, destination) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let (source_instructions, source) =
        lower_pointer_address_expression_to_word(&call.arguments[1], context, temporaries)?;
    instructions.extend(source_instructions);
    let byte_count = lower_usize_expression_to_value(&call.arguments[2], context, temporaries)?;
    instructions.extend(byte_count.instructions);
    instructions.push(Instruction::CopyPointerBytes {
        destination,
        source,
        byte_count: byte_count.value,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_store_u8_to_ptr_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 3 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`store_u8_to_ptr` requires arguments `(destination: *u8, offset: usize, value: u8)`",
        ));
    }

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let offset = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(offset.instructions);
    let value = lower_u8_expression_to_value(&call.arguments[2], context, temporaries)?;
    instructions.extend(value.instructions);
    instructions.push(Instruction::StoreU8ToPointer {
        pointer,
        offset: offset.value,
        value: value.value,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_store_value_to_ptr_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 3 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`store_value_to_ptr` requires arguments `(destination: *T, offset: usize, value: T)`",
        ));
    }

    let Some(pointee_type) = context.function_call_type_substitution(call, "T") else {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`store_value_to_ptr` requires a concrete pointer element type",
        ));
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`store_value_to_ptr` requires resolved type information",
        ));
    };

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let offset = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(offset.instructions);
    let stored_value = unwrap_pointer_store_move(&call.arguments[2]);

    if let Some(value_type) = context
        .ir_type_for_type_expr(&pointee_type)
        .filter(|ty| matches!(ty, Type::Borrow { .. }))
        .or_else(|| {
            scalar_or_view_type_from_type_expr_with_resolver(&pointee_type, resolved, |source| {
                context.resolved_source(source)
            })
        })
    {
        match value_type {
            Type::U8 => {
                let value = lower_u8_expression_to_value(stored_value, context, temporaries)?;
                instructions.extend(value.instructions);
                instructions.push(Instruction::StoreU8ToPointer {
                    pointer,
                    offset: offset.value,
                    value: value.value,
                });
            }
            Type::I32 => {
                let value = lower_i32_expression_to_value(stored_value, context, temporaries)?;
                instructions.extend(value.instructions);
                instructions.push(Instruction::StoreI32ToPointer {
                    pointer,
                    offset: offset.value,
                    value: value.value,
                });
            }
            Type::Usize => {
                let value = lower_usize_expression_to_value(stored_value, context, temporaries)?;
                instructions.extend(value.instructions);
                instructions.push(Instruction::StoreUsizeToPointer {
                    pointer,
                    offset: offset.value,
                    value: value.value,
                });
            }
            Type::Bool => {
                let value = lower_bool_expression_to_value_with_temporaries(
                    stored_value,
                    context,
                    "E8006",
                    temporaries,
                )?;
                instructions.extend(value.instructions);
                instructions.push(Instruction::StoreBoolToPointer {
                    pointer,
                    offset: offset.value,
                    value: value.value,
                });
            }
            Type::Str => {
                let value = lower_str_expression_to_value(stored_value, context, temporaries)?;
                instructions.extend(value.instructions);
                instructions.push(Instruction::StoreStrToPointer {
                    pointer,
                    offset: offset.value,
                    value: value.value,
                });
            }
            Type::Borrow { .. } => {
                let (borrow_instructions, value) = lower_borrow_argument(
                    stored_value,
                    &value_type,
                    "store_value_to_ptr",
                    context,
                    temporaries,
                )?;
                instructions.extend(borrow_instructions);
                let destination = temporaries.next_usize()?;
                instructions.push(Instruction::SetUsizeFromBorrow {
                    destination,
                    source: value.source,
                });
                instructions.push(Instruction::StoreUsizeToPointer {
                    pointer,
                    offset: offset.value,
                    value: UsizeValue::Location(destination),
                });
            }
            Type::Slice { .. }
            | Type::Aggregate { .. }
            | Type::DirectAggregate { .. }
            | Type::Error
            | Type::Void
            | Type::Never
            | Type::Optional(_)
            | Type::Fallible(_)
            | Type::ComposedOutcome { .. } => {
                return Err(unsupported_pointer_primitive_diagnostic(
                    "`store_value_to_ptr` supports only scalar, borrow, and `&str` element types",
                ));
            }
        }
        return Ok(instructions);
    }

    let value = abi_value_from_type_expr_with_resolver(&pointee_type, resolved, |source| {
        context.resolved_source(source)
    });
    if let Ok(value) = value
        && matches!(
            value.ty,
            AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_) | AbiType::Outcome { .. }
        )
        && supported_aggregate_copy_layout(value.layout)
    {
        let (value_instructions, source) = lower_aggregate_store_value_source(
            &call.arguments[2],
            value.layout,
            context,
            temporaries,
            resolved,
        )?;
        instructions.extend(value_instructions);
        instructions.push(Instruction::CopyAggregateToPointer {
            pointer,
            offset: offset.value,
            source,
            layout: value.layout,
        });
        return Ok(instructions);
    }

    Err(unsupported_pointer_primitive_diagnostic(
        "`store_value_to_ptr` supports only scalar, string-view, and aggregate element types",
    ))
}

fn unwrap_pointer_store_move(expression: &Expr) -> &Expr {
    match unwrap_group(expression) {
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => &unary.operand,
        expression => expression,
    }
}

fn lower_aggregate_store_value_source(
    expression: &Expr,
    layout: ValueLayout,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    resolved: &crate::resolve::ResolveOutput,
) -> Result<(Vec<Instruction>, AggregateLocation), Vec<Diagnostic>> {
    match expression {
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            lower_aggregate_store_value_source(
                &unary.operand,
                layout,
                context,
                temporaries,
                resolved,
            )
        }
        Expr::Identifier(identifier) => {
            let source = context
                .aggregate_local(&identifier.name)
                .map(|local| (local.layout, AggregateLocation::Slot(local.slot_index)))
                .or_else(|| {
                    context.outcome_local(&identifier.name).and_then(|local| {
                        local.is_live.then_some((
                            local.storage.layout,
                            AggregateLocation::Slot(local.slot_index),
                        ))
                    })
                });
            let Some((source_layout, source)) = source else {
                return Err(unsupported_pointer_primitive_diagnostic(
                    "`store_value_to_ptr` requires an aggregate source value",
                ));
            };
            if source_layout != layout {
                return Err(unsupported_pointer_primitive_diagnostic(
                    "`store_value_to_ptr` aggregate source layout does not match pointer element layout",
                ));
            }
            Ok((Vec::new(), source))
        }
        Expr::StructLiteral(literal) => {
            let slot_index = temporaries.next_aggregate_slot();
            let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
            instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
                literal,
                layout,
                AggregateLocation::Slot(slot_index),
                "E8006",
                "pointer stores",
                resolved,
                context,
                temporaries,
            )?);
            Ok((instructions, AggregateLocation::Slot(slot_index)))
        }
        Expr::Group(group) => lower_aggregate_store_value_source(
            &group.expression,
            layout,
            context,
            temporaries,
            resolved,
        ),
        _ => Err(unsupported_pointer_primitive_diagnostic(
            "`store_value_to_ptr` supports aggregate locals and struct literals",
        )),
    }
}

pub(in crate::ir::lower::expressions) fn lower_str_from_raw_parts_primitive_call_to_location(
    call: &CallExpr,
    destination: StrLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`str_from_raw_parts` requires arguments `(pointer: *u8, len: usize)`",
        ));
    }

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let len = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(len.instructions);
    instructions.push(Instruction::SetStrRawParts {
        destination,
        pointer,
        len: len.value,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_str_subview_primitive_call_to_location(
    call: &CallExpr,
    destination: StrLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 3 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`str_subview_unchecked` requires arguments `(text: &str, start: usize, len: usize)`",
        ));
    }

    let source = lower_str_expression_to_value(&call.arguments[0], context, temporaries)?;
    let start = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    let len = lower_usize_expression_to_value(&call.arguments[2], context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(start.instructions);
    instructions.extend(len.instructions);
    instructions.push(Instruction::SetStrSubview {
        destination,
        source: source.value,
        start: start.value,
        len: len.value,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_slice_from_raw_parts_primitive_call_to_location(
    call: &CallExpr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`slice_from_raw_parts` requires arguments `(pointer: *T, len: usize)`",
        ));
    }

    let (mut instructions, pointer) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let len = lower_usize_expression_to_value(&call.arguments[1], context, temporaries)?;
    instructions.extend(len.instructions);
    instructions.push(Instruction::SetSliceRawParts {
        destination,
        pointer,
        len: len.value,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_str_bytes_primitive_call_to_location(
    call: &CallExpr,
    destination: SliceLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (mut instructions, value) =
        lower_str_bytes_primitive_call_to_value(call, context, temporaries)?;
    instructions.push(Instruction::SetSlice { destination, value });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_str_bytes_primitive_call_to_value(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, SliceValue), Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(unsupported_pointer_primitive_diagnostic(
            "`bytes_from_str` requires argument `(value: &str)`",
        ));
    }

    let text = lower_str_expression_to_value(&call.arguments[0], context, temporaries)?;
    Ok((text.instructions, SliceValue::StrBytes(text.value)))
}

pub(in crate::ir::lower::expressions) fn lower_close_fd_raw_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "native lowering can only lower primitive `close_fd_raw` with argument `(i32)`",
        )]);
    }

    let fd = lower_i32_expression_to_value(&call.arguments[0], context, temporaries)?;
    let mut instructions = fd.instructions;
    instructions.push(Instruction::CloseFd { fd: fd.value });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_exit_raw_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if context.primitive_name_for_call(call) == Some("allocation_abort_raw") {
        if !call.arguments.is_empty() {
            return Err(vec![Diagnostic::error(
                "E8006",
                "native lowering can only lower primitive `allocation_abort_raw` without arguments",
            )]);
        }
        return Ok(vec![Instruction::ProcessExit {
            code: I32Value::Const(70),
        }]);
    }
    if call.arguments.len() != 1 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "native lowering can only lower primitive `exit_raw` with argument `(i32)`",
        )]);
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let code = lower_i32_expression_to_value(&call.arguments[0], context, &mut temporaries)?;
    let mut instructions = code.instructions;
    instructions.push(Instruction::ProcessExit { code: code.value });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_read_bytes_raw_primitive_call(
    call: &CallExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "native lowering can only lower primitive `read_bytes_raw` with arguments `(i32, &+[u8])`",
        )]);
    };

    let fd = lower_i32_expression_to_value(&call.arguments[0], context, temporaries)?;
    let buffer = lower_slice_expression_to_value(&call.arguments[1], context, temporaries)?;
    let mut instructions = fd.instructions;
    instructions.extend(buffer.instructions);
    instructions.push(Instruction::ReadSlice {
        destination,
        fd: fd.value,
        buffer: buffer.value,
        failure_mode,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_open_read_raw_primitive_call(
    call: &CallExpr,
    destination: I32Location,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: OutcomeFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 1 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "native lowering can only lower primitive `open_read_raw` with argument `(*u8)`",
        )]);
    }

    let (mut instructions, path) =
        lower_pointer_address_expression_to_word(&call.arguments[0], context, temporaries)?;
    let (flags, mode) = match context.primitive_name_for_call(call) {
        Some("create_raw") => (1 + 512 + 1024, 438),
        Some("append_raw") => (1 + 8 + 512, 438),
        _ => (0, 0),
    };
    instructions.push(Instruction::OpenRead {
        destination,
        path,
        flags: UsizeValue::Const(flags),
        mode: UsizeValue::Const(mode),
        failure_mode,
    });
    Ok(instructions)
}

fn macos_syscall_arity(call: &CallExpr, context: &LoweringContext) -> Option<usize> {
    match context.primitive_name_for_call(call)? {
        "syscall0" => Some(0),
        "syscall1" => Some(1),
        "syscall2" => Some(2),
        "syscall3" => Some(3),
        "syscall4" => Some(4),
        "syscall5" => Some(5),
        "syscall6" => Some(6),
        _ => None,
    }
}

fn unsupported_macos_syscall_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!("native lowering cannot lower macOS syscall primitive: {reason}"),
    )]
}

pub(in crate::ir::lower) fn lower_pointer_address_expression_to_word(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, UsizeValue), Vec<Diagnostic>> {
    match expression {
        Expr::Call(call)
            if context.primitive_name_for_call(call) == Some("from_addr")
                && call.arguments.len() == 1 =>
        {
            let address =
                lower_usize_expression_to_value(&call.arguments[0], context, temporaries)?;
            Ok((address.instructions, address.value))
        }
        Expr::Call(call) if primitive_from_ref_call(call, context) => {
            lower_from_ref_primitive_call_to_word(call, context, temporaries)
        }
        Expr::Call(call) => {
            let destination = temporaries.next_usize()?;
            let instructions = lower_usize_normal_call(call, destination, context, temporaries)?;
            Ok((instructions, UsizeValue::Location(destination)))
        }
        Expr::Identifier(identifier) => context
            .usize_location(&identifier.name)
            .map(|location| (Vec::new(), UsizeValue::Location(location)))
            .ok_or_else(|| {
                unsupported_pointer_primitive_diagnostic("pointer argument must be a pointer value")
            }),
        Expr::Member(_) => {
            let access = lower_aggregate_member_field_access(expression, context, temporaries)?
                .filter(|access| access.kind == AggregateFieldKind::Usize)
                .ok_or_else(|| {
                    unsupported_pointer_primitive_diagnostic(
                        "pointer argument must be a pointer aggregate field",
                    )
                })?;
            let destination = temporaries.next_usize()?;
            let mut instructions = access.instructions;
            instructions.push(Instruction::LoadAggregateUsize {
                destination,
                source: access.source,
                offset: access.offset,
            });
            Ok((instructions, UsizeValue::Location(destination)))
        }
        Expr::Group(group) => {
            lower_pointer_address_expression_to_word(&group.expression, context, temporaries)
        }
        _ => Err(unsupported_pointer_primitive_diagnostic(
            "pointer argument must come from a pointer value, pointer-returning call, `from_addr(...)`, `from_ref(...)`, `from_ref_mut(...)`, or a pointer aggregate field",
        )),
    }
}

fn unsupported_pointer_primitive_diagnostic(reason: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "native lowering cannot lower pointer primitive call: {}",
            reason.into()
        ),
    )]
}

pub(in crate::ir::lower::expressions) fn lower_write_text_raw_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "native lowering can only lower primitive `write_text_raw` with arguments `(i32, &str)`",
        )]);
    };

    let fd = lower_i32_expression_to_value(&call.arguments[0], context, temporaries)?;
    let text = lower_str_expression_to_value(&call.arguments[1], context, temporaries)?;
    let mut instructions = fd.instructions;
    instructions.extend(text.instructions);
    instructions.push(Instruction::WriteStr {
        fd: fd.value,
        text: text.value,
    });
    Ok(instructions)
}

pub(in crate::ir::lower::expressions) fn lower_write_bytes_raw_primitive_call(
    call: &CallExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if call.arguments.len() != 2 {
        return Err(vec![Diagnostic::error(
            "E8006",
            "native lowering can only lower primitive `write_bytes_raw` with arguments `(i32, &[u8])`",
        )]);
    };

    let fd = lower_i32_expression_to_value(&call.arguments[0], context, temporaries)?;
    let bytes = lower_slice_expression_to_value(&call.arguments[1], context, temporaries)?;
    let mut instructions = fd.instructions;
    instructions.extend(bytes.instructions);
    instructions.push(Instruction::WriteSlice {
        fd: fd.value,
        bytes: bytes.value,
    });
    Ok(instructions)
}

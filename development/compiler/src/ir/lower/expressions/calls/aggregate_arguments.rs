use super::*;

pub(super) fn lower_tracked_closure_argument_source(
    argument: &Expr,
    parameter_type: &Type,
    parameter_type_expr: Option<&TypeExpr>,
    _callee_name: &str,
    evaluation: &mut CallEvaluationContext<'_, '_>,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<(Vec<Instruction>, AggregateArgumentSource)>, Vec<Diagnostic>> {
    let Expr::Closure(closure) = unwrap_group(argument) else {
        return Ok(None);
    };
    let Some(parameter_type_expr @ TypeExpr::Closure(_)) = parameter_type_expr else {
        return Ok(None);
    };
    let expected_layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    let slot_index = temporaries.next_aggregate_slot();
    evaluation.sync_temporaries(temporaries)?;
    let instructions = crate::ir::lower::closures::lower_closure_to_slot(
        closure,
        parameter_type_expr,
        slot_index,
        evaluation.context(),
        temporaries,
    )?;
    let actual_layout = evaluation
        .context()
        .abi_value_for_type_expr(parameter_type_expr)
        .map(|value| value.layout);
    if actual_layout != Some(expected_layout) {
        return Err(vec![Diagnostic::error(
            "E8015",
            "closure argument layout does not match its specialized callable parameter",
        )]);
    }
    Ok(Some((
        instructions,
        AggregateArgumentSource::Slot(slot_index),
    )))
}

pub(super) fn lower_tracked_spread_argument_source(
    argument: &Expr,
    parameter_type: &Type,
    parameter_type_expr: Option<&TypeExpr>,
    _callee_name: &str,
    evaluation: &mut CallEvaluationContext<'_, '_>,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<(Vec<Instruction>, AggregateArgumentSource)>, Vec<Diagnostic>> {
    let Some(spread) = crate::typecheck::sequence_spread(argument) else {
        return Ok(None);
    };
    let Some(plan) = evaluation.context().sequence_spread_plan(spread.span) else {
        return Err(spread_argument_diagnostic("semantic plan is unavailable"));
    };
    let Some(parameter_type_expr) = parameter_type_expr else {
        return Err(spread_argument_diagnostic(
            "iterator parameter type is unavailable",
        ));
    };
    let expected_layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    let drop_kind = evaluation
        .context()
        .aggregate_drop_for_type_expr(parameter_type_expr);
    let slot_index = temporaries.next_aggregate_slot();
    evaluation.sync_temporaries(temporaries)?;
    if let Some(drop_kind) = drop_kind.clone()
        && !evaluation.complete_temporary(slot_index, expected_layout, drop_kind)
    {
        return Err(spread_argument_diagnostic(
            "iterator temporary drop state conflicts",
        ));
    }
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    match plan.source_mode {
        crate::typecheck::TypecheckCollectionForSourceMode::Direct => {
            let Some((_root_source, resolved)) = evaluation.context().resolved_calls() else {
                return Err(spread_argument_diagnostic(
                    "resolution facts are unavailable",
                ));
            };
            let iterator_ir = evaluation
                .context()
                .ir_type_for_type_expr(&plan.iterator_type)
                .ok_or_else(|| {
                    spread_argument_diagnostic("direct iterator IR type is unavailable")
                })?;
            instructions.extend(lower_aggregate_return_expression_to_location(
                &spread.operand,
                &iterator_ir,
                AggregateLocation::Slot(slot_index),
                evaluation.context().function_name(),
                resolved,
                evaluation.context(),
            )?);
        }
        crate::typecheck::TypecheckCollectionForSourceMode::ReadonlyConversion
        | crate::typecheck::TypecheckCollectionForSourceMode::OwnedConversion => {
            let conversion = plan
                .conversion
                .as_ref()
                .ok_or_else(|| spread_argument_diagnostic("conversion plan is unavailable"))?;
            let target = evaluation
                .context()
                .iteration_method_target(conversion)
                .ok_or_else(|| spread_argument_diagnostic("conversion target is unavailable"))?;
            let return_type = evaluation
                .context()
                .call_return_type(&target)
                .cloned()
                .ok_or_else(|| spread_argument_diagnostic("conversion ABI is unavailable"))?;
            let receiver = match plan.mode {
                crate::typecheck::TypecheckSequenceSpreadMode::Copy => Expr::Borrow(BorrowExpr {
                    span: spread.operand.span(),
                    operator_span: spread.operator_span,
                    is_readwrite: false,
                    expression: spread.operand.clone(),
                }),
                crate::typecheck::TypecheckSequenceSpreadMode::Readonly
                | crate::typecheck::TypecheckSequenceSpreadMode::Move => (*spread.operand).clone(),
            };
            let call = crate::ir::lower::collection_for::synthetic_call(
                spread.span,
                &conversion.target_name,
                vec![receiver],
            );
            let parameter_types = vec![crate::ir::lower::collection_for::receiver_parameter_type(
                conversion,
                spread.span,
            )];
            let (argument_instructions, arguments) = lower_call_arguments_with_explicit_types(
                &call,
                &target,
                &conversion.target_name,
                evaluation.context(),
                temporaries,
                Some(&parameter_types),
            )?;
            instructions.extend(argument_instructions);
            push_aggregate_call_instruction(
                &mut instructions,
                &return_type,
                AggregateLocation::Slot(slot_index),
                target,
                arguments,
                expected_layout,
            );
        }
    }
    Ok(Some((
        instructions,
        AggregateArgumentSource::Slot(slot_index),
    )))
}

fn spread_argument_diagnostic(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8014",
        format!("IR cannot prepare sequence spread: {detail}"),
    )]
}

pub(super) fn lower_tracked_interpolation_argument_source(
    argument: &Expr,
    parameter_type: &Type,
    parameter_type_expr: Option<&TypeExpr>,
    callee_name: &str,
    evaluation: &mut CallEvaluationContext<'_, '_>,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<(Vec<Instruction>, AggregateArgumentSource)>, Vec<Diagnostic>> {
    let Expr::InterpolatedString(interpolated) = unwrap_group(argument) else {
        return Ok(None);
    };
    let expected_layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    let Some(parameter_type_expr) = parameter_type_expr else {
        return Ok(None);
    };
    let Some(drop_kind) = evaluation
        .context()
        .aggregate_drop_for_type_expr(parameter_type_expr)
    else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let slot_index = temporaries.next_aggregate_slot();
    evaluation.sync_temporaries(temporaries)?;
    if !evaluation.complete_temporary(slot_index, expected_layout, drop_kind) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(
        crate::ir::lower::interpolation::lower_interpolated_string_to_slot(
            interpolated,
            slot_index,
            evaluation.context(),
        )?,
    );
    Ok(Some((
        instructions,
        AggregateArgumentSource::Slot(slot_index),
    )))
}

pub(super) fn lower_tracked_payload_argument_source(
    argument: &Expr,
    parameter_type: &Type,
    parameter_type_expr: Option<&TypeExpr>,
    callee_name: &str,
    evaluation: &mut CallEvaluationContext<'_, '_>,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<(Vec<Instruction>, AggregateArgumentSource)>, Vec<Diagnostic>> {
    if payload_enum_constructor_member_and_arguments(argument).is_none() {
        return Ok(None);
    }
    let Some(parameter_type_expr) = parameter_type_expr else {
        return Ok(None);
    };
    let expected_layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    let Some(drop_kind @ AggregateDrop::PayloadEnum(_)) = evaluation
        .context()
        .aggregate_drop_for_type_expr(parameter_type_expr)
    else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = evaluation.context().resolved_calls() else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let value = abi_value_from_type_expr_with_resolver(parameter_type_expr, resolved, |source| {
        evaluation.context().resolved_source(source)
    })
    .map_err(|_error| unsupported_aggregate_argument_diagnostic(callee_name, parameter_type))?;
    let AbiType::Enum(enum_) = &value.ty else {
        return Ok(None);
    };
    if value.layout != expected_layout {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let slot_index = temporaries.next_aggregate_slot();
    let progress =
        PayloadInitializationProgress::with_allocator(argument, enum_, &drop_kind, temporaries)?;
    evaluation.sync_temporaries(temporaries)?;
    if !evaluation.register_payload_fields(
        slot_index,
        expected_layout,
        drop_kind.clone(),
        progress.tag(),
        progress.drop_states(),
    ) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(progress.initialize());
    let Some(mut constructor) = lower_payload_enum_constructor_to_location_with_progress(
        argument,
        &value.ty,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8006",
        &format!("arguments for function `{callee_name}`"),
        resolved,
        evaluation.context(),
        temporaries,
        Some(&progress),
    )?
    else {
        return Ok(None);
    };
    instructions.append(&mut constructor);
    if !evaluation.complete_temporary(slot_index, expected_layout, drop_kind) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    Ok(Some((
        instructions,
        AggregateArgumentSource::Slot(slot_index),
    )))
}

pub(super) fn lower_tracked_struct_argument_source(
    argument: &Expr,
    parameter_type: &Type,
    parameter_type_expr: Option<&TypeExpr>,
    callee_name: &str,
    evaluation: &mut CallEvaluationContext<'_, '_>,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<(Vec<Instruction>, AggregateArgumentSource)>, Vec<Diagnostic>> {
    let Expr::StructLiteral(literal) = unwrap_group(argument) else {
        return Ok(None);
    };
    let Some(parameter_type_expr) = parameter_type_expr else {
        return Ok(None);
    };
    let expected_layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    let Some(drop_kind @ (AggregateDrop::Direct(_) | AggregateDrop::Struct(_))) = evaluation
        .context()
        .aggregate_drop_for_type_expr(parameter_type_expr)
    else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = evaluation.context().resolved_calls() else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let value = abi_value_from_type_expr_with_resolver(parameter_type_expr, resolved, |source| {
        evaluation.context().resolved_source(source)
    })
    .map_err(|_error| unsupported_aggregate_argument_diagnostic(callee_name, parameter_type))?;
    let AbiType::Struct(fields) = &value.ty else {
        return Ok(None);
    };
    if value.layout != expected_layout {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let slot_index = temporaries.next_aggregate_slot();
    let progress = StructInitializationProgress::new_with_temporaries(
        fields,
        literal,
        &drop_kind,
        temporaries,
    )?;
    evaluation.sync_temporaries(temporaries)?;
    if !evaluation.register_struct_fields(
        slot_index,
        expected_layout,
        drop_kind.clone(),
        progress.drop_states(),
    ) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(progress.initialize());
    instructions.extend(
        lower_aggregate_struct_literal_to_location_at_offset_with_temporaries(
            literal,
            expected_layout,
            AggregateLocation::Slot(slot_index),
            0,
            "E8006",
            &format!("arguments for function `{callee_name}`"),
            resolved,
            evaluation.context(),
            temporaries,
            Some(&progress),
        )?,
    );
    if !evaluation.complete_temporary(slot_index, expected_layout, drop_kind) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    Ok(Some((
        instructions,
        AggregateArgumentSource::Slot(slot_index),
    )))
}

pub(super) fn lower_tracked_array_argument_source(
    argument: &Expr,
    parameter_type: &Type,
    parameter_type_expr: Option<&TypeExpr>,
    callee_name: &str,
    evaluation: &mut CallEvaluationContext<'_, '_>,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<(Vec<Instruction>, AggregateArgumentSource)>, Vec<Diagnostic>> {
    let Expr::ArrayLiteral(literal) = unwrap_group(argument) else {
        return Ok(None);
    };
    if !array_literal_requires_runtime_progress(literal) {
        return Ok(None);
    }
    let Some(parameter_type_expr) = parameter_type_expr else {
        return Ok(None);
    };
    let expected_layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => return Ok(None),
    };
    let Some(drop_kind @ AggregateDrop::Array(_)) = evaluation
        .context()
        .aggregate_drop_for_type_expr(parameter_type_expr)
    else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = evaluation.context().resolved_calls() else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let value = abi_value_from_type_expr_with_resolver(parameter_type_expr, resolved, |source| {
        evaluation.context().resolved_source(source)
    })
    .map_err(|_error| unsupported_aggregate_argument_diagnostic(callee_name, parameter_type))?;
    if value.layout != expected_layout || !matches!(&value.ty, AbiType::Array { .. }) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let slot_index = temporaries.next_aggregate_slot();
    let AbiType::Array { element, .. } = &value.ty else {
        return Ok(None);
    };
    let initialized = temporaries.next_usize()?;
    let progress = ArrayInitializationProgress::with_allocator(
        literal,
        element,
        &drop_kind,
        initialized,
        temporaries,
    )?;
    evaluation.sync_temporaries(temporaries)?;
    if !evaluation.register_array_prefix(
        slot_index,
        expected_layout,
        drop_kind.clone(),
        progress.location(),
        progress.element_states(),
    ) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(progress.initialize());
    instructions.extend(lower_aggregate_array_literal_to_location_with_progress(
        literal,
        &value.ty,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        0,
        "E8006",
        &format!("arguments for function `{callee_name}`"),
        resolved,
        evaluation.context(),
        temporaries,
        Some(&progress),
    )?);
    if !evaluation.complete_temporary(slot_index, expected_layout, drop_kind) {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    Ok(Some((
        instructions,
        AggregateArgumentSource::Slot(slot_index),
    )))
}

pub(super) fn lower_aggregate_argument_source(
    argument: &Expr,
    is_owned_method_receiver: bool,
    parameter_type: &Type,
    parameter_type_expr: Option<&TypeExpr>,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    if crate::typecheck::sequence_spread(argument).is_some() {
        return Err(spread_argument_diagnostic(
            "spread argument bypassed its planned iterator lowering",
        ));
    }
    let expected_layout = match parameter_type {
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => *layout,
        _ => unreachable!("aggregate argument lowering requires aggregate parameter type"),
    };
    if matches!(
        unwrap_group(argument),
        Expr::TypedSequenceLiteral(_) | Expr::TypedStringLiteral(_)
    ) {
        let slot_index = temporaries.next_aggregate_slot();
        let mut instructions = vec![Instruction::ReserveAggregateSlot {
            slot_index,
            layout: expected_layout,
        }];
        instructions.extend(
            crate::ir::lower::typed_literals::lower_typed_literal_to_location(
                argument,
                AggregateLocation::Slot(slot_index),
                context,
            )?
            .ok_or_else(|| {
                unsupported_aggregate_argument_diagnostic(callee_name, parameter_type)
            })?,
        );
        return Ok((instructions, AggregateArgumentSource::Slot(slot_index)));
    }
    if let Some(source) = lower_payload_enum_constructor_argument_source(
        argument,
        parameter_type,
        parameter_type_expr,
        expected_layout,
        callee_name,
        context,
        temporaries,
    )? {
        return Ok(source);
    }

    match unwrap_group(argument) {
        Expr::Identifier(identifier) => lower_aggregate_local_argument_source(
            &identifier.name,
            if is_owned_method_receiver {
                AggregateValueUse::ExplicitMove
            } else {
                AggregateValueUse::ImplicitCopy
            },
            expected_layout,
            parameter_type,
            callee_name,
            context,
        ),
        Expr::Member(_) => lower_aggregate_member_argument_source(
            argument,
            expected_layout,
            parameter_type,
            callee_name,
            context,
            temporaries,
        ),
        Expr::Index(index) => lower_aggregate_slice_index_argument_source(
            index,
            expected_layout,
            parameter_type,
            callee_name,
            context,
            temporaries,
        ),
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unary.operand.as_ref() else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_aggregate_local_argument_source(
                &identifier.name,
                AggregateValueUse::ExplicitMove,
                expected_layout,
                parameter_type,
                callee_name,
                context,
            )
        }
        Expr::StructLiteral(literal) => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            let slot_index = temporaries.next_aggregate_slot();
            let mut instructions = vec![Instruction::ReserveAggregateSlot {
                slot_index,
                layout: expected_layout,
            }];
            instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
                literal,
                expected_layout,
                AggregateLocation::Slot(slot_index),
                "E8006",
                &format!("arguments for function `{callee_name}`"),
                resolved,
                context,
                temporaries,
            )?);
            Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
        }
        Expr::ArrayLiteral(literal) => {
            let Some(parameter_type_expr) = parameter_type_expr else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            let value =
                abi_value_from_type_expr_with_resolver(parameter_type_expr, resolved, |source| {
                    context.resolved_source(source)
                })
                .map_err(|_error| {
                    unsupported_aggregate_argument_diagnostic(callee_name, parameter_type)
                })?;
            if value.layout != expected_layout || !matches!(value.ty, AbiType::Array { .. }) {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            }

            let slot_index = temporaries.next_aggregate_slot();
            let mut instructions = vec![Instruction::ReserveAggregateSlot {
                slot_index,
                layout: expected_layout,
            }];
            instructions.extend(lower_aggregate_array_literal_to_location_with_temporaries(
                literal,
                &value.ty,
                expected_layout,
                AggregateLocation::Slot(slot_index),
                "E8006",
                &format!("arguments for function `{callee_name}`"),
                resolved,
                context,
                temporaries,
            )?);
            Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
        }
        Expr::Call(call) => lower_aggregate_call_argument_source(
            call,
            parameter_type,
            callee_name,
            context,
            temporaries,
        ),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_aggregate_fallible_call_argument_source(
                call,
                parameter_type,
                callee_name,
                context,
                temporaries,
                propagating_failure_mode(context)?,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_aggregate_fallible_call_argument_source(
                call,
                parameter_type,
                callee_name,
                context,
                temporaries,
                FallibleFailureMode::Trap,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_aggregate_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_aggregate_fallible_call_argument_source(
                call,
                parameter_type,
                callee_name,
                context,
                temporaries,
                lower_catch_failure_mode(catch, context, 0)?,
            )
        }
        Expr::Otherwise(otherwise) => {
            let slot_index = temporaries.next_aggregate_slot();
            let expected_abi_type =
                aggregate_argument_expected_abi_type(parameter_type_expr, expected_layout, context);
            let mut instructions = vec![Instruction::ReserveAggregateSlot {
                slot_index,
                layout: expected_layout,
            }];
            instructions.extend(lower_aggregate_optional_otherwise_to_location(
                AggregateLocation::Slot(slot_index),
                0,
                expected_layout,
                expected_abi_type.as_ref(),
                otherwise,
                context,
                || unsupported_aggregate_argument_diagnostic(callee_name, parameter_type),
            )?);
            Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
        }
        _ => Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        )),
    }
}

fn lower_payload_enum_constructor_argument_source(
    argument: &Expr,
    parameter_type: &Type,
    parameter_type_expr: Option<&TypeExpr>,
    expected_layout: ValueLayout,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<(Vec<Instruction>, AggregateArgumentSource)>, Vec<Diagnostic>> {
    let Some((member, _arguments)) = payload_enum_constructor_member_and_arguments(argument) else {
        return Ok(None);
    };
    let Some(parameter_type_expr) = parameter_type_expr else {
        return Ok(None);
    };
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let value = abi_value_from_type_expr_with_resolver(parameter_type_expr, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_error| unsupported_aggregate_argument_diagnostic(callee_name, parameter_type))?;
    let AbiType::Enum(enum_) = &value.ty else {
        return Ok(None);
    };
    if value.layout != expected_layout {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    if !enum_
        .variants
        .iter()
        .any(|variant| variant.name == member.member)
    {
        return Ok(None);
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    let Some(mut constructor_instructions) = lower_payload_enum_constructor_to_location(
        argument,
        &value.ty,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8006",
        &format!("arguments for function `{callee_name}`"),
        resolved,
        context,
    )?
    else {
        return Ok(None);
    };
    instructions.append(&mut constructor_instructions);
    Ok(Some((
        instructions,
        AggregateArgumentSource::Slot(slot_index),
    )))
}

fn aggregate_argument_expected_abi_type(
    parameter_type_expr: Option<&TypeExpr>,
    expected_layout: ValueLayout,
    context: &LoweringContext,
) -> Option<AbiType> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let value = abi_value_from_type_expr_with_resolver(parameter_type_expr?, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;
    (value.layout == expected_layout).then_some(value.ty)
}

fn lower_aggregate_local_argument_source(
    name: &str,
    value_use: AggregateValueUse,
    expected_layout: crate::abi::ValueLayout,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let Some(local) = context.aggregate_local(name) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if local.layout != expected_layout
        || (value_use == AggregateValueUse::ImplicitCopy && !local.is_copy)
    {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    Ok((Vec::new(), AggregateArgumentSource::Slot(local.slot_index)))
}

fn lower_aggregate_slice_index_argument_source(
    expression: &IndexExpr,
    expected_layout: crate::abi::ValueLayout,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)
        .map_err(|_| unsupported_aggregate_argument_diagnostic(callee_name, parameter_type))?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)
        .map_err(|_| unsupported_aggregate_argument_diagnostic(callee_name, parameter_type))?;
    let SliceValue::Location(source_location) = source.value else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(source.instructions);
    instructions.extend(index.instructions);
    let index = materialize_slice_borrow_index(&mut instructions, index.value, temporaries)?;
    instructions.push(Instruction::CopySliceElementToAggregate {
        destination: AggregateLocation::Slot(slot_index),
        source: source_location,
        index,
        layout: expected_layout,
    });
    Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AggregateValueUse {
    ImplicitCopy,
    ExplicitMove,
}

fn lower_aggregate_member_argument_source(
    argument: &Expr,
    expected_layout: crate::abi::ValueLayout,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let access = lower_aggregate_member_field_access(argument, context, temporaries)?
        .ok_or_else(|| unsupported_aggregate_argument_diagnostic(callee_name, parameter_type))?;
    let source = access.source;
    let source_offset = access.offset;
    let is_copy = access.is_copy;
    let Some(layout) = access.kind.copy_aggregate_layout() else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if layout != expected_layout || !is_copy {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = access.instructions;
    instructions.push(Instruction::ReserveAggregateSlot { slot_index, layout });
    instructions.push(Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(slot_index),
        destination_offset: 0,
        source,
        source_offset,
        layout,
    });
    Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
}

fn lower_aggregate_call_argument_source(
    call: &CallExpr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let Some(return_type) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if return_type != parameter_type {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    let Some(layout) = aggregate_type_layout(return_type) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (call_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.extend(call_instructions);
    push_aggregate_call_instruction(
        &mut instructions,
        return_type,
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
    );
    Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
}

fn lower_aggregate_fallible_call_argument_source(
    call: &CallExpr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<(Vec<Instruction>, AggregateArgumentSource), Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if success_type.as_ref() != parameter_type {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    let Some(layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Err(unsupported_aggregate_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (call_instructions, arguments) =
        lower_call_arguments(call, &target, &call_name, context, temporaries)?;
    instructions.extend(call_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok((instructions, AggregateArgumentSource::Slot(slot_index)))
}

fn unsupported_aggregate_argument_diagnostic(
    callee_name: &str,
    parameter_type: &Type,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower `{}` arguments for function `{callee_name}` from supported aggregate locals, struct literals, or aggregate calls",
            describe_type(parameter_type),
        ),
    )]
}

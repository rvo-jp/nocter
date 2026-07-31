use super::*;

#[derive(Clone)]
pub(super) struct BranchPrologueBinding {
    pub(super) name: String,
    pub(super) source_slot: usize,
    pub(super) payload_offset: u32,
    pub(super) source_drop_flag: Option<BoolLocation>,
    pub(super) kind: BranchPrologueBindingKind,
    pub(super) diagnostic_code: &'static str,
}

#[derive(Clone)]
pub(super) enum BranchPrologueBindingKind {
    ScalarOrStrView(AbiType),
    SliceView(SliceTypeInfo),
    CopyAggregate {
        layout: ValueLayout,
        fields: Vec<AggregateField>,
    },
    MoveAggregate {
        layout: ValueLayout,
        fields: Vec<AggregateField>,
        drop_kind: AggregateDrop,
    },
}

impl BranchPrologueBinding {
    pub(super) fn lower(
        &self,
        context: &mut LoweringContext,
    ) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
        let source = AggregateLocation::Slot(self.source_slot);
        match &self.kind {
            BranchPrologueBindingKind::MoveAggregate {
                layout,
                fields,
                drop_kind,
            } => {
                let Some(source_drop_flag) = self.source_drop_flag else {
                    return Err(unsupported_if_is_diagnostic(self.diagnostic_code));
                };
                let slot_index = context.define_aggregate_local(
                    self.name.clone(),
                    *layout,
                    false,
                    Some(drop_kind.clone()),
                    fields.clone(),
                );
                context.mark_aggregate_local_dropped_by_slot(self.source_slot);
                Ok(vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index,
                        layout: *layout,
                    },
                    Instruction::CopyAggregateRange {
                        destination: AggregateLocation::Slot(slot_index),
                        destination_offset: 0,
                        source,
                        source_offset: self.payload_offset,
                        layout: *layout,
                    },
                    Instruction::SetBool {
                        destination: source_drop_flag,
                        value: BoolValue::Const(false),
                    },
                ])
            }
            BranchPrologueBindingKind::CopyAggregate { layout, fields } => {
                let slot_index = context.define_aggregate_local(
                    self.name.clone(),
                    *layout,
                    true,
                    None,
                    fields.clone(),
                );
                Ok(vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index,
                        layout: *layout,
                    },
                    Instruction::CopyAggregateRange {
                        destination: AggregateLocation::Slot(slot_index),
                        destination_offset: 0,
                        source,
                        source_offset: self.payload_offset,
                        layout: *layout,
                    },
                ])
            }
            BranchPrologueBindingKind::SliceView(info) => {
                let destination = context.next_slice_local_location()?;
                let SliceLocation::Local(index) = destination else {
                    unreachable!("local slice binding locations are local pairs");
                };
                let instructions = payload_view_binding_loads(
                    index,
                    source,
                    self.payload_offset,
                    self.diagnostic_code,
                )?;
                context.define_slice_local(
                    self.name.clone(),
                    info.element_kind,
                    info.element_type.clone(),
                );
                Ok(instructions)
            }
            BranchPrologueBindingKind::ScalarOrStrView(payload_type) => match payload_type {
                AbiType::I32 => {
                    let destination = context.next_i32_local_location()?;
                    context.define_i32_local(self.name.clone());
                    Ok(vec![Instruction::LoadAggregateI32 {
                        destination,
                        source,
                        offset: self.payload_offset,
                    }])
                }
                AbiType::U8 => {
                    let destination = context.next_u8_local_location()?;
                    context.define_u8_local(self.name.clone());
                    Ok(vec![Instruction::LoadAggregateU8 {
                        destination,
                        source,
                        offset: self.payload_offset,
                    }])
                }
                AbiType::Usize => {
                    let destination = context.next_usize_local_location()?;
                    context.define_usize_local(self.name.clone());
                    Ok(vec![Instruction::LoadAggregateUsize {
                        destination,
                        source,
                        offset: self.payload_offset,
                    }])
                }
                AbiType::Bool => {
                    let destination = context.next_bool_local_location()?;
                    context.define_bool_local(self.name.clone());
                    Ok(vec![Instruction::LoadAggregateBool {
                        destination,
                        source,
                        offset: self.payload_offset,
                    }])
                }
                AbiType::StrView => {
                    let destination = context.next_str_local_location()?;
                    let StrLocation::Local(index) = destination else {
                        unreachable!("local str binding locations are local pairs");
                    };
                    let instructions = payload_view_binding_loads(
                        index,
                        source,
                        self.payload_offset,
                        self.diagnostic_code,
                    )?;
                    context.define_str_local(self.name.clone());
                    Ok(instructions)
                }
                _ => Err(unsupported_if_is_diagnostic(self.diagnostic_code)),
            },
        }
    }
}

pub(super) fn payload_view_binding_loads(
    index: usize,
    source: AggregateLocation,
    payload_offset: u32,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let len_index = index.checked_add(1).ok_or_else(|| {
        payload_binding_overflow_diagnostic(
            diagnostic_code,
            "IR v0 cannot lower payload enum bindings with overflowing local indexes",
        )
    })?;
    let len_offset = payload_offset.checked_add(8).ok_or_else(|| {
        payload_binding_overflow_diagnostic(
            diagnostic_code,
            "IR v0 cannot lower payload enum bindings with overflowing payload offsets",
        )
    })?;
    Ok(vec![
        Instruction::LoadAggregateUsize {
            destination: UsizeLocation::Local(index),
            source,
            offset: payload_offset,
        },
        Instruction::LoadAggregateUsize {
            destination: UsizeLocation::Local(len_index),
            source,
            offset: len_offset,
        },
    ])
}

pub(super) fn payload_binding_overflow_diagnostic(
    diagnostic_code: &'static str,
    message: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(diagnostic_code, message)]
}

pub(super) fn payloadless_if_is_variant_expression(statement: &IfIsStmt) -> Expr {
    Expr::Member(MemberExpr {
        span: statement.pattern_span,
        object: Box::new(Expr::Identifier(IdentifierExpr {
            span: statement.enum_name_span,
            name: statement.enum_name.clone(),
        })),
        member: statement.variant_name.clone(),
        member_span: statement.variant_name_span,
    })
}

pub(super) fn tag_only_if_is_payload_pattern_is_supported(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
) -> bool {
    matches!(
        (payload, payload_len),
        (None, 0)
            | (Some(SwitchPayloadPattern::Discard(_)), 1)
            | (Some(SwitchPayloadPattern::Binding(_)), 1)
    )
}

pub(super) fn tag_only_switch_payload_pattern_is_supported(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
) -> bool {
    matches!(
        (payload, payload_len),
        (None, 0)
            | (Some(SwitchPayloadPattern::Discard(_)), 1)
            | (Some(SwitchPayloadPattern::Binding(_)), 1)
    )
}

pub(super) fn tag_only_if_is_then_prologue(
    statement: &IfIsStmt,
    source_slot: usize,
    source_drop_flag: Option<BoolLocation>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BranchPrologue, Vec<Diagnostic>> {
    let Some(SwitchPayloadPattern::Binding(binding)) = &statement.payload else {
        return Ok(BranchPrologue::empty());
    };
    let (payload_offset, payload_type) =
        payload_enum_variant_payload_abi(&statement.expression, &statement.variant_name, context)
            .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    let kind = payload_branch_prologue_binding_kind(binding, payload_type, context)
        .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    Ok(BranchPrologue::single_binding(BranchPrologueBinding {
        name: binding.name.clone(),
        source_slot,
        payload_offset,
        source_drop_flag,
        kind,
        diagnostic_code,
    }))
}

pub(super) fn payload_enum_variant_payload_abi(
    expression: &Expr,
    variant_name: &str,
    context: &LoweringContext,
) -> Option<(u32, AbiType)> {
    let ty = context.expression_type_expr(expression.span())?;
    let (_, resolved) = context.resolved_calls()?;
    let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;
    let AbiType::Enum(enum_) = value.ty else {
        return None;
    };
    let payload_offset = u32::try_from(enum_.payload_offset).ok()?;
    let payload_type = enum_
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)?
        .payload
        .clone()?;
    Some((payload_offset, payload_type))
}

pub(super) fn payload_binding_abi_type_is_supported(payload_type: &AbiType) -> bool {
    matches!(
        payload_type,
        AbiType::I32 | AbiType::U8 | AbiType::Usize | AbiType::Bool | AbiType::StrView
    )
}

pub(super) fn payload_branch_prologue_binding_kind(
    binding: &crate::ast::SwitchPayloadBinding,
    payload_type: AbiType,
    context: &LoweringContext,
) -> Option<BranchPrologueBindingKind> {
    if payload_binding_abi_type_is_supported(&payload_type) {
        return Some(BranchPrologueBindingKind::ScalarOrStrView(payload_type));
    }

    let ty = context.binding_type_expr(binding.span)?;
    let (_, resolved) = context.resolved_calls()?;
    let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;

    if context.payload_binding_mode(binding.span) == Some(TypecheckPayloadBindingMode::Move) {
        if !matches!(payload_type, AbiType::Struct(_) | AbiType::Array { .. })
            || value.ty != payload_type
            || !supported_aggregate_copy_layout(value.layout)
        {
            return None;
        }
        let (root_source, _) = context.resolved_calls()?;
        let fields =
            aggregate_fields_from_type_expr_with_resolver(&ty, root_source, resolved, |source| {
                context.resolved_source(source)
            })
            .unwrap_or_default();
        let drop_kind = context.aggregate_drop_for_type_expr(&ty)?;
        return Some(BranchPrologueBindingKind::MoveAggregate {
            layout: value.layout,
            fields,
            drop_kind,
        });
    }

    if matches!(payload_type, AbiType::SliceView) {
        if !matches!(value.ty, AbiType::SliceView)
            || value.layout != layout_of(&payload_type).ok()?
        {
            return None;
        }
        return Some(BranchPrologueBindingKind::SliceView(
            slice_type_info_from_type_expr_with_resolver(&ty, resolved, |source| {
                context.resolved_source(source)
            }),
        ));
    }

    if !matches!(payload_type, AbiType::Struct(_) | AbiType::Array { .. }) {
        return None;
    }
    let payload_layout = layout_of(&payload_type).ok()?;
    if !supported_aggregate_copy_layout(payload_layout) {
        return None;
    }
    if value.layout != payload_layout
        || !matches!(value.ty, AbiType::Struct(_) | AbiType::Array { .. })
        || !type_expr_is_copy_aggregate_value_with_resolver(&ty, resolved, |source| {
            context.resolved_source(source)
        })
    {
        return None;
    }
    let (root_source, _) = context.resolved_calls()?;
    let fields =
        aggregate_fields_from_type_expr_with_resolver(&ty, root_source, resolved, |source| {
            context.resolved_source(source)
        })
        .unwrap_or_default();
    Some(BranchPrologueBindingKind::CopyAggregate {
        layout: payload_layout,
        fields,
    })
}

pub(super) struct LoweredPayloadEnumPatternTarget {
    pub(super) leading_instructions: Vec<Instruction>,
    pub(super) slot_index: usize,
    pub(super) cleanup: Option<PatternTargetCleanup>,
    pub(super) drop_flag: Option<BoolLocation>,
}

pub(super) fn lower_payload_enum_pattern_target(
    expression: &Expr,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    unsupported_diagnostic: fn(&'static str) -> Vec<Diagnostic>,
    needs_drop_flag: bool,
) -> Result<LoweredPayloadEnumPatternTarget, Vec<Diagnostic>> {
    if let Some(slot_index) = tag_only_if_is_aggregate_source_slot(expression, context) {
        return Ok(LoweredPayloadEnumPatternTarget {
            leading_instructions: Vec::new(),
            slot_index,
            cleanup: None,
            drop_flag: None,
        });
    }
    if !payload_enum_pattern_target_expression_shape_is_supported(expression, context) {
        return Err(unsupported_diagnostic(diagnostic_code));
    }

    let Some(ty) = context.expression_type_expr(expression.span()) else {
        return Err(unsupported_diagnostic(diagnostic_code));
    };
    let (value, fields, return_type) = {
        let Some((root_source, resolved)) = context.resolved_calls() else {
            return Err(unsupported_diagnostic(diagnostic_code));
        };
        let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
            context.resolved_source(source)
        })
        .map_err(|_| unsupported_diagnostic(diagnostic_code))?;
        if !matches!(value.ty, AbiType::Enum(_)) {
            return Err(unsupported_diagnostic(diagnostic_code));
        }

        let fields =
            aggregate_fields_from_type_expr_with_resolver(&ty, root_source, resolved, |source| {
                context.resolved_source(source)
            })
            .ok_or_else(|| unsupported_diagnostic(diagnostic_code))?;
        let return_type = return_type_from_type_expr_with_resolver(&ty, resolved, |source| {
            context.resolved_source(source)
        })
        .ok_or_else(|| unsupported_diagnostic(diagnostic_code))?;
        (value, fields, return_type)
    };
    let drop_kind = context.aggregate_drop_for_type_expr(&ty);
    let local_mark = context.local_mark();
    let local_name = payload_enum_pattern_target_name(expression);
    let slot_index =
        context.define_aggregate_local(local_name.clone(), value.layout, false, drop_kind, fields);
    context.mark_aggregate_local_dropped(&local_name);

    let drop_flag = if needs_drop_flag {
        let flag = context.next_bool_local_location()?;
        context.define_bool_local(format!("{local_name}:needs-drop"));
        Some(flag)
    } else {
        None
    };

    let mut leading_instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    if let Some(drop_flag) = drop_flag {
        leading_instructions.push(Instruction::SetBool {
            destination: drop_flag,
            value: BoolValue::Const(true),
        });
    }
    let lowered_expression = {
        let function_name = context.function_name().to_string();
        let Some((_, resolved)) = context.resolved_calls() else {
            return Err(unsupported_diagnostic(diagnostic_code));
        };
        if let Some(instructions) = lower_payload_enum_constructor_value_to_location(
            expression,
            &value,
            value.layout,
            AggregateLocation::Slot(slot_index),
            &function_name,
            resolved,
            context,
        )
        .map_err(|_| unsupported_diagnostic(diagnostic_code))?
        {
            instructions
        } else {
            lower_aggregate_return_expression_to_location(
                expression,
                &return_type,
                AggregateLocation::Slot(slot_index),
                &function_name,
                resolved,
                context,
            )
            .map_err(|_| unsupported_diagnostic(diagnostic_code))?
        }
    };
    leading_instructions.extend(lowered_expression);
    mark_explicit_moves_in_expression(expression, context);
    context.mark_aggregate_local_initialized(&local_name);

    Ok(LoweredPayloadEnumPatternTarget {
        leading_instructions,
        slot_index,
        cleanup: Some(PatternTargetCleanup {
            local_mark,
            drop_flag,
        }),
        drop_flag,
    })
}

pub(super) fn payload_enum_pattern_target_expression_shape_is_supported(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match unwrap_group(expression) {
        Expr::Identifier(_) | Expr::Call(_) => true,
        Expr::Member(member) => context.enum_variant_tag(member).is_some(),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            matches!(unwrap_group(&unary.operand), Expr::Identifier(_))
        }
        _ => false,
    }
}

pub(super) fn payload_enum_pattern_target_name(expression: &Expr) -> String {
    let span = expression.span();
    format!(
        "<payload-pattern-target:{}:{}:{}>",
        span.source.raw(),
        span.start,
        span.end
    )
}

pub(super) fn tag_only_if_is_aggregate_source_slot(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<usize> {
    let Expr::Identifier(identifier) = unwrap_group(expression) else {
        return None;
    };
    context
        .aggregate_local(&identifier.name)
        .map(|local| local.slot_index)
}

pub(super) fn tag_only_if_is_target_name(statement: &IfIsStmt) -> String {
    format!(
        "<if-is:{}:{}:{}>",
        statement.span.source.raw(),
        statement.span.start,
        statement.span.end
    )
}

pub(super) fn payload_enum_tag_only_switch_variant_names(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> Option<Vec<String>> {
    let Some(first_arm) = statement.arms.first() else {
        return statement.wildcard_arm.as_ref().and_then(|_| {
            context.payload_enum_variant_names_for_expression(&statement.expression)
        });
    };
    let (_, resolved) = context.resolved_calls()?;
    let target_symbol = resolved.type_symbol_by_name(&first_arm.enum_name)?;
    if target_symbol.kind != crate::resolve::TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return None;
    }

    let arms_are_supported = statement.arms.iter().all(|arm| {
        let Some(arm_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
            return false;
        };
        if arm_symbol.canonical_name != target_symbol.canonical_name {
            return false;
        }
        let Some(variant) = target_symbol
            .variants
            .iter()
            .find(|variant| variant.name == arm.variant_name)
        else {
            return false;
        };
        tag_only_switch_payload_pattern_is_supported(arm.payload.as_ref(), variant.payload.len())
    });
    arms_are_supported.then(|| {
        target_symbol
            .variants
            .iter()
            .map(|variant| variant.name.clone())
            .collect()
    })
}

pub(super) fn tag_only_switch_body(
    statement: &SwitchStmt,
    target: Expr,
    variant_names: &[String],
    source_slot: usize,
    source_drop_flag: Option<BoolLocation>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredPayloadlessSwitchBody, Vec<Diagnostic>> {
    let Some((condition_arms, fallback)) = tag_only_switch_condition_arms_and_fallback(
        statement,
        variant_names,
        source_slot,
        source_drop_flag,
        context,
        diagnostic_code,
    )?
    else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };

    if condition_arms.is_empty() {
        return Ok(LoweredPayloadlessSwitchBody::Direct(fallback));
    }

    let mut current = LoweredPayloadlessSwitchBody::Direct(fallback);
    for arm in condition_arms.iter().rev() {
        current = LoweredPayloadlessSwitchBody::Conditional(LoweredSwitchCondition {
            condition: Expr::Binary(BinaryExpr {
                span: arm.span,
                left: Box::new(target.clone()),
                operator: BinaryOperator::Equal,
                operator_span: arm.span,
                right: Box::new(tag_only_switch_variant_tag_expression(
                    arm,
                    context,
                    diagnostic_code,
                )?),
            }),
            then_branch: tag_only_switch_arm_block(
                arm,
                &statement.expression,
                source_slot,
                source_drop_flag,
                context,
                diagnostic_code,
            )?,
            else_body: Box::new(current),
        });
    }

    Ok(current)
}

pub(super) fn tag_only_switch_condition_arms_and_fallback<'a>(
    statement: &'a SwitchStmt,
    variant_names: &[String],
    source_slot: usize,
    source_drop_flag: Option<BoolLocation>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Option<(&'a [SwitchArm], LoweredSwitchBlock)>, Vec<Diagnostic>> {
    if let Some(wildcard_arm) = &statement.wildcard_arm {
        return Ok(Some((
            &statement.arms,
            LoweredSwitchBlock {
                block: wildcard_arm.body.clone(),
                prologue: BranchPrologue::empty(),
            },
        )));
    }

    if !payloadless_switch_covers_all_variants(statement, variant_names) {
        return Ok(Some((
            &statement.arms,
            LoweredSwitchBlock {
                block: Block {
                    span: statement.span,
                    statements: Vec::new(),
                    result: None,
                },
                prologue: BranchPrologue::empty(),
            },
        )));
    }

    if statement.arms.len() == 1 {
        return Ok(Some((
            &[],
            tag_only_switch_arm_block(
                &statement.arms[0],
                &statement.expression,
                source_slot,
                source_drop_flag,
                context,
                diagnostic_code,
            )?,
        )));
    }

    let Some((last, condition_arms)) = statement.arms.split_last() else {
        return Ok(None);
    };
    Ok(Some((
        condition_arms,
        tag_only_switch_arm_block(
            last,
            &statement.expression,
            source_slot,
            source_drop_flag,
            context,
            diagnostic_code,
        )?,
    )))
}

pub(super) fn tag_only_switch_arm_block(
    arm: &SwitchArm,
    target_expression: &Expr,
    source_slot: usize,
    source_drop_flag: Option<BoolLocation>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredSwitchBlock, Vec<Diagnostic>> {
    Ok(LoweredSwitchBlock {
        block: arm.body.clone(),
        prologue: tag_only_switch_arm_prologue(
            arm,
            target_expression,
            source_slot,
            source_drop_flag,
            context,
            diagnostic_code,
        )?,
    })
}

pub(super) fn tag_only_switch_arm_prologue(
    arm: &SwitchArm,
    target_expression: &Expr,
    source_slot: usize,
    source_drop_flag: Option<BoolLocation>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BranchPrologue, Vec<Diagnostic>> {
    let Some(SwitchPayloadPattern::Binding(binding)) = &arm.payload else {
        return Ok(BranchPrologue::empty());
    };
    let (payload_offset, payload_type) =
        payload_enum_variant_payload_abi(target_expression, &arm.variant_name, context)
            .ok_or_else(|| unsupported_switch_diagnostic(diagnostic_code))?;
    let kind = payload_branch_prologue_binding_kind(binding, payload_type, context)
        .ok_or_else(|| unsupported_switch_diagnostic(diagnostic_code))?;
    Ok(BranchPrologue::single_binding(BranchPrologueBinding {
        name: binding.name.clone(),
        source_slot,
        payload_offset,
        source_drop_flag,
        kind,
        diagnostic_code,
    }))
}

pub(super) fn tag_only_switch_variant_tag_expression(
    arm: &SwitchArm,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Expr, Vec<Diagnostic>> {
    let variant = payloadless_switch_variant_expression(arm);
    let Expr::Member(member) = &variant else {
        unreachable!("switch variant expression must be a member expression");
    };
    let tag = context
        .enum_variant_tag(member)
        .ok_or_else(|| unsupported_switch_diagnostic(diagnostic_code))?;
    Ok(Expr::IntegerLiteral(LiteralExpr {
        span: arm.variant_name_span,
        value: tag.to_string(),
    }))
}

pub(super) fn payloadless_switch_variant_names(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> Option<Vec<String>> {
    let Some(first_arm) = statement.arms.first() else {
        return context.payloadless_enum_variant_names_for_expression(&statement.expression);
    };
    let (_, resolved) = context.resolved_calls()?;
    let target_symbol = resolved.type_symbol_by_name(&first_arm.enum_name)?;
    if target_symbol.kind != crate::resolve::TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return None;
    }

    let arms_are_supported = statement.arms.iter().all(|arm| {
        if arm.payload.is_some() {
            return false;
        }
        let Some(arm_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
            return false;
        };
        if arm_symbol.canonical_name != target_symbol.canonical_name {
            return false;
        }
        let variant = payloadless_switch_variant_expression(arm);
        let Expr::Member(member) = &variant else {
            unreachable!("payloadless switch variant expression must be a member expression");
        };
        context.payloadless_enum_variant_tag(member).is_some()
    });
    arms_are_supported.then(|| {
        target_symbol
            .variants
            .iter()
            .map(|variant| variant.name.clone())
            .collect()
    })
}

pub(super) fn payloadless_switch_body(
    statement: &SwitchStmt,
    target: Expr,
    variant_names: &[String],
    diagnostic_code: &'static str,
) -> Result<LoweredPayloadlessSwitchBody, Vec<Diagnostic>> {
    let Some((condition_arms, fallback)) =
        payloadless_switch_condition_arms_and_fallback(statement, variant_names)
    else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };

    let fallback = LoweredSwitchBlock {
        block: fallback,
        prologue: BranchPrologue::empty(),
    };
    if condition_arms.is_empty() {
        return Ok(LoweredPayloadlessSwitchBody::Direct(fallback));
    }

    let mut current = LoweredPayloadlessSwitchBody::Direct(fallback);
    for arm in condition_arms.iter().rev() {
        current = LoweredPayloadlessSwitchBody::Conditional(LoweredSwitchCondition {
            condition: Expr::Binary(BinaryExpr {
                span: arm.span,
                left: Box::new(target.clone()),
                operator: BinaryOperator::Equal,
                operator_span: arm.span,
                right: Box::new(payloadless_switch_variant_expression(arm)),
            }),
            then_branch: LoweredSwitchBlock {
                block: arm.body.clone(),
                prologue: BranchPrologue::empty(),
            },
            else_body: Box::new(current),
        });
    }

    Ok(current)
}

pub(super) fn payloadless_switch_condition_arms_and_fallback<'a>(
    statement: &'a SwitchStmt,
    variant_names: &[String],
) -> Option<(&'a [SwitchArm], Block)> {
    if let Some(wildcard_arm) = &statement.wildcard_arm {
        return Some((&statement.arms, wildcard_arm.body.clone()));
    }

    if !payloadless_switch_covers_all_variants(statement, variant_names) {
        return Some((
            &statement.arms,
            Block {
                span: statement.span,
                statements: Vec::new(),
                result: None,
            },
        ));
    }

    if statement.arms.len() == 1 {
        return Some((&statement.arms, statement.arms[0].body.clone()));
    }

    let (last, condition_arms) = statement.arms.split_last()?;
    Some((condition_arms, last.body.clone()))
}

pub(super) fn payloadless_switch_covers_all_variants(
    statement: &SwitchStmt,
    variant_names: &[String],
) -> bool {
    variant_names.iter().all(|variant_name| {
        statement
            .arms
            .iter()
            .any(|arm| arm.variant_name == *variant_name)
    })
}

pub(super) fn payloadless_switch_variant_expression(arm: &SwitchArm) -> Expr {
    Expr::Member(MemberExpr {
        span: arm.span,
        object: Box::new(Expr::Identifier(IdentifierExpr {
            span: arm.enum_name_span,
            name: arm.enum_name.clone(),
        })),
        member: arm.variant_name.clone(),
        member_span: arm.variant_name_span,
    })
}

pub(super) fn payloadless_switch_target_name(statement: &SwitchStmt) -> String {
    format!(
        "<match:{}:{}:{}>",
        statement.span.source.raw(),
        statement.span.start,
        statement.span.end
    )
}

pub(super) fn unsupported_if_is_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower payloadless `if is` branches or tag-only payload enum `if is` branches over supported enum pattern targets",
    )]
}

pub(super) fn unsupported_switch_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower payloadless enum `match` statements or tag-only payload enum `match` statements over supported enum pattern targets",
    )]
}

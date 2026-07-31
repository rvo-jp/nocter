use super::*;

pub(in crate::ir::lower) fn payloadless_if_is_as_if_statement(
    statement: &IfIsStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<IfStmt, Vec<Diagnostic>> {
    if statement.payload.is_some() {
        return Err(unsupported_if_is_diagnostic(diagnostic_code));
    }

    let variant = payloadless_if_is_variant_expression(statement);
    let Expr::Member(member) = &variant else {
        unreachable!("payloadless if-is variant expression must be a member expression");
    };
    if context.payloadless_enum_variant_tag(member).is_none() {
        return Err(unsupported_if_is_diagnostic(diagnostic_code));
    }

    Ok(IfStmt {
        span: statement.span,
        condition: Expr::Binary(BinaryExpr {
            span: statement.pattern_span,
            left: Box::new(statement.expression.clone()),
            operator: BinaryOperator::Equal,
            operator_span: statement.pattern_span,
            right: Box::new(variant),
        }),
        then_block: statement.then_block.clone(),
        else_block: statement.else_block.clone(),
    })
}

pub(in crate::ir::lower) struct LoweredTagOnlyIfIs {
    pub(in crate::ir::lower) leading_instructions: Vec<Instruction>,
    pub(in crate::ir::lower) statement: IfStmt,
    pub(in crate::ir::lower) then_prologue: BranchPrologue,
    pub(in crate::ir::lower) target_cleanup: Option<PatternTargetCleanup>,
}

#[derive(Clone, Copy)]
pub(in crate::ir::lower) struct PatternTargetCleanup {
    pub(in crate::ir::lower::functions) local_mark: usize,
}

impl PatternTargetCleanup {
    pub(in crate::ir::lower) fn append_to(
        self,
        instructions: &mut Vec<Instruction>,
        context: &mut LoweringContext,
    ) -> Result<(), Vec<Diagnostic>> {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            context,
            self.local_mark,
        )?);
        Ok(())
    }
}

#[derive(Clone)]
pub(in crate::ir::lower) struct BranchPrologue {
    bindings: Vec<BranchPrologueBinding>,
}

impl BranchPrologue {
    pub(in crate::ir::lower) fn empty() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub(in crate::ir::lower::functions) fn single_binding(binding: BranchPrologueBinding) -> Self {
        Self {
            bindings: vec![binding],
        }
    }

    pub(in crate::ir::lower) fn apply(
        &self,
        context: &mut LoweringContext,
    ) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
        let mut instructions = Vec::new();
        for binding in &self.bindings {
            instructions.extend(binding.lower(context)?);
        }
        Ok(instructions)
    }
}

pub(in crate::ir::lower) fn tag_only_if_is_as_control_flow(
    statement: &IfIsStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredTagOnlyIfIs, Vec<Diagnostic>> {
    let variant = payloadless_if_is_variant_expression(statement);
    let Expr::Member(member) = &variant else {
        unreachable!("if-is variant expression must be a member expression");
    };

    if context.payloadless_enum_variant_tag(member).is_some() {
        return payloadless_if_is_as_if_statement(statement, context, diagnostic_code).map(
            |statement| LoweredTagOnlyIfIs {
                leading_instructions: Vec::new(),
                statement,
                then_prologue: BranchPrologue::empty(),
                target_cleanup: None,
            },
        );
    }

    let tag = context
        .enum_variant_tag(member)
        .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    let payload_len = context
        .enum_variant_payload_len(member)
        .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    if !tag_only_if_is_payload_pattern_is_supported(statement.payload.as_ref(), payload_len) {
        return Err(unsupported_if_is_diagnostic(diagnostic_code));
    }
    let source = lower_payload_enum_pattern_target(
        &statement.expression,
        context,
        diagnostic_code,
        unsupported_if_is_diagnostic,
    )?;
    let then_prologue =
        tag_only_if_is_then_prologue(statement, source.slot_index, context, diagnostic_code)?;
    let target_name = tag_only_if_is_target_name(statement);
    let target = context.next_u8_local_location()?;
    context.define_u8_local(target_name.clone());

    let mut leading_instructions = source.leading_instructions;
    leading_instructions.push(Instruction::LoadAggregateU8 {
        destination: target,
        source: AggregateLocation::Slot(source.slot_index),
        offset: 0,
    });

    Ok(LoweredTagOnlyIfIs {
        leading_instructions,
        statement: IfStmt {
            span: statement.span,
            condition: Expr::Binary(BinaryExpr {
                span: statement.pattern_span,
                left: Box::new(Expr::Identifier(IdentifierExpr {
                    span: statement.expression.span(),
                    name: target_name,
                })),
                operator: BinaryOperator::Equal,
                operator_span: statement.pattern_span,
                right: Box::new(Expr::IntegerLiteral(LiteralExpr {
                    span: statement.variant_name_span,
                    value: tag.to_string(),
                })),
            }),
            then_block: statement.then_block.clone(),
            else_block: statement.else_block.clone(),
        },
        then_prologue,
        target_cleanup: source.cleanup,
    })
}

pub(in crate::ir::lower) struct LoweredPayloadlessSwitch {
    pub(in crate::ir::lower) leading_instructions: Vec<Instruction>,
    pub(in crate::ir::lower) body: LoweredPayloadlessSwitchBody,
    pub(in crate::ir::lower) target_cleanup: Option<PatternTargetCleanup>,
}

#[derive(Clone)]
pub(in crate::ir::lower) enum LoweredPayloadlessSwitchBody {
    Direct(LoweredSwitchBlock),
    Conditional(LoweredSwitchCondition),
}

#[derive(Clone)]
pub(in crate::ir::lower) struct LoweredSwitchBlock {
    pub(in crate::ir::lower) block: Block,
    pub(in crate::ir::lower) prologue: BranchPrologue,
}

#[derive(Clone)]
pub(in crate::ir::lower) struct LoweredSwitchCondition {
    pub(in crate::ir::lower) condition: Expr,
    pub(in crate::ir::lower) then_branch: LoweredSwitchBlock,
    pub(in crate::ir::lower) else_body: Box<LoweredPayloadlessSwitchBody>,
}

pub(in crate::ir::lower) fn payloadless_switch_as_control_flow(
    statement: &SwitchStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredPayloadlessSwitch, Vec<Diagnostic>> {
    let Some(variant_names) = payloadless_switch_variant_names(statement, context) else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };

    let target_name = payloadless_switch_target_name(statement);
    let target = context.next_u8_local_location()?;
    context.define_u8_local(target_name.clone());
    let leading_instructions =
        lower_u8_expression_to_location(&statement.expression, target, context)?;
    let target_expression = Expr::Identifier(IdentifierExpr {
        span: statement.expression.span(),
        name: target_name,
    });
    let body = payloadless_switch_body(
        statement,
        target_expression,
        &variant_names,
        diagnostic_code,
    )?;

    Ok(LoweredPayloadlessSwitch {
        leading_instructions,
        body,
        target_cleanup: None,
    })
}

pub(in crate::ir::lower) fn tag_only_switch_as_control_flow(
    statement: &SwitchStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredPayloadlessSwitch, Vec<Diagnostic>> {
    if let Ok(switch) = payloadless_switch_as_control_flow(statement, context, diagnostic_code) {
        return Ok(switch);
    }

    let Some(variant_names) = payload_enum_tag_only_switch_variant_names(statement, context) else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };
    let source = lower_payload_enum_pattern_target(
        &statement.expression,
        context,
        diagnostic_code,
        unsupported_switch_diagnostic,
    )?;

    let target_name = payloadless_switch_target_name(statement);
    let target = context.next_u8_local_location()?;
    context.define_u8_local(target_name.clone());
    let target_expression = Expr::Identifier(IdentifierExpr {
        span: statement.expression.span(),
        name: target_name,
    });
    let body = tag_only_switch_body(
        statement,
        target_expression,
        &variant_names,
        source.slot_index,
        context,
        diagnostic_code,
    )?;

    let mut leading_instructions = source.leading_instructions;
    leading_instructions.push(Instruction::LoadAggregateU8 {
        destination: target,
        source: AggregateLocation::Slot(source.slot_index),
        offset: 0,
    });

    Ok(LoweredPayloadlessSwitch {
        leading_instructions,
        body,
        target_cleanup: source.cleanup,
    })
}

pub(in crate::ir::lower) fn payloadless_switch_is_exhaustive(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> bool {
    if statement.wildcard_arm.is_some() {
        return payloadless_switch_variant_names(statement, context).is_some();
    }

    payloadless_switch_variant_names(statement, context).is_some_and(|variant_names| {
        payloadless_switch_covers_all_variants(statement, &variant_names)
    })
}

pub(in crate::ir::lower) fn lowerable_switch_is_exhaustive(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> bool {
    if statement.wildcard_arm.is_some() {
        return payloadless_switch_variant_names(statement, context).is_some()
            || payload_enum_tag_only_switch_variant_names(statement, context).is_some();
    }

    payloadless_switch_variant_names(statement, context).is_some_and(|variant_names| {
        payloadless_switch_covers_all_variants(statement, &variant_names)
    }) || payload_enum_tag_only_switch_variant_names(statement, context).is_some_and(
        |variant_names| payloadless_switch_covers_all_variants(statement, &variant_names),
    )
}

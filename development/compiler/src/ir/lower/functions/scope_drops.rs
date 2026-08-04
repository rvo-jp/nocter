use super::*;

pub(in crate::ir::lower) fn lower_scope_end_drop_instructions(
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let pending = context.pending_aggregate_drops();
    let mut instructions = Vec::new();
    for drop_ in &pending {
        instructions.extend(lower_pending_aggregate_drop(drop_, context)?);
    }
    instructions.extend(context.allocation_context_restore_instructions());
    instructions.extend(context.all_region_cleanup_instructions());
    Ok(instructions)
}

pub(in crate::ir::lower) fn is_scope_exit_instruction(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Return
            | Instruction::ReturnOutcomeSuccess
            | Instruction::ReturnOptionalNone
            | Instruction::ReturnFallibleFailure { .. }
            | Instruction::TailCall { .. }
    )
}

pub(in crate::ir::lower) fn mark_pending_aggregate_drops(context: &mut LoweringContext) {
    let pending = context.pending_aggregate_drops();
    for drop_ in &pending {
        context.mark_aggregate_local_dropped(&drop_.name);
    }
}

pub(in crate::ir::lower) fn lower_pending_aggregate_drop(
    drop_: &PendingAggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match &drop_.obligation {
        DropObligation::Inactive => Ok(Vec::new()),
        DropObligation::Complete => lower_aggregate_drop_instructions(
            &drop_.name,
            drop_.slot_index,
            drop_.layout,
            &drop_.drop_kind,
            context,
        ),
        DropObligation::ArrayPrefix {
            initialized,
            elements,
        } => lower_array_prefix_drop_instructions(
            &drop_.name,
            AggregateLocation::Slot(drop_.slot_index),
            0,
            &drop_.drop_kind,
            *initialized,
            elements,
            context,
        ),
        DropObligation::StructFields { fields } => lower_struct_fields_drop_instructions(
            &drop_.name,
            AggregateLocation::Slot(drop_.slot_index),
            0,
            &drop_.drop_kind,
            fields,
            context,
        ),
        DropObligation::PayloadFields { tag, fields } => lower_payload_fields_drop_instructions(
            &drop_.name,
            AggregateLocation::Slot(drop_.slot_index),
            0,
            &drop_.drop_kind,
            *tag,
            fields,
            context,
        ),
    }
}

pub(in crate::ir::lower) fn mark_explicit_moves_in_block(
    block: &Block,
    context: &mut LoweringContext,
) {
    for statement in &block.statements {
        mark_lowered_statement_aggregate_uses(statement, context);
    }
    if let Some(result) = &block.result {
        mark_explicit_moves_in_expression(result, context);
    }
}

pub(in crate::ir::lower) fn expression_contains_explicit_aggregate_move_matching(
    expression: &Expr,
    context: &LoweringContext,
    matches_move: &impl Fn(&str, &LoweringContext) -> bool,
) -> bool {
    match expression {
        Expr::Closure(closure) => closure.captures.iter().any(|capture| {
            capture.mode == crate::ast::ClosureCaptureMode::Move
                && matches_move(&capture.name, context)
        }),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = unwrap_group(&unary.operand) {
                matches_move(&identifier.name, context)
            } else {
                expression_contains_explicit_aggregate_move_matching(
                    &unary.operand,
                    context,
                    matches_move,
                )
            }
        }
        Expr::ArrayLiteral(literal) => literal.elements.iter().any(|element| {
            expression_contains_explicit_aggregate_move_matching(element, context, matches_move)
        }),
        Expr::TypedSequenceLiteral(literal) => {
            literal.elements.iter().any(|element| {
                expression_contains_explicit_aggregate_move_matching(element, context, matches_move)
            }) || literal.using.as_ref().is_some_and(|using| {
                expression_contains_explicit_aggregate_move_matching(
                    &using.allocator,
                    context,
                    matches_move,
                )
            })
        }
        Expr::TypedStringLiteral(literal) => literal.using.as_ref().is_some_and(|using| {
            expression_contains_explicit_aggregate_move_matching(
                &using.allocator,
                context,
                matches_move,
            )
        }),
        Expr::StructLiteral(literal) => literal.fields.iter().any(|field| {
            expression_contains_explicit_aggregate_move_matching(
                &field.value,
                context,
                matches_move,
            )
        }),
        Expr::Propagate(propagation) => expression_contains_explicit_aggregate_move_matching(
            &propagation.expression,
            context,
            matches_move,
        ),
        Expr::Force(force) => expression_contains_explicit_aggregate_move_matching(
            &force.expression,
            context,
            matches_move,
        ),
        Expr::Catch(catch) => expression_contains_explicit_aggregate_move_matching(
            &catch.expression,
            context,
            matches_move,
        ),
        Expr::Borrow(borrow) => expression_contains_explicit_aggregate_move_matching(
            &borrow.expression,
            context,
            matches_move,
        ),
        Expr::Unary(unary) => expression_contains_explicit_aggregate_move_matching(
            &unary.operand,
            context,
            matches_move,
        ),
        Expr::Binary(binary) => {
            expression_contains_explicit_aggregate_move_matching(
                &binary.left,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &binary.right,
                context,
                matches_move,
            )
        }
        Expr::TypeConversion(conversion) => expression_contains_explicit_aggregate_move_matching(
            &conversion.expression,
            context,
            matches_move,
        ),
        Expr::Call(call) => {
            expression_contains_explicit_aggregate_move_matching(
                &call.callee,
                context,
                matches_move,
            ) || call.arguments.iter().any(|argument| {
                expression_contains_explicit_aggregate_move_matching(
                    argument,
                    context,
                    matches_move,
                )
            })
        }
        Expr::Member(member) => expression_contains_explicit_aggregate_move_matching(
            &member.object,
            context,
            matches_move,
        ),
        Expr::Index(index) => {
            expression_contains_explicit_aggregate_move_matching(
                &index.object,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &index.index,
                context,
                matches_move,
            )
        }
        Expr::Group(group) => expression_contains_explicit_aggregate_move_matching(
            &group.expression,
            context,
            matches_move,
        ),
        Expr::Otherwise(otherwise) => {
            expression_contains_explicit_aggregate_move_matching(
                &otherwise.value,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &otherwise.fallback,
                context,
                matches_move,
            )
        }
        Expr::If(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.condition,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.then_block,
                context,
                matches_move,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_contains_explicit_aggregate_move_matching(block, context, matches_move)
            })
        }
        Expr::IfIs(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.then_block,
                context,
                matches_move,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_contains_explicit_aggregate_move_matching(block, context, matches_move)
            })
        }
        Expr::Match(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                matches_move,
            ) || statement.arms.iter().any(|arm| {
                block_contains_explicit_aggregate_move_matching(&arm.body, context, matches_move)
            }) || statement.wildcard_arm.as_ref().is_some_and(|arm| {
                block_contains_explicit_aggregate_move_matching(&arm.body, context, matches_move)
            })
        }
        Expr::InterpolatedString(interpolated) => interpolated.parts.iter().any(|part| {
            if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                expression_contains_explicit_aggregate_move_matching(
                    &part.expression,
                    context,
                    matches_move,
                )
            } else {
                false
            }
        }),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => false,
    }
}

pub(in crate::ir::lower) fn block_contains_explicit_aggregate_move_matching(
    block: &Block,
    context: &LoweringContext,
    matches_move: &impl Fn(&str, &LoweringContext) -> bool,
) -> bool {
    block.statements.iter().any(|statement| {
        statement_contains_explicit_aggregate_move_matching(statement, context, matches_move)
    }) || block.result.as_ref().is_some_and(|result| {
        expression_contains_explicit_aggregate_move_matching(result, context, matches_move)
    })
}

pub(in crate::ir::lower) fn statement_contains_explicit_aggregate_move_matching(
    statement: &Stmt,
    context: &LoweringContext,
    matches_move: &impl Fn(&str, &LoweringContext) -> bool,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(statement) => statement.expression.as_ref().is_some_and(|expression| {
            expression_contains_explicit_aggregate_move_matching(expression, context, matches_move)
        }),
        Stmt::Binding(statement) => expression_contains_explicit_aggregate_move_matching(
            &statement.initializer,
            context,
            matches_move,
        ),
        Stmt::Assignment(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.target,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &statement.value,
                context,
                matches_move,
            )
        }
        Stmt::If(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.condition,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.then_block,
                context,
                matches_move,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_contains_explicit_aggregate_move_matching(block, context, matches_move)
            })
        }
        Stmt::IfIs(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.then_block,
                context,
                matches_move,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_contains_explicit_aggregate_move_matching(block, context, matches_move)
            })
        }
        Stmt::Switch(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                matches_move,
            ) || statement.arms.iter().any(|arm| {
                block_contains_explicit_aggregate_move_matching(&arm.body, context, matches_move)
            }) || statement.wildcard_arm.as_ref().is_some_and(|arm| {
                block_contains_explicit_aggregate_move_matching(&arm.body, context, matches_move)
            })
        }
        Stmt::ForRange(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.start,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &statement.end,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.body,
                context,
                matches_move,
            )
        }
        Stmt::CollectionFor(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.source,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.body,
                context,
                matches_move,
            )
        }
        Stmt::LiteralPackFor(statement) => {
            block_contains_explicit_aggregate_move_matching(&statement.body, context, matches_move)
        }
        Stmt::While(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.condition,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.body,
                context,
                matches_move,
            )
        }
        Stmt::Loop(statement) => {
            block_contains_explicit_aggregate_move_matching(&statement.body, context, matches_move)
        }
        Stmt::Region(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.allocator,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.body,
                context,
                matches_move,
            )
        }
        Stmt::Expression(statement) => expression_contains_explicit_aggregate_move_matching(
            &statement.expression,
            context,
            matches_move,
        ),
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => false,
    }
}

pub(in crate::ir::lower) fn drop_parameter_matches_local(
    parameter_type: &Type,
    layout: crate::abi::ValueLayout,
) -> bool {
    let Type::Borrow {
        is_readwrite: true,
        inner,
    } = parameter_type
    else {
        return false;
    };

    match inner.as_ref() {
        Type::Aggregate {
            layout: parameter_layout,
        }
        | Type::DirectAggregate {
            layout: parameter_layout,
            ..
        } => *parameter_layout == layout,
        _ => false,
    }
}

pub(in crate::ir::lower) fn unsupported_drop_statement_diagnostic(name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        format!("native lowering cannot lower drop statement for binding `{name}`"),
    )]
}

pub(in crate::ir::lower) fn lower_drop_statement(
    statement: &DropStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(local) = context.aggregate_local(&statement.name) else {
        return Err(unsupported_drop_statement_diagnostic(&statement.name));
    };
    let Some(drop_kind) = local.drop_kind else {
        context.mark_aggregate_local_dropped(&statement.name);
        return Ok(Vec::new());
    };

    context.mark_aggregate_local_dropped(&statement.name);
    lower_aggregate_drop_instructions(
        &statement.name,
        local.slot_index,
        local.layout,
        &drop_kind,
        context,
    )
}

pub(in crate::ir::lower) fn lower_never_expression_with_scope_drops(
    expression: &Expr,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(instructions) = lower_never_return_expression(expression, context)? else {
        return Ok(None);
    };
    mark_explicit_moves_in_expression(expression, context);
    append_scope_end_drops_before_exit(instructions, context).map(Some)
}

pub(in crate::ir::lower) fn append_scope_end_drops_before_exit(
    mut instructions: Vec<Instruction>,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(return_index) = instructions.iter().rposition(is_scope_exit_instruction) else {
        return Ok(instructions);
    };
    let drops = lower_scope_end_drop_instructions(context)?;
    instructions.splice(return_index..return_index, drops);
    mark_pending_aggregate_drops(context);
    Ok(instructions)
}

pub(in crate::ir::lower) fn replacement_drop_for_aggregate_slot(
    slot_index: usize,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(drop_) = context.pending_aggregate_drop_by_slot(slot_index) else {
        return Ok(Vec::new());
    };
    lower_pending_aggregate_drop(&drop_, context)
}

pub(in crate::ir::lower) fn lower_scope_end_drops_for_locals_since(
    context: &mut LoweringContext,
    local_mark: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let pending = context.pending_aggregate_drops_since(local_mark);
    let mut instructions = Vec::new();
    for drop_ in &pending {
        instructions.extend(lower_pending_aggregate_drop(drop_, context)?);
    }
    for drop_ in &pending {
        context.mark_aggregate_local_dropped(&drop_.name);
    }
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_aggregate_drop_instructions(
    name: &str,
    slot_index: usize,
    layout: ValueLayout,
    drop_kind: &AggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_drop_instructions_at_root_location(
        name,
        AggregateLocation::Slot(slot_index),
        layout,
        drop_kind,
        context,
    )
}

pub(in crate::ir::lower) fn mark_lowered_statement_aggregate_uses(
    statement: &Stmt,
    context: &mut LoweringContext,
) {
    match statement {
        Stmt::Binding(statement) => {
            mark_explicit_moves_in_expression(&statement.initializer, context);
        }
        Stmt::Assignment(statement) => {
            if let Expr::Identifier(identifier) = unwrap_group(&statement.target) {
                context.mark_aggregate_local_initialized(&identifier.name);
            }
            mark_explicit_moves_in_expression(&statement.value, context);
        }
        Stmt::Expression(statement) => {
            mark_explicit_moves_in_expression(&statement.expression, context);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                mark_explicit_moves_in_expression(expression, context);
            }
        }
        Stmt::CollectionFor(statement) => {
            mark_explicit_moves_in_expression(&statement.source, context);
        }
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Drop(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::LiteralPackFor(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Region(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => {}
    }
}

pub(in crate::ir::lower) fn mark_explicit_moves_in_expression(
    expression: &Expr,
    context: &mut LoweringContext,
) {
    match expression {
        Expr::Closure(closure) => {
            for capture in &closure.captures {
                if capture.mode == crate::ast::ClosureCaptureMode::Move {
                    context.mark_aggregate_local_moved(&capture.name);
                }
            }
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = unwrap_group(&unary.operand) {
                context.mark_aggregate_local_moved(&identifier.name);
            } else {
                mark_explicit_moves_in_expression(&unary.operand, context);
            }
        }
        Expr::ArrayLiteral(literal) => {
            for element in &literal.elements {
                mark_explicit_moves_in_expression(element, context);
            }
        }
        Expr::TypedSequenceLiteral(literal) => {
            for element in &literal.elements {
                mark_explicit_moves_in_expression(element, context);
            }
            if let Some(using) = &literal.using {
                mark_explicit_moves_in_expression(&using.allocator, context);
            }
        }
        Expr::TypedStringLiteral(literal) => {
            if let Some(using) = &literal.using {
                mark_explicit_moves_in_expression(&using.allocator, context);
            }
        }
        Expr::StructLiteral(literal) => {
            for field in &literal.fields {
                mark_explicit_moves_in_expression(&field.value, context);
            }
        }
        Expr::Propagate(propagation) => {
            mark_explicit_moves_in_expression(&propagation.expression, context);
        }
        Expr::Force(force) => {
            mark_explicit_moves_in_expression(&force.expression, context);
        }
        Expr::Catch(catch) => {
            mark_explicit_moves_in_expression(&catch.expression, context);
        }
        Expr::Borrow(borrow) => {
            mark_explicit_moves_in_expression(&borrow.expression, context);
        }
        Expr::Unary(unary) => {
            mark_explicit_moves_in_expression(&unary.operand, context);
        }
        Expr::Binary(binary) => {
            mark_explicit_moves_in_expression(&binary.left, context);
            mark_explicit_moves_in_expression(&binary.right, context);
        }
        Expr::TypeConversion(conversion) => {
            mark_explicit_moves_in_expression(&conversion.expression, context);
        }
        Expr::Call(call) => {
            if let Expr::Member(member) = unwrap_group(&call.callee)
                && context.method_call_receiver_kind(member.member_span)
                    == Some(crate::typecheck::TypecheckMethodReceiverKind::Owned)
                && let Expr::Identifier(identifier) = unwrap_group(&member.object)
            {
                context.mark_aggregate_local_moved(&identifier.name);
            }
            mark_explicit_moves_in_expression(&call.callee, context);
            for argument in &call.arguments {
                mark_explicit_moves_in_expression(argument, context);
            }
        }
        Expr::Member(member) => {
            mark_explicit_moves_in_expression(&member.object, context);
        }
        Expr::Index(index) => {
            mark_explicit_moves_in_expression(&index.object, context);
            mark_explicit_moves_in_expression(&index.index, context);
        }
        Expr::Group(group) => {
            mark_explicit_moves_in_expression(&group.expression, context);
        }
        Expr::Otherwise(otherwise) => {
            mark_explicit_moves_in_expression(&otherwise.value, context);
            mark_explicit_moves_in_block(&otherwise.fallback, context);
        }
        Expr::If(statement) => {
            mark_explicit_moves_in_expression(&statement.condition, context);
            mark_explicit_moves_in_block(&statement.then_block, context);
            if let Some(block) = &statement.else_block {
                mark_explicit_moves_in_block(block, context);
            }
        }
        Expr::IfIs(statement) => {
            mark_explicit_moves_in_expression(&statement.expression, context);
            mark_explicit_moves_in_block(&statement.then_block, context);
            if let Some(block) = &statement.else_block {
                mark_explicit_moves_in_block(block, context);
            }
        }
        Expr::Match(statement) => {
            mark_explicit_moves_in_expression(&statement.expression, context);
            for arm in &statement.arms {
                mark_explicit_moves_in_block(&arm.body, context);
            }
            if let Some(arm) = &statement.wildcard_arm {
                mark_explicit_moves_in_block(&arm.body, context);
            }
        }
        Expr::InterpolatedString(interpolated) => {
            for part in &interpolated.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    mark_explicit_moves_in_expression(&part.expression, context);
                }
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

pub(in crate::ir::lower) fn expression_contains_explicit_aggregate_move(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_contains_explicit_aggregate_move_matching(expression, context, &|name, context| {
        context.aggregate_local(name).is_some()
    })
}

pub(in crate::ir::lower) fn expression_contains_explicit_aggregate_move_outside(
    expression: &Expr,
    context: &LoweringContext,
    local_mark: usize,
) -> bool {
    expression_contains_explicit_aggregate_move_matching(expression, context, &|name, context| {
        context.aggregate_local(name).is_some()
            && !context.aggregate_local_defined_since(name, local_mark)
    })
}

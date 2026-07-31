use super::*;

pub(super) fn lower_scope_end_drop_instructions(
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let pending = context.pending_aggregate_drops();
    let mut instructions = Vec::new();
    for drop_ in &pending {
        instructions.extend(lower_pending_aggregate_drop(drop_, context)?);
    }
    Ok(instructions)
}

pub(super) fn is_scope_exit_instruction(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Return
            | Instruction::ReturnFallibleSuccess
            | Instruction::ReturnOptionalNone
            | Instruction::ReturnFallibleFailure { .. }
            | Instruction::TailCall { .. }
    )
}

pub(super) fn mark_pending_aggregate_drops(context: &mut LoweringContext) {
    let pending = context.pending_aggregate_drops();
    for drop_ in &pending {
        context.mark_aggregate_local_dropped(&drop_.name);
    }
}

pub(super) fn lower_pending_aggregate_drop(
    drop_: &PendingAggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_drop_instructions(
        &drop_.name,
        drop_.slot_index,
        drop_.layout,
        &drop_.drop_kind,
        context,
    )
}

pub(super) fn mark_explicit_moves_in_block(block: &Block, context: &mut LoweringContext) {
    for statement in &block.statements {
        mark_lowered_statement_aggregate_uses(statement, context);
    }
    if let Some(result) = &block.result {
        mark_explicit_moves_in_expression(result, context);
    }
}

pub(super) fn expression_contains_explicit_aggregate_move_matching(
    expression: &Expr,
    context: &LoweringContext,
    matches_move: &impl Fn(&str, &LoweringContext) -> bool,
) -> bool {
    match expression {
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

pub(super) fn block_contains_explicit_aggregate_move_matching(
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

pub(super) fn statement_contains_explicit_aggregate_move_matching(
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
        Stmt::Expression(statement) => expression_contains_explicit_aggregate_move_matching(
            &statement.expression,
            context,
            matches_move,
        ),
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => false,
    }
}

pub(super) fn drop_parameter_matches_local(
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

pub(super) fn unsupported_drop_statement_diagnostic(name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        format!("IR v0 cannot lower drop statement for binding `{name}`"),
    )]
}

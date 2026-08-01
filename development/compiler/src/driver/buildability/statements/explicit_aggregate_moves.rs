use super::*;

#[derive(Clone, Copy)]
pub(in crate::driver::buildability) enum ExplicitAggregateMoveScope<'a> {
    Any,
    OutsideLocals(&'a HashSet<String>),
}

pub(in crate::driver::buildability) fn expression_explicit_aggregate_move_span(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ByteSpan> {
    explicit_aggregate_move_span_in_expression(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        ExplicitAggregateMoveScope::Any,
    )
}

pub(in crate::driver::buildability) fn expression_explicit_outer_aggregate_move_span(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    local_bindings: &HashSet<String>,
) -> Option<ByteSpan> {
    explicit_aggregate_move_span_in_expression(
        expression,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings),
    )
}

pub(in crate::driver::buildability) fn explicit_aggregate_move_span_in_expression(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match expression {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = unwrap_group_expr(&unary.operand) {
                explicit_aggregate_move_matches_identifier(
                    identifier,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
                .then_some(unary.span)
            } else {
                explicit_aggregate_move_span_in_expression(
                    &unary.operand,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            }
        }
        Expr::ArrayLiteral(literal) => literal.elements.iter().find_map(|element| {
            explicit_aggregate_move_span_in_expression(
                element,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::StructLiteral(literal) => literal.fields.iter().find_map(|field| {
            explicit_aggregate_move_span_in_expression(
                &field.value,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::Propagate(propagation) => explicit_aggregate_move_span_in_expression(
            &propagation.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Force(force) => explicit_aggregate_move_span_in_expression(
            &force.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Catch(catch) => explicit_aggregate_move_span_in_expression(
            &catch.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Borrow(borrow) => explicit_aggregate_move_span_in_expression(
            &borrow.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Unary(unary) => explicit_aggregate_move_span_in_expression(
            &unary.operand,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Binary(binary) => explicit_aggregate_move_span_in_expression(
            &binary.left,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &binary.right,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::TypeConversion(conversion) => explicit_aggregate_move_span_in_expression(
            &conversion.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Call(call) => explicit_aggregate_move_span_in_expression(
            &call.callee,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            call.arguments.iter().find_map(|argument| {
                explicit_aggregate_move_span_in_expression(
                    argument,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::Member(member) => explicit_aggregate_move_span_in_expression(
            &member.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Index(index) => explicit_aggregate_move_span_in_expression(
            &index.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &index.index,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::Group(group) => explicit_aggregate_move_span_in_expression(
            &group.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Expr::Otherwise(expression) => explicit_aggregate_move_span_in_expression(
            &expression.value,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &expression.fallback,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Expr::If(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.then_block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::IfIs(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_payload_block(
                &statement.then_block,
                statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding_name()),
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::Match(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            statement.arms.iter().find_map(|arm| {
                explicit_aggregate_move_span_in_payload_block(
                    &arm.body,
                    arm.payload
                        .as_ref()
                        .and_then(|payload| payload.binding_name()),
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        })
        .or_else(|| {
            statement.wildcard_arm.as_ref().and_then(|arm| {
                explicit_aggregate_move_span_in_block(
                    &arm.body,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Expr::InterpolatedString(interpolated) => interpolated.parts.iter().find_map(|part| {
            if let InterpolatedStringPart::Expression(part) = part {
                explicit_aggregate_move_span_in_expression(
                    &part.expression,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            } else {
                None
            }
        }),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

pub(in crate::driver::buildability) fn explicit_aggregate_move_span_in_block(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match scope {
        ExplicitAggregateMoveScope::Any => block
            .statements
            .iter()
            .find_map(|statement| {
                explicit_aggregate_move_span_in_statement(
                    statement,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
            .or_else(|| {
                block.result.as_ref().and_then(|result| {
                    explicit_aggregate_move_span_in_expression(
                        result,
                        resolved,
                        resolved_sources,
                        typecheck_facts,
                        generic_substitutions,
                        scope,
                    )
                })
            }),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            let mut nested_locals = local_bindings.clone();
            for statement in &block.statements {
                let span = explicit_aggregate_move_span_in_statement(
                    statement,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
                );
                if span.is_some() {
                    return span;
                }
                if let Stmt::Binding(statement) = statement {
                    nested_locals.insert(statement.name.clone());
                }
            }
            block.result.as_ref().and_then(|result| {
                explicit_aggregate_move_span_in_expression(
                    result,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
                )
            })
        }
    }
}

pub(in crate::driver::buildability) fn explicit_aggregate_move_span_in_statement(
    statement: &Stmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match statement {
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Drop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => None,
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            explicit_aggregate_move_span_in_expression(
                expression,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::Binding(statement) => explicit_aggregate_move_span_in_expression(
            &statement.initializer,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Stmt::Assignment(statement) => explicit_aggregate_move_span_in_expression(
            &statement.target,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &statement.value,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::If(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.then_block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::IfIs(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_payload_block(
                &statement.then_block,
                statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding_name()),
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            statement.else_block.as_ref().and_then(|block| {
                explicit_aggregate_move_span_in_block(
                    block,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::Switch(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            statement.arms.iter().find_map(|arm| {
                explicit_aggregate_move_span_in_payload_block(
                    &arm.body,
                    arm.payload
                        .as_ref()
                        .and_then(|payload| payload.binding_name()),
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        })
        .or_else(|| {
            statement.wildcard_arm.as_ref().and_then(|arm| {
                explicit_aggregate_move_span_in_block(
                    &arm.body,
                    resolved,
                    resolved_sources,
                    typecheck_facts,
                    generic_substitutions,
                    scope,
                )
            })
        }),
        Stmt::ForRange(statement) => explicit_aggregate_move_span_in_expression(
            &statement.start,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_expression(
                &statement.end,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        })
        .or_else(|| {
            explicit_aggregate_move_span_in_for_range_body(
                statement,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::While(statement) => explicit_aggregate_move_span_in_expression(
            &statement.condition,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.body,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::Loop(statement) => explicit_aggregate_move_span_in_block(
            &statement.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        Stmt::Region(statement) => explicit_aggregate_move_span_in_expression(
            &statement.allocator,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        )
        .or_else(|| {
            explicit_aggregate_move_span_in_block(
                &statement.body,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                scope,
            )
        }),
        Stmt::Expression(statement) => explicit_aggregate_move_span_in_expression(
            &statement.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
    }
}

pub(in crate::driver::buildability) fn explicit_aggregate_move_span_in_for_range_body(
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match scope {
        ExplicitAggregateMoveScope::Any => explicit_aggregate_move_span_in_block(
            &statement.body,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            let mut body_locals = local_bindings.clone();
            body_locals.insert(statement.name.clone());
            explicit_aggregate_move_span_in_block(
                &statement.body,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                ExplicitAggregateMoveScope::OutsideLocals(&body_locals),
            )
        }
    }
}

pub(in crate::driver::buildability) fn explicit_aggregate_move_span_in_payload_block(
    block: &Block,
    payload_name: Option<&str>,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> Option<ByteSpan> {
    match (scope, payload_name) {
        (ExplicitAggregateMoveScope::OutsideLocals(local_bindings), Some(payload_name)) => {
            let mut nested_locals = local_bindings.clone();
            nested_locals.insert(payload_name.to_owned());
            explicit_aggregate_move_span_in_block(
                block,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                ExplicitAggregateMoveScope::OutsideLocals(&nested_locals),
            )
        }
        _ => explicit_aggregate_move_span_in_block(
            block,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
            scope,
        ),
    }
}

pub(in crate::driver::buildability) fn explicit_aggregate_move_matches_identifier(
    identifier: &IdentifierExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    scope: ExplicitAggregateMoveScope<'_>,
) -> bool {
    match scope {
        ExplicitAggregateMoveScope::Any => identifier_is_aggregate_for_buildability(
            identifier,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        ExplicitAggregateMoveScope::OutsideLocals(local_bindings) => {
            identifier_is_outer_aggregate_for_buildability(
                identifier,
                resolved,
                resolved_sources,
                typecheck_facts,
                generic_substitutions,
                local_bindings,
            )
        }
    }
}

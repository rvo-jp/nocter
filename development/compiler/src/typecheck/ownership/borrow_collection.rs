use super::*;

pub(super) fn collect_direct_borrow_expressions_in_statement(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrows: &mut Vec<DirectBorrowSource>,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_direct_borrow_expressions(expression, resolved, environment, borrows);
            }
        }
        Stmt::Binding(statement) => {
            collect_direct_borrow_expressions(
                &statement.initializer,
                resolved,
                environment,
                borrows,
            );
        }
        Stmt::Assignment(statement) => {
            collect_direct_borrow_expressions(&statement.target, resolved, environment, borrows);
            collect_direct_borrow_expressions(&statement.value, resolved, environment, borrows);
        }
        Stmt::If(statement) => {
            collect_direct_borrow_expressions(&statement.condition, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &statement.then_block,
                resolved,
                environment,
                borrows,
            );
            if let Some(else_block) = &statement.else_block {
                collect_direct_borrow_expressions_in_block(
                    else_block,
                    resolved,
                    environment,
                    borrows,
                );
            }
        }
        Stmt::IfIs(statement) => {
            collect_direct_borrow_expressions(
                &statement.expression,
                resolved,
                environment,
                borrows,
            );
            collect_direct_borrow_expressions_in_block(
                &statement.then_block,
                resolved,
                environment,
                borrows,
            );
            if let Some(else_block) = &statement.else_block {
                collect_direct_borrow_expressions_in_block(
                    else_block,
                    resolved,
                    environment,
                    borrows,
                );
            }
        }
        Stmt::Switch(statement) => {
            collect_direct_borrow_expressions(
                &statement.expression,
                resolved,
                environment,
                borrows,
            );
            for arm in &statement.arms {
                collect_direct_borrow_expressions_in_block(
                    &arm.body,
                    resolved,
                    environment,
                    borrows,
                );
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                collect_direct_borrow_expressions_in_block(
                    &wildcard_arm.body,
                    resolved,
                    environment,
                    borrows,
                );
            }
        }
        Stmt::ForRange(statement) => {
            collect_direct_borrow_expressions(&statement.start, resolved, environment, borrows);
            collect_direct_borrow_expressions(&statement.end, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &statement.body,
                resolved,
                environment,
                borrows,
            );
        }
        Stmt::CollectionFor(statement) => {
            collect_direct_borrow_expressions(&statement.source, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &statement.body,
                resolved,
                environment,
                borrows,
            );
        }
        Stmt::LiteralPackFor(statement) => collect_direct_borrow_expressions_in_block(
            &statement.body,
            resolved,
            environment,
            borrows,
        ),
        Stmt::While(statement) => {
            collect_direct_borrow_expressions(&statement.condition, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &statement.body,
                resolved,
                environment,
                borrows,
            );
        }
        Stmt::Loop(statement) => collect_direct_borrow_expressions_in_block(
            &statement.body,
            resolved,
            environment,
            borrows,
        ),
        Stmt::Region(statement) => {
            collect_direct_borrow_expressions(&statement.allocator, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &statement.body,
                resolved,
                environment,
                borrows,
            );
        }
        Stmt::Expression(statement) => {
            collect_direct_borrow_expressions(&statement.expression, resolved, environment, borrows)
        }
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_direct_borrow_expressions_in_block(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrows: &mut Vec<DirectBorrowSource>,
) {
    for statement in &block.statements {
        collect_direct_borrow_expressions_in_statement(statement, resolved, environment, borrows);
    }
    if let Some(result) = &block.result {
        collect_direct_borrow_expressions(result, resolved, environment, borrows);
    }
}

pub(super) fn collect_direct_borrow_expressions(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrows: &mut Vec<DirectBorrowSource>,
) {
    if let Some(spread) = crate::typecheck::sequence_spread(expression)
        && !matches!(
            spread.operand.without_groups(),
            Expr::Borrow(_)
                | Expr::Unary(crate::ast::UnaryExpr {
                    operator: UnaryOperator::Move,
                    ..
                })
        )
        && let Some(source) = expression_place(&spread.operand)
    {
        borrows.push(DirectBorrowSource {
            source,
            source_span: spread.operand.span(),
            is_readwrite: false,
        });
    }
    if let Some(source) = direct_borrow_source(expression) {
        borrows.push(source);
    }

    match expression {
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_direct_borrow_expressions(element, resolved, environment, borrows);
            }
            if let Some(using) = &expression.using {
                collect_direct_borrow_expressions(&using.allocator, resolved, environment, borrows);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_direct_borrow_expressions(&using.allocator, resolved, environment, borrows);
            }
        }
        Expr::Propagate(expression) => collect_direct_borrow_expressions(
            &expression.expression,
            resolved,
            environment,
            borrows,
        ),
        Expr::Force(expression) => collect_direct_borrow_expressions(
            &expression.expression,
            resolved,
            environment,
            borrows,
        ),
        Expr::Catch(expression) => {
            collect_direct_borrow_expressions(
                &expression.expression,
                resolved,
                environment,
                borrows,
            );
            collect_direct_borrow_expressions_in_block(
                &expression.catch_block,
                resolved,
                environment,
                borrows,
            );
        }
        Expr::Borrow(expression) => collect_direct_borrow_expressions(
            &expression.expression,
            resolved,
            environment,
            borrows,
        ),
        Expr::Binary(expression) => {
            collect_direct_borrow_expressions(&expression.left, resolved, environment, borrows);
            collect_direct_borrow_expressions(&expression.right, resolved, environment, borrows);
        }
        Expr::Unary(expression) => {
            collect_direct_borrow_expressions(&expression.operand, resolved, environment, borrows)
        }
        Expr::TypeConversion(expression) => {
            collect_direct_borrow_expressions(
                &expression.expression,
                resolved,
                environment,
                borrows,
            );
        }
        Expr::Call(expression) => {
            if let Some(source) = method_borrow_receiver_source(expression, resolved, environment) {
                borrows.push(source);
            }
            collect_direct_borrow_expressions(&expression.callee, resolved, environment, borrows);
            for argument in &expression.arguments {
                collect_direct_borrow_expressions(argument, resolved, environment, borrows);
            }
        }
        Expr::Member(expression) => {
            collect_direct_borrow_expressions(&expression.object, resolved, environment, borrows)
        }
        Expr::Index(expression) => {
            collect_direct_borrow_expressions(&expression.object, resolved, environment, borrows);
            collect_direct_borrow_expressions(&expression.index, resolved, environment, borrows);
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_direct_borrow_expressions(element, resolved, environment, borrows);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_direct_borrow_expressions(&field.value, resolved, environment, borrows);
            }
        }
        Expr::Group(expression) => collect_direct_borrow_expressions(
            &expression.expression,
            resolved,
            environment,
            borrows,
        ),
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    collect_direct_borrow_expressions(
                        &part.expression,
                        resolved,
                        environment,
                        borrows,
                    );
                }
            }
        }
        Expr::Otherwise(expression) => {
            collect_direct_borrow_expressions(&expression.value, resolved, environment, borrows);
            collect_direct_borrow_expressions_in_block(
                &expression.fallback,
                resolved,
                environment,
                borrows,
            );
        }
        Expr::If(expression) => {
            collect_direct_borrow_expressions(
                &expression.condition,
                resolved,
                environment,
                borrows,
            );
            collect_direct_borrow_expressions_in_block(
                &expression.then_block,
                resolved,
                environment,
                borrows,
            );
            if let Some(block) = &expression.else_block {
                collect_direct_borrow_expressions_in_block(block, resolved, environment, borrows);
            }
        }
        Expr::IfIs(expression) => {
            collect_direct_borrow_expressions(
                &expression.expression,
                resolved,
                environment,
                borrows,
            );
            collect_direct_borrow_expressions_in_block(
                &expression.then_block,
                resolved,
                environment,
                borrows,
            );
            if let Some(block) = &expression.else_block {
                collect_direct_borrow_expressions_in_block(block, resolved, environment, borrows);
            }
        }
        Expr::Match(expression) => {
            collect_direct_borrow_expressions(
                &expression.expression,
                resolved,
                environment,
                borrows,
            );
            for arm in &expression.arms {
                collect_direct_borrow_expressions_in_block(
                    &arm.body,
                    resolved,
                    environment,
                    borrows,
                );
            }
            if let Some(arm) = &expression.wildcard_arm {
                collect_direct_borrow_expressions_in_block(
                    &arm.body,
                    resolved,
                    environment,
                    borrows,
                );
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

pub(super) fn direct_borrow_source(expression: &Expr) -> Option<DirectBorrowSource> {
    let Expr::Borrow(borrow) = unwrap_group(expression) else {
        return None;
    };
    let source = expression_place(&borrow.expression)?;
    Some(DirectBorrowSource {
        source,
        source_span: borrow.expression.span(),
        is_readwrite: borrow.is_readwrite,
    })
}

pub(super) fn returned_borrow_sources(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
    active_borrows: &[ActiveBorrow],
) -> Vec<DirectBorrowSource> {
    if !type_contains_borrow_like(
        &expression_type(expression, resolved, environment),
        resolved,
    ) {
        return Vec::new();
    }
    if let Some(source) = direct_borrow_source(expression) {
        return vec![source];
    }

    match unwrap_group(expression) {
        Expr::Identifier(identifier) => {
            return active_borrows
                .iter()
                .filter(|borrow| borrow.borrow_name == identifier.name)
                .map(|borrow| DirectBorrowSource {
                    source: borrow.source.clone(),
                    source_span: identifier.span,
                    is_readwrite: borrow.is_readwrite,
                })
                .collect();
        }
        Expr::Otherwise(otherwise) => {
            let mut sources = returned_borrow_sources(
                &otherwise.value,
                resolved,
                environment,
                summaries,
                active_borrows,
            );
            if let Some(result) = &otherwise.fallback.result {
                sources.extend(returned_borrow_sources(
                    result,
                    resolved,
                    environment,
                    summaries,
                    active_borrows,
                ));
            }
            return sources;
        }
        Expr::Propagate(propagation) => {
            return returned_borrow_sources(
                &propagation.expression,
                resolved,
                environment,
                summaries,
                active_borrows,
            );
        }
        Expr::Force(force) => {
            return returned_borrow_sources(
                &force.expression,
                resolved,
                environment,
                summaries,
                active_borrows,
            );
        }
        Expr::Catch(catch) => {
            let mut sources = returned_borrow_sources(
                &catch.expression,
                resolved,
                environment,
                summaries,
                active_borrows,
            );
            if let Some(result) = &catch.catch_block.result {
                sources.extend(returned_borrow_sources(
                    result,
                    resolved,
                    environment,
                    summaries,
                    active_borrows,
                ));
            }
            return sources;
        }
        Expr::If(if_) => {
            let mut sources = borrow_sources_for_block_result(
                &if_.then_block,
                resolved,
                environment,
                summaries,
                active_borrows,
            );
            if let Some(else_block) = &if_.else_block {
                sources.extend(borrow_sources_for_block_result(
                    else_block,
                    resolved,
                    environment,
                    summaries,
                    active_borrows,
                ));
            }
            return sources;
        }
        _ => {}
    }

    let Expr::Call(call) = unwrap_group(expression) else {
        return Vec::new();
    };
    let Some(signature) = resolved_call_signature(resolved, call, environment) else {
        return Vec::new();
    };
    let Some(declaration) = signature.declaration_span else {
        return Vec::new();
    };
    let Some(result) = summaries.result(CallableId::declared_at(declaration)) else {
        return Vec::new();
    };

    let is_readwrite = returned_type_contains_readwrite_borrow(
        &expression_type(expression, resolved, environment),
        resolved,
    );
    result
        .input_origins()
        .into_iter()
        .filter_map(|input| call_input_expression(input, call, &signature, resolved, environment))
        .flat_map(|input| {
            borrow_sources_for_input(
                input.expression,
                input.is_borrowed,
                is_readwrite,
                resolved,
                environment,
                summaries,
                active_borrows,
            )
        })
        .collect()
}

fn borrow_sources_for_block_result(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
    active_borrows: &[ActiveBorrow],
) -> Vec<DirectBorrowSource> {
    block.result.as_deref().map_or_else(Vec::new, |result| {
        returned_borrow_sources(result, resolved, environment, summaries, active_borrows)
    })
}

struct CallInput<'a> {
    expression: &'a Expr,
    is_borrowed: bool,
}

fn call_input_expression<'a>(
    input: InputId,
    call: &'a crate::ast::CallExpr,
    signature: &crate::typecheck::calls::CheckedCallSignature<'_>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<CallInput<'a>> {
    if signature.kind == crate::typecheck::calls::CheckedCallKind::Method
        && let Some((_, method)) = resolved_method_for_call(resolved, call, environment)
        && InputId::declared_at(method.receiver.name_span) == input
    {
        return method_member_for_call(call).map(|member| CallInput {
            expression: member.object.as_ref(),
            is_borrowed: method.receiver.mode != MethodReceiverMode::Owned,
        });
    }

    signature
        .signature
        .parameters
        .iter()
        .position(|parameter| InputId::declared_at(parameter.name_span) == input)
        .and_then(|index| {
            call.arguments.get(index).map(|expression| CallInput {
                expression,
                is_borrowed: matches!(
                    signature.signature.parameters[index].ty,
                    TypeExpr::Borrow(_)
                ),
            })
        })
}

fn borrow_sources_for_input(
    expression: &Expr,
    input_is_borrowed: bool,
    result_is_readwrite: bool,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
    active_borrows: &[ActiveBorrow],
) -> Vec<DirectBorrowSource> {
    if let Some(mut source) = direct_borrow_source(expression) {
        source.is_readwrite = result_is_readwrite;
        return vec![source];
    }

    if let Expr::Identifier(identifier) = unwrap_group(expression) {
        let aliases = active_borrows
            .iter()
            .filter(|borrow| borrow.borrow_name == identifier.name)
            .map(|borrow| DirectBorrowSource {
                source: borrow.source.clone(),
                source_span: identifier.span,
                is_readwrite: result_is_readwrite,
            })
            .collect::<Vec<_>>();
        if !aliases.is_empty() {
            return aliases;
        }
    }

    if input_is_borrowed
        && !type_contains_borrow_like(
            &expression_type(expression, resolved, environment),
            resolved,
        )
        && let Some(source) = expression_place(expression)
    {
        return vec![DirectBorrowSource {
            source,
            source_span: expression.span(),
            is_readwrite: result_is_readwrite,
        }];
    }

    returned_borrow_sources(expression, resolved, environment, summaries, active_borrows)
}

fn method_borrow_receiver_source(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<DirectBorrowSource> {
    let method = method_member_for_call(call)?;
    let (_, signature) = resolved_method_for_call(resolved, call, environment)?;
    if signature.receiver.mode == MethodReceiverMode::Owned {
        return None;
    }
    let source = expression_place(&method.object)?;
    Some(DirectBorrowSource {
        source,
        source_span: method.object.span(),
        is_readwrite: signature.receiver.mode == MethodReceiverMode::ReadwriteBorrow,
    })
}

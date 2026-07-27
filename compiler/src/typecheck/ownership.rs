use super::bindings::continuing_binding_type;
use super::calls::{method_member_for_call, resolved_method_for_call};
use super::diagnostics::{
    active_borrow_conflict_diagnostic, invalid_drop_target_diagnostic,
    uninitialized_binding_diagnostic,
};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_function,
    environment_for_if_is_binding, environment_for_method, environment_for_parameters_in_impl,
    environment_for_switch_arm,
};
use super::expressions::{collection_builtin_call_type, expression_type};
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::variants::switch_statement_covers_all_variants;
use crate::ast::{
    AstFile, Block, Expr, IdentifierExpr, ImplDecl, ImplMember, Item, Stmt, TypeExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_ownership_states(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let mut environment = environment_for_function(function, resolved);
                let mut ownership = OwnershipState::default();
                ownership.define_parameters(
                    &function.parameters.parameters,
                    &environment,
                    resolved,
                );
                check_block_ownership(
                    sources,
                    &function.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
            Item::Impl(impl_) => {
                check_impl_member_ownership(sources, impl_, resolved, diagnostics);
            }
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Interface(_) => {}
        }
    }
}

fn check_impl_member_ownership(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for member in &impl_.members {
        match member {
            ImplMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut environment = environment_for_method(method, resolved, impl_);
                let mut ownership = OwnershipState::default();
                ownership.define_binding_from_environment(
                    &method.receiver.name,
                    method.receiver.name_span,
                    &environment,
                    resolved,
                );
                ownership.define_parameters(&method.parameters.parameters, &environment, resolved);
                check_block_ownership(
                    sources,
                    body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
            ImplMember::Drop(drop_) => {
                let mut environment = environment_for_parameters_in_impl(
                    std::slice::from_ref(&drop_.binding),
                    resolved,
                    impl_,
                );
                let mut ownership = OwnershipState::default();
                ownership.define_parameters(
                    std::slice::from_ref(&drop_.binding),
                    &environment,
                    resolved,
                );
                check_block_ownership(
                    sources,
                    &drop_.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    &mut ownership,
                );
            }
        }
    }
}

fn check_block_ownership(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) -> FlowState {
    let mut active_borrows: Vec<ActiveBorrow> = Vec::new();
    for (index, statement) in block.statements.iter().enumerate() {
        active_borrows.retain(|borrow| {
            statements_use_identifier_before_terminal(
                &block.statements[index..],
                &borrow.borrow_name,
            ) || block
                .result
                .as_ref()
                .is_some_and(|result| expression_uses_identifier(result, &borrow.borrow_name))
        });
        check_statement_borrow_conflicts(
            sources,
            statement,
            resolved,
            environment,
            &active_borrows,
            diagnostics,
        );

        let flow = check_statement_ownership(
            sources,
            statement,
            resolved,
            diagnostics,
            environment,
            ownership,
        );
        record_statement_borrow(
            statement,
            &block.statements[index + 1..],
            block.result.as_deref(),
            &mut active_borrows,
        );
        if !flow.reaches_end {
            return flow;
        }
    }
    if let Some(result) = &block.result {
        active_borrows.retain(|borrow| expression_uses_identifier(result, &borrow.borrow_name));
        check_expression_borrow_conflicts(
            sources,
            result,
            resolved,
            environment,
            &active_borrows,
            diagnostics,
        );
        check_expression_ownership(
            sources,
            result,
            resolved,
            diagnostics,
            environment,
            ownership,
        );
        if expression_type(result, resolved, environment) == Type::Never {
            return FlowState::terminal();
        }
    }
    FlowState::fallthrough()
}

fn check_statement_borrow_conflicts(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    active_borrows: &[ActiveBorrow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut new_borrows = Vec::new();
    collect_direct_borrow_expressions_in_statement(
        statement,
        resolved,
        environment,
        &mut new_borrows,
    );

    for borrow in active_borrows {
        if let Some(action) =
            statement_conflicting_action(statement, &borrow.source, resolved, environment)
        {
            let action_name = action.place.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action.description,
                action.span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }

        if let Some(new_borrow) = new_borrows
            .iter()
            .find(|new_borrow| new_borrow.source.conflicts_with(&borrow.source))
            && (new_borrow.is_readwrite || borrow.is_readwrite)
        {
            let action = if new_borrow.is_readwrite {
                "create readwrite borrow of"
            } else {
                "create readonly borrow of"
            };
            let action_name = new_borrow.source.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action,
                new_borrow.source_span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }

        if borrow.is_readwrite
            && let Some(action) =
                statement_read_action(statement, &borrow.source, resolved, environment)
        {
            let action_name = action.place.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action.description,
                action.span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }
    }
}

fn check_expression_borrow_conflicts(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    active_borrows: &[ActiveBorrow],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut new_borrows = Vec::new();
    collect_direct_borrow_expressions(expression, resolved, environment, &mut new_borrows);

    for borrow in active_borrows {
        if let Some(action) =
            expression_move_action(expression, &borrow.source, resolved, environment)
        {
            let action_name = action.place.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action.description,
                action.span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }

        if let Some(new_borrow) = new_borrows
            .iter()
            .find(|new_borrow| new_borrow.source.conflicts_with(&borrow.source))
            && (new_borrow.is_readwrite || borrow.is_readwrite)
        {
            let action = if new_borrow.is_readwrite {
                "create readwrite borrow of"
            } else {
                "create readonly borrow of"
            };
            let action_name = new_borrow.source.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action,
                new_borrow.source_span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }

        if borrow.is_readwrite
            && let Some(action) =
                expression_read_action(expression, &borrow.source, resolved, environment)
        {
            let action_name = action.place.display();
            diagnostics.push(active_borrow_conflict_diagnostic(
                sources,
                &action_name,
                action.description,
                action.span,
                &borrow.borrow_name,
                borrow.borrow_span,
                borrow.is_readwrite,
            ));
            return;
        }
    }
}

fn record_statement_borrow(
    statement: &Stmt,
    later_statements: &[Stmt],
    later_result: Option<&Expr>,
    active_borrows: &mut Vec<ActiveBorrow>,
) {
    let Stmt::Binding(binding) = statement else {
        return;
    };
    let Some(source) = direct_borrow_source(&binding.initializer) else {
        return;
    };
    if !statements_use_identifier_before_terminal(later_statements, &binding.name)
        && !later_result.is_some_and(|result| expression_uses_identifier(result, &binding.name))
    {
        return;
    }

    active_borrows.push(ActiveBorrow {
        source: source.source,
        borrow_name: binding.name.clone(),
        borrow_span: binding.name_span,
        is_readwrite: source.is_readwrite,
    });
}

fn statement_conflicting_action(
    statement: &Stmt,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => None,
        Stmt::Drop(statement)
            if BorrowPlace::whole(statement.name.clone()).conflicts_with(source) =>
        {
            Some(BorrowAction {
                place: BorrowPlace::whole(statement.name.clone()),
                span: statement.name_span,
                description: "drop",
            })
        }
        Stmt::Assignment(statement)
            if assignment_target_place(&statement.target)
                .as_ref()
                .is_some_and(|target| target.conflicts_with(source)) =>
        {
            Some(BorrowAction {
                place: assignment_target_place(&statement.target)?,
                span: statement.target.span(),
                description: "assign to",
            })
        }
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            expression_move_action(expression, source, resolved, environment)
        }),
        Stmt::Binding(statement) => {
            expression_move_action(&statement.initializer, source, resolved, environment)
        }
        Stmt::Assignment(statement) => {
            expression_move_action(&statement.value, source, resolved, environment)
        }
        Stmt::If(statement) => {
            expression_move_action(&statement.condition, source, resolved, environment)
                .or_else(|| block_move_action(&statement.then_block, source, resolved, environment))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| block_move_action(block, source, resolved, environment))
                })
        }
        Stmt::IfIs(statement) => {
            expression_move_action(&statement.expression, source, resolved, environment)
                .or_else(|| block_move_action(&statement.then_block, source, resolved, environment))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| block_move_action(block, source, resolved, environment))
                })
        }
        Stmt::Switch(statement) => {
            expression_move_action(&statement.expression, source, resolved, environment)
                .or_else(|| {
                    statement
                        .arms
                        .iter()
                        .find_map(|arm| block_move_action(&arm.body, source, resolved, environment))
                })
                .or_else(|| {
                    statement
                        .else_arm
                        .as_ref()
                        .and_then(|arm| block_move_action(&arm.body, source, resolved, environment))
                })
        }
        Stmt::ForRange(statement) => {
            expression_move_action(&statement.start, source, resolved, environment)
                .or_else(|| expression_move_action(&statement.end, source, resolved, environment))
                .or_else(|| block_move_action(&statement.body, source, resolved, environment))
        }
        Stmt::While(statement) => {
            expression_move_action(&statement.condition, source, resolved, environment)
                .or_else(|| block_move_action(&statement.body, source, resolved, environment))
        }
        Stmt::Loop(statement) => block_move_action(&statement.body, source, resolved, environment),
        Stmt::Expression(statement) => {
            expression_move_action(&statement.expression, source, resolved, environment)
        }
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => None,
    }
}

fn block_move_action(
    block: &Block,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    block
        .statements
        .iter()
        .find_map(|statement| {
            statement_conflicting_action(statement, source, resolved, environment)
        })
        .or_else(|| {
            block
                .result
                .as_ref()
                .and_then(|result| expression_move_action(result, source, resolved, environment))
        })
}

fn statement_read_action(
    statement: &Stmt,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => None,
        Stmt::Return(statement) => statement.expression.as_ref().and_then(|expression| {
            expression_read_action(expression, source, resolved, environment)
        }),
        Stmt::Binding(statement) => {
            expression_read_action(&statement.initializer, source, resolved, environment)
        }
        Stmt::Assignment(statement) => {
            expression_read_action(&statement.target, source, resolved, environment)
                .or_else(|| expression_read_action(&statement.value, source, resolved, environment))
        }
        Stmt::If(statement) => {
            expression_read_action(&statement.condition, source, resolved, environment)
                .or_else(|| block_read_action(&statement.then_block, source, resolved, environment))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| block_read_action(block, source, resolved, environment))
                })
        }
        Stmt::IfIs(statement) => {
            expression_read_action(&statement.expression, source, resolved, environment)
                .or_else(|| block_read_action(&statement.then_block, source, resolved, environment))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| block_read_action(block, source, resolved, environment))
                })
        }
        Stmt::Switch(statement) => {
            expression_read_action(&statement.expression, source, resolved, environment)
                .or_else(|| {
                    statement
                        .arms
                        .iter()
                        .find_map(|arm| block_read_action(&arm.body, source, resolved, environment))
                })
                .or_else(|| {
                    statement
                        .else_arm
                        .as_ref()
                        .and_then(|arm| block_read_action(&arm.body, source, resolved, environment))
                })
        }
        Stmt::ForRange(statement) => {
            expression_read_action(&statement.start, source, resolved, environment)
                .or_else(|| expression_read_action(&statement.end, source, resolved, environment))
                .or_else(|| block_read_action(&statement.body, source, resolved, environment))
        }
        Stmt::While(statement) => {
            expression_read_action(&statement.condition, source, resolved, environment)
                .or_else(|| block_read_action(&statement.body, source, resolved, environment))
        }
        Stmt::Loop(statement) => block_read_action(&statement.body, source, resolved, environment),
        Stmt::Expression(statement) => {
            expression_read_action(&statement.expression, source, resolved, environment)
        }
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => None,
    }
}

fn block_read_action(
    block: &Block,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    block
        .statements
        .iter()
        .find_map(|statement| statement_read_action(statement, source, resolved, environment))
        .or_else(|| {
            block
                .result
                .as_ref()
                .and_then(|result| expression_read_action(result, source, resolved, environment))
        })
}

fn expression_move_action(
    expression: &Expr,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    match expression {
        Expr::Unary(expression) if expression.operator == UnaryOperator::Move => {
            match expression.operand.as_ref() {
                Expr::Identifier(identifier)
                    if BorrowPlace::whole(identifier.name.clone()).conflicts_with(source) =>
                {
                    Some(BorrowAction {
                        place: BorrowPlace::whole(identifier.name.clone()),
                        span: identifier.span,
                        description: "move",
                    })
                }
                _ => None,
            }
        }
        Expr::Propagate(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::Force(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::Catch(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment).or_else(
                || block_move_action(&expression.catch_block, source, resolved, environment),
            )
        }
        Expr::Borrow(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::Binary(expression) => {
            expression_move_action(&expression.left, source, resolved, environment).or_else(|| {
                expression_move_action(&expression.right, source, resolved, environment)
            })
        }
        Expr::Unary(expression) => {
            expression_move_action(&expression.operand, source, resolved, environment)
        }
        Expr::TypeConversion(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::Call(expression) => {
            if let Some(identifier) =
                owned_method_receiver_identifier(expression, resolved, environment)
                && BorrowPlace::whole(identifier.name.clone()).conflicts_with(source)
            {
                return Some(BorrowAction {
                    place: BorrowPlace::whole(identifier.name.clone()),
                    span: identifier.span,
                    description: "move",
                });
            }
            expression_move_action(&expression.callee, source, resolved, environment).or_else(
                || {
                    expression.arguments.iter().find_map(|argument| {
                        expression_move_action(argument, source, resolved, environment)
                    })
                },
            )
        }
        Expr::Member(expression) => {
            expression_move_action(&expression.object, source, resolved, environment)
        }
        Expr::Index(expression) => {
            expression_move_action(&expression.object, source, resolved, environment).or_else(
                || expression_move_action(&expression.index, source, resolved, environment),
            )
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| expression_move_action(element, source, resolved, environment)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| expression_move_action(&field.value, source, resolved, environment)),
        Expr::Group(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
        }
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    expression_move_action(&part.expression, source, resolved, environment)
                }
                crate::ast::InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::Otherwise(expression) => {
            expression_move_action(&expression.value, source, resolved, environment)
                .or_else(|| block_move_action(&expression.fallback, source, resolved, environment))
        }
        Expr::If(expression) => {
            expression_move_action(&expression.condition, source, resolved, environment)
                .or_else(|| {
                    block_move_action(&expression.then_block, source, resolved, environment)
                })
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| block_move_action(block, source, resolved, environment))
                })
        }
        Expr::IfIs(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
                .or_else(|| {
                    block_move_action(&expression.then_block, source, resolved, environment)
                })
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| block_move_action(block, source, resolved, environment))
                })
        }
        Expr::Match(expression) => {
            expression_move_action(&expression.expression, source, resolved, environment)
                .or_else(|| {
                    expression
                        .arms
                        .iter()
                        .find_map(|arm| block_move_action(&arm.body, source, resolved, environment))
                })
                .or_else(|| {
                    expression
                        .else_arm
                        .as_ref()
                        .and_then(|arm| block_move_action(&arm.body, source, resolved, environment))
                })
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

fn expression_read_action(
    expression: &Expr,
    source: &BorrowPlace,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<BorrowAction> {
    match expression {
        Expr::Identifier(identifier) => {
            let place = BorrowPlace::whole(identifier.name.clone());
            place.conflicts_with(source).then_some(BorrowAction {
                place,
                span: identifier.span,
                description: "use",
            })
        }
        Expr::Unary(expression) if expression.operator == UnaryOperator::Move => None,
        Expr::Propagate(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::Force(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::Catch(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment).or_else(
                || block_read_action(&expression.catch_block, source, resolved, environment),
            )
        }
        Expr::Borrow(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::Binary(expression) => {
            expression_read_action(&expression.left, source, resolved, environment).or_else(|| {
                expression_read_action(&expression.right, source, resolved, environment)
            })
        }
        Expr::Unary(expression) => {
            expression_read_action(&expression.operand, source, resolved, environment)
        }
        Expr::TypeConversion(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::Call(expression) => {
            expression_read_action(&expression.callee, source, resolved, environment).or_else(
                || {
                    expression.arguments.iter().find_map(|argument| {
                        expression_read_action(argument, source, resolved, environment)
                    })
                },
            )
        }
        Expr::Member(expression) => {
            if let Some(place) = member_expression_place(expression)
                && place.conflicts_with(source)
            {
                return Some(BorrowAction {
                    place,
                    span: expression.span,
                    description: "use",
                });
            }
            if expression_place_has_only_named_fields(&expression.object) {
                None
            } else {
                expression_read_action(&expression.object, source, resolved, environment)
            }
        }
        Expr::Index(expression) => {
            if let Some(place) = index_expression_place(expression)
                && place.conflicts_with(source)
            {
                return Some(BorrowAction {
                    place,
                    span: expression.span,
                    description: "use",
                });
            }
            let object_action = if expression_place_has_only_named_fields(&expression.object) {
                None
            } else {
                expression_read_action(&expression.object, source, resolved, environment)
            };
            object_action.or_else(|| {
                expression_read_action(&expression.index, source, resolved, environment)
            })
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| expression_read_action(element, source, resolved, environment)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| expression_read_action(&field.value, source, resolved, environment)),
        Expr::Group(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
        }
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    expression_read_action(&part.expression, source, resolved, environment)
                }
                crate::ast::InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::Otherwise(expression) => {
            expression_read_action(&expression.value, source, resolved, environment)
                .or_else(|| block_read_action(&expression.fallback, source, resolved, environment))
        }
        Expr::If(expression) => {
            expression_read_action(&expression.condition, source, resolved, environment)
                .or_else(|| {
                    block_read_action(&expression.then_block, source, resolved, environment)
                })
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| block_read_action(block, source, resolved, environment))
                })
        }
        Expr::IfIs(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
                .or_else(|| {
                    block_read_action(&expression.then_block, source, resolved, environment)
                })
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| block_read_action(block, source, resolved, environment))
                })
        }
        Expr::Match(expression) => {
            expression_read_action(&expression.expression, source, resolved, environment)
                .or_else(|| {
                    expression
                        .arms
                        .iter()
                        .find_map(|arm| block_read_action(&arm.body, source, resolved, environment))
                })
                .or_else(|| {
                    expression
                        .else_arm
                        .as_ref()
                        .and_then(|arm| block_read_action(&arm.body, source, resolved, environment))
                })
        }
        Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

fn collect_direct_borrow_expressions_in_statement(
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
            if let Some(else_arm) = &statement.else_arm {
                collect_direct_borrow_expressions_in_block(
                    &else_arm.body,
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

fn collect_direct_borrow_expressions(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrows: &mut Vec<DirectBorrowSource>,
) {
    if let Some(source) = direct_borrow_source(expression) {
        borrows.push(source);
    }

    match expression {
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
            if let Some(arm) = &expression.else_arm {
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
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn direct_borrow_source(expression: &Expr) -> Option<DirectBorrowSource> {
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

fn method_borrow_receiver_source(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<DirectBorrowSource> {
    let method = method_member_for_call(call)?;
    let (_, signature) = resolved_method_for_call(resolved, call, environment)?;
    let TypeExpr::Borrow(receiver) = &signature.receiver.ty else {
        return None;
    };
    let source = expression_place(&method.object)?;
    Some(DirectBorrowSource {
        source,
        source_span: method.object.span(),
        is_readwrite: receiver.is_readwrite,
    })
}

fn statement_uses_identifier(statement: &Stmt, name: &str) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(statement) => statement
            .expression
            .as_ref()
            .is_some_and(|expression| expression_uses_identifier(expression, name)),
        Stmt::Binding(statement) => expression_uses_identifier(&statement.initializer, name),
        Stmt::Assignment(statement) => {
            expression_uses_identifier(&statement.target, name)
                || expression_uses_identifier(&statement.value, name)
        }
        Stmt::If(statement) => {
            expression_uses_identifier(&statement.condition, name)
                || block_uses_identifier(&statement.then_block, name)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_identifier(block, name))
        }
        Stmt::IfIs(statement) => {
            expression_uses_identifier(&statement.expression, name)
                || block_uses_identifier(&statement.then_block, name)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_identifier(block, name))
        }
        Stmt::Switch(statement) => {
            expression_uses_identifier(&statement.expression, name)
                || statement
                    .arms
                    .iter()
                    .any(|arm| block_uses_identifier(&arm.body, name))
                || statement
                    .else_arm
                    .as_ref()
                    .is_some_and(|arm| block_uses_identifier(&arm.body, name))
        }
        Stmt::ForRange(statement) => {
            expression_uses_identifier(&statement.start, name)
                || expression_uses_identifier(&statement.end, name)
                || block_uses_identifier(&statement.body, name)
        }
        Stmt::While(statement) => {
            expression_uses_identifier(&statement.condition, name)
                || block_uses_identifier(&statement.body, name)
        }
        Stmt::Loop(statement) => block_uses_identifier(&statement.body, name),
        Stmt::Expression(statement) => expression_uses_identifier(&statement.expression, name),
        Stmt::Drop(statement) => statement.name == name,
        Stmt::Break(_) | Stmt::Continue(_) => false,
    }
}

fn block_uses_identifier(block: &Block, name: &str) -> bool {
    statements_use_identifier_before_terminal(&block.statements, name)
        || block
            .result
            .as_ref()
            .is_some_and(|result| expression_uses_identifier(result, name))
}

fn statements_use_identifier_before_terminal(statements: &[Stmt], name: &str) -> bool {
    for statement in statements {
        if statement_uses_identifier(statement, name) {
            return true;
        }
        if statement_is_unconditionally_terminal(statement) {
            return false;
        }
    }
    false
}

fn statement_is_unconditionally_terminal(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_)
    )
}

fn expression_uses_identifier(expression: &Expr, name: &str) -> bool {
    match expression {
        Expr::Identifier(identifier) => identifier.name == name,
        Expr::Propagate(expression) => expression_uses_identifier(&expression.expression, name),
        Expr::Force(expression) => expression_uses_identifier(&expression.expression, name),
        Expr::Catch(expression) => {
            expression_uses_identifier(&expression.expression, name)
                || block_uses_identifier(&expression.catch_block, name)
        }
        Expr::Borrow(expression) => expression_uses_identifier(&expression.expression, name),
        Expr::Binary(expression) => {
            expression_uses_identifier(&expression.left, name)
                || expression_uses_identifier(&expression.right, name)
        }
        Expr::Unary(expression) => expression_uses_identifier(&expression.operand, name),
        Expr::TypeConversion(expression) => {
            expression_uses_identifier(&expression.expression, name)
        }
        Expr::Call(expression) => {
            expression_uses_identifier(&expression.callee, name)
                || expression
                    .arguments
                    .iter()
                    .any(|argument| expression_uses_identifier(argument, name))
        }
        Expr::Member(expression) => expression_uses_identifier(&expression.object, name),
        Expr::Index(expression) => {
            expression_uses_identifier(&expression.object, name)
                || expression_uses_identifier(&expression.index, name)
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .any(|element| expression_uses_identifier(element, name)),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .any(|field| expression_uses_identifier(&field.value, name)),
        Expr::Group(expression) => expression_uses_identifier(&expression.expression, name),
        Expr::InterpolatedString(expression) => expression.parts.iter().any(|part| match part {
            crate::ast::InterpolatedStringPart::Expression(part) => {
                expression_uses_identifier(&part.expression, name)
            }
            crate::ast::InterpolatedStringPart::Text(_) => false,
        }),
        Expr::Otherwise(expression) => {
            expression_uses_identifier(&expression.value, name)
                || block_uses_identifier(&expression.fallback, name)
        }
        Expr::If(expression) => {
            expression_uses_identifier(&expression.condition, name)
                || block_uses_identifier(&expression.then_block, name)
                || expression
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_identifier(block, name))
        }
        Expr::IfIs(expression) => {
            expression_uses_identifier(&expression.expression, name)
                || block_uses_identifier(&expression.then_block, name)
                || expression
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_identifier(block, name))
        }
        Expr::Match(expression) => {
            expression_uses_identifier(&expression.expression, name)
                || expression
                    .arms
                    .iter()
                    .any(|arm| block_uses_identifier(&arm.body, name))
                || expression
                    .else_arm
                    .as_ref()
                    .is_some_and(|arm| block_uses_identifier(&arm.body, name))
        }
        Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => false,
    }
}

fn check_statement_ownership(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) -> FlowState {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => FlowState::fallthrough(),
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_ownership(
                    sources,
                    expression,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
            FlowState::terminal()
        }
        Stmt::Binding(statement) => {
            check_expression_ownership(
                sources,
                &statement.initializer,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            let initializer_reaches_end = initializer_type != Type::Never;
            let mut flow = FlowState::fallthrough();
            if !initializer_reaches_end {
                return FlowState::terminal();
            }
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            environment.define_binding(
                statement.name.clone(),
                binding_type.clone(),
                binding_kind_is_mutable(statement.kind),
            );
            ownership.define_binding(
                statement.name.clone(),
                statement.name_span,
                &binding_type,
                resolved,
            );
            flow.reaches_end = true;
            flow
        }
        Stmt::Assignment(statement) => {
            check_assignment_target_ownership(
                sources,
                &statement.target,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &statement.value,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            if let Some(identifier) = whole_identifier(&statement.target)
                && let Some(ty) = environment.get(&identifier.name)
            {
                ownership.define_binding(identifier.name.clone(), identifier.span, ty, resolved);
            }
            if expression_type(&statement.value, resolved, environment) == Type::Never {
                FlowState::terminal()
            } else {
                FlowState::fallthrough()
            }
        }
        Stmt::If(statement) => {
            check_expression_ownership(
                sources,
                &statement.condition,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut then_environment = environment.clone();
            let mut then_ownership = ownership.clone();
            let then_flow = check_block_ownership(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_ownership,
            );
            let then_reaches_end = then_flow.reaches_end;
            let mut flow = FlowState::from_nested(then_flow);
            let mut incoming = Vec::new();
            if then_reaches_end {
                incoming.push(then_ownership);
            }
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                let else_flow = check_block_ownership(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
                let else_reaches_end = else_flow.reaches_end;
                flow.extend_nested(else_flow);
                if else_reaches_end {
                    incoming.push(else_ownership);
                }
            } else {
                incoming.push(ownership.clone());
            }
            flow.reaches_end = !incoming.is_empty();
            if flow.reaches_end {
                ownership.join_branches(&incoming);
            }
            flow
        }
        Stmt::IfIs(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            let mut then_ownership = ownership.clone();
            if let Some(payload) = &statement.payload {
                then_ownership.define_binding_from_environment(
                    &payload.name,
                    payload.span,
                    &then_environment,
                    resolved,
                );
            }
            let then_flow = check_block_ownership(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                &mut then_ownership,
            );
            let then_reaches_end = then_flow.reaches_end;
            let mut flow = FlowState::from_nested(then_flow);
            let mut incoming = Vec::new();
            if then_reaches_end {
                incoming.push(then_ownership);
            }
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                let else_flow = check_block_ownership(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
                let else_reaches_end = else_flow.reaches_end;
                flow.extend_nested(else_flow);
                if else_reaches_end {
                    incoming.push(else_ownership);
                }
            } else {
                incoming.push(ownership.clone());
            }
            flow.reaches_end = !incoming.is_empty();
            if flow.reaches_end {
                ownership.join_branches(&incoming);
            }
            flow
        }
        Stmt::Switch(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut flow = FlowState::terminal();
            let mut branch_ownerships = Vec::new();
            for arm in &statement.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &statement.expression, resolved, environment);
                let mut arm_ownership = ownership.clone();
                if let Some(payload) = &arm.payload {
                    arm_ownership.define_binding_from_environment(
                        &payload.name,
                        payload.span,
                        &arm_environment,
                        resolved,
                    );
                }
                let arm_flow = check_block_ownership(
                    sources,
                    &arm.body,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    &mut arm_ownership,
                );
                if arm_flow.reaches_end {
                    branch_ownerships.push(arm_ownership);
                }
                flow.extend_nested(arm_flow);
            }
            if let Some(else_arm) = &statement.else_arm {
                let mut else_environment = environment.clone();
                let mut else_ownership = ownership.clone();
                let else_flow = check_block_ownership(
                    sources,
                    &else_arm.body,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    &mut else_ownership,
                );
                if else_flow.reaches_end {
                    branch_ownerships.push(else_ownership);
                }
                flow.extend_nested(else_flow);
            } else if !switch_statement_covers_all_variants(statement, resolved, environment) {
                branch_ownerships.push(ownership.clone());
            }
            flow.reaches_end = !branch_ownerships.is_empty();
            if flow.reaches_end {
                ownership.join_branches(&branch_ownerships);
            }
            flow
        }
        Stmt::While(statement) => {
            check_expression_ownership(
                sources,
                &statement.condition,
                resolved,
                diagnostics,
                environment,
                ownership,
            );

            let mut body_environment = environment.clone();
            let mut body_ownership = ownership.clone();
            let body_flow = check_block_ownership(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
            let mut incoming = vec![ownership.clone()];
            if body_flow.reaches_end {
                incoming.push(body_ownership);
            }
            incoming.extend(body_flow.break_states.iter().cloned());
            incoming.extend(body_flow.continue_states.iter().cloned());
            ownership.join_branches(&incoming);
            FlowState::fallthrough()
        }
        Stmt::ForRange(statement) => {
            check_expression_ownership(
                sources,
                &statement.start,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &statement.end,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            let mut body_ownership = ownership.clone();
            let body_flow = check_block_ownership(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
            let mut incoming = vec![ownership.clone()];
            if body_flow.reaches_end {
                incoming.push(body_ownership);
            }
            incoming.extend(body_flow.break_states.iter().cloned());
            incoming.extend(body_flow.continue_states.iter().cloned());
            ownership.join_branches(&incoming);
            FlowState::fallthrough()
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            let mut body_ownership = ownership.clone();
            let body_flow = check_block_ownership(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                &mut body_ownership,
            );
            let mut incoming = body_flow.break_states.clone();
            if body_flow.reaches_end {
                incoming.push(body_ownership);
            }
            incoming.extend(body_flow.continue_states.iter().cloned());
            if incoming.is_empty() {
                FlowState::terminal()
            } else {
                ownership.join_branches(&incoming);
                FlowState::fallthrough()
            }
        }
        Stmt::Drop(statement) => {
            let Some(ty) = environment.get(&statement.name) else {
                diagnostics.push(invalid_drop_target_diagnostic(
                    sources,
                    statement.name.as_str(),
                    statement.name_span,
                    None,
                ));
                return FlowState::fallthrough();
            };
            if non_copy_struct_type_name(ty, resolved).is_none() {
                diagnostics.push(invalid_drop_target_diagnostic(
                    sources,
                    statement.name.as_str(),
                    statement.name_span,
                    Some(ty),
                ));
                return FlowState::fallthrough();
            }
            ownership.ensure_binding_from_environment(
                &statement.name,
                statement.name_span,
                environment,
                resolved,
            );
            ownership.drop_binding(sources, &statement.name, statement.name_span, diagnostics);
            FlowState::fallthrough()
        }
        Stmt::Expression(statement) => {
            check_expression_ownership(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            if expression_type(&statement.expression, resolved, environment) == Type::Never {
                FlowState::terminal()
            } else {
                FlowState::fallthrough()
            }
        }
        Stmt::Break(_) => FlowState::break_with(ownership.clone()),
        Stmt::Continue(_) => FlowState::continue_with(ownership.clone()),
    }
}

fn check_expression_ownership(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) {
    match expression {
        Expr::Identifier(identifier) => {
            ownership.require_initialized(sources, identifier, "use", diagnostics);
        }
        Expr::Unary(expression) if expression.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = expression.operand.as_ref() {
                if let Some(ty) = environment.get(&identifier.name)
                    && non_copy_struct_type_name(ty, resolved).is_some()
                {
                    ownership.ensure_binding_from_environment(
                        &identifier.name,
                        identifier.span,
                        environment,
                        resolved,
                    );
                    ownership.move_binding(sources, identifier, diagnostics);
                }
            }
        }
        Expr::Propagate(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Force(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Catch(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            let success_ownership = ownership.clone();
            let mut catch_environment = environment_for_catch(
                expression.error_name.clone(),
                &expression.expression,
                resolved,
                environment,
            );
            let mut catch_ownership = ownership.clone();
            catch_ownership.define_binding_from_environment(
                &expression.error_name,
                expression.error_span,
                &catch_environment,
                resolved,
            );
            let catch_flow = check_block_ownership(
                sources,
                &expression.catch_block,
                resolved,
                diagnostics,
                &mut catch_environment,
                &mut catch_ownership,
            );
            if catch_flow.reaches_end {
                ownership.join_branches(&[success_ownership, catch_ownership]);
            }
        }
        Expr::Borrow(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Binary(expression) => {
            check_expression_ownership(
                sources,
                &expression.left,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &expression.right,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Unary(expression) => {
            check_expression_ownership(
                sources,
                &expression.operand,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::TypeConversion(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Call(expression) => {
            if let Some(identifier) =
                owned_method_receiver_identifier(expression, resolved, environment)
            {
                ownership.ensure_binding_from_environment(
                    &identifier.name,
                    identifier.span,
                    environment,
                    resolved,
                );
                ownership.move_binding(sources, identifier, diagnostics);
            } else if collection_builtin_call_type(expression, resolved, environment).is_some() {
                if let Some(method) = method_member_for_call(expression) {
                    check_expression_ownership(
                        sources,
                        &method.object,
                        resolved,
                        diagnostics,
                        environment,
                        ownership,
                    );
                }
            } else if let Some(method) = method_member_for_call(expression)
                && resolved_method_for_call(resolved, expression, environment).is_some()
            {
                check_expression_ownership(
                    sources,
                    &method.object,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            } else {
                check_expression_ownership(
                    sources,
                    &expression.callee,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }

            for argument in &expression.arguments {
                check_expression_ownership(
                    sources,
                    argument,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::Member(expression) => {
            check_expression_ownership(
                sources,
                &expression.object,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Index(expression) => {
            check_expression_ownership(
                sources,
                &expression.object,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            check_expression_ownership(
                sources,
                &expression.index,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression_ownership(
                    sources,
                    element,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression_ownership(
                    sources,
                    &field.value,
                    resolved,
                    diagnostics,
                    environment,
                    ownership,
                );
            }
        }
        Expr::Group(expression) => {
            check_expression_ownership(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    check_expression_ownership(
                        sources,
                        &part.expression,
                        resolved,
                        diagnostics,
                        environment,
                        ownership,
                    );
                }
            }
        }
        Expr::Otherwise(expression) => {
            check_expression_ownership(
                sources,
                &expression.value,
                resolved,
                diagnostics,
                environment,
                ownership,
            );
            let present_ownership = ownership.clone();
            let mut fallback_environment = environment.clone();
            let mut fallback_ownership = ownership.clone();
            let fallback_flow = check_block_ownership(
                sources,
                &expression.fallback,
                resolved,
                diagnostics,
                &mut fallback_environment,
                &mut fallback_ownership,
            );
            let mut incoming = vec![present_ownership];
            if fallback_flow.reaches_end {
                incoming.push(fallback_ownership);
            }
            ownership.join_branches(&incoming);
        }
        Expr::If(expression) => {
            check_statement_ownership(
                sources,
                &Stmt::If((**expression).clone()),
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::IfIs(expression) => {
            check_statement_ownership(
                sources,
                &Stmt::IfIs((**expression).clone()),
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::Match(expression) => {
            check_statement_ownership(
                sources,
                &Stmt::Switch((**expression).clone()),
                resolved,
                diagnostics,
                environment,
                ownership,
            );
        }
        Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_assignment_target_ownership(
    sources: &SourceMap,
    target: &Expr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    ownership: &mut OwnershipState,
) {
    if whole_identifier(target).is_some() {
        return;
    }
    check_expression_ownership(
        sources,
        target,
        resolved,
        diagnostics,
        environment,
        ownership,
    );
}

fn whole_identifier(expression: &Expr) -> Option<&IdentifierExpr> {
    match expression {
        Expr::Identifier(identifier) => Some(identifier),
        Expr::Group(group) => whole_identifier(&group.expression),
        _ => None,
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn assignment_target_place(expression: &Expr) -> Option<BorrowPlace> {
    expression_place(expression)
}

fn expression_place(expression: &Expr) -> Option<BorrowPlace> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some(BorrowPlace::whole(identifier.name.clone())),
        Expr::Member(member) => member_expression_place(member),
        Expr::Index(index) => index_expression_place(index),
        _ => None,
    }
}

fn member_expression_place(member: &crate::ast::MemberExpr) -> Option<BorrowPlace> {
    let mut place = expression_place(&member.object)?;
    place.push_field(member.member.clone());
    Some(place)
}

fn index_expression_place(index: &crate::ast::IndexExpr) -> Option<BorrowPlace> {
    let mut place = expression_place(&index.object)?;
    place.mark_unknown();
    Some(place)
}

fn expression_place_has_only_named_fields(expression: &Expr) -> bool {
    expression_place(expression).is_some_and(|place| place.fields.is_some())
}

fn owned_method_receiver_identifier<'a>(
    call: &'a crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<&'a IdentifierExpr> {
    let method = method_member_for_call(call)?;
    let (_, signature) = resolved_method_for_call(resolved, call, environment)?;
    if !matches!(signature.receiver.ty, TypeExpr::Reference(ref reference) if reference.name == "Self")
    {
        return None;
    }

    let Expr::Identifier(identifier) = method.object.as_ref() else {
        return None;
    };
    let receiver_type = expression_type(&method.object, resolved, environment);
    non_copy_struct_type_name(&receiver_type, resolved)?;
    Some(identifier)
}

fn non_copy_struct_type_name<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a str> {
    let canonical_name = ty.nominal_name()?;
    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Struct && !symbol.is_copy)
        .map(|symbol| symbol.canonical_name.as_str())
}

#[derive(Debug, Clone)]
struct ActiveBorrow {
    source: BorrowPlace,
    borrow_name: String,
    borrow_span: ByteSpan,
    is_readwrite: bool,
}

#[derive(Debug, Clone)]
struct DirectBorrowSource {
    source: BorrowPlace,
    source_span: ByteSpan,
    is_readwrite: bool,
}

#[derive(Debug, Clone)]
struct BorrowAction {
    place: BorrowPlace,
    span: ByteSpan,
    description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BorrowPlace {
    root: String,
    fields: Option<Vec<String>>,
}

impl BorrowPlace {
    fn whole(root: String) -> Self {
        Self {
            root,
            fields: Some(Vec::new()),
        }
    }

    fn push_field(&mut self, field: String) {
        if let Some(fields) = &mut self.fields {
            fields.push(field);
        }
    }

    fn mark_unknown(&mut self) {
        self.fields = None;
    }

    fn conflicts_with(&self, other: &Self) -> bool {
        if self.root != other.root {
            return false;
        }
        let (Some(left), Some(right)) = (&self.fields, &other.fields) else {
            return true;
        };
        left.starts_with(right) || right.starts_with(left)
    }

    fn display(&self) -> String {
        let Some(fields) = &self.fields else {
            return self.root.clone();
        };
        if fields.is_empty() {
            self.root.clone()
        } else {
            format!("{}.{}", self.root, fields.join("."))
        }
    }
}

#[derive(Debug, Clone)]
struct FlowState {
    reaches_end: bool,
    break_states: Vec<OwnershipState>,
    continue_states: Vec<OwnershipState>,
}

impl FlowState {
    fn fallthrough() -> Self {
        Self {
            reaches_end: true,
            break_states: Vec::new(),
            continue_states: Vec::new(),
        }
    }

    fn terminal() -> Self {
        Self {
            reaches_end: false,
            break_states: Vec::new(),
            continue_states: Vec::new(),
        }
    }

    fn break_with(state: OwnershipState) -> Self {
        Self {
            reaches_end: false,
            break_states: vec![state],
            continue_states: Vec::new(),
        }
    }

    fn continue_with(state: OwnershipState) -> Self {
        Self {
            reaches_end: false,
            break_states: Vec::new(),
            continue_states: vec![state],
        }
    }

    fn from_nested(flow: FlowState) -> Self {
        flow
    }

    fn extend_nested(&mut self, flow: FlowState) {
        self.break_states.extend(flow.break_states);
        self.continue_states.extend(flow.continue_states);
    }
}

#[derive(Debug, Clone, Default)]
struct OwnershipState {
    bindings: HashMap<String, OwnedBinding>,
}

impl OwnershipState {
    fn define_parameters(
        &mut self,
        parameters: &[crate::ast::Parameter],
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        for parameter in parameters {
            self.define_binding_from_environment(
                &parameter.name,
                parameter.name_span,
                environment,
                resolved,
            );
        }
    }

    fn define_binding_from_environment(
        &mut self,
        name: &str,
        span: ByteSpan,
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        if let Some(ty) = environment.get(name) {
            self.define_binding(name.to_string(), span, ty, resolved);
        } else {
            self.bindings.remove(name);
        }
    }

    fn ensure_binding_from_environment(
        &mut self,
        name: &str,
        span: ByteSpan,
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        if self.bindings.contains_key(name) {
            return;
        }
        self.define_binding_from_environment(name, span, environment, resolved);
    }

    fn define_binding(
        &mut self,
        name: String,
        span: ByteSpan,
        ty: &Type,
        resolved: &ResolveOutput,
    ) {
        if non_copy_struct_type_name(ty, resolved).is_some() {
            self.bindings.insert(
                name,
                OwnedBinding {
                    state: BindingState::Initialized { span },
                },
            );
        } else {
            self.bindings.remove(&name);
        }
    }

    fn require_initialized(
        &self,
        sources: &SourceMap,
        identifier: &IdentifierExpr,
        action: &'static str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let Some(binding) = self.bindings.get(&identifier.name) else {
            return true;
        };
        let BindingState::Initialized { .. } = binding.state else {
            diagnostics.push(uninitialized_binding_diagnostic(
                sources,
                &identifier.name,
                identifier.span,
                action,
                binding.state.previous_action(),
                binding.state.previous_span(),
            ));
            return false;
        };
        true
    }

    fn join_branches(&mut self, branch_ownerships: &[OwnershipState]) {
        if branch_ownerships.is_empty() {
            return;
        }
        for (name, binding) in &mut self.bindings {
            let mut joined_state = branch_ownerships[0]
                .bindings
                .get(name)
                .map(|binding| binding.state)
                .unwrap_or(binding.state);
            for branch_ownership in &branch_ownerships[1..] {
                let branch_state = branch_ownership
                    .bindings
                    .get(name)
                    .map(|binding| binding.state)
                    .unwrap_or(binding.state);
                joined_state = BindingState::join(joined_state, branch_state);
            }
            binding.state = joined_state;
        }
    }

    fn move_binding(
        &mut self,
        sources: &SourceMap,
        identifier: &IdentifierExpr,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self.require_initialized(sources, identifier, "move", diagnostics) {
            return;
        }
        if let Some(binding) = self.bindings.get_mut(&identifier.name) {
            binding.state = BindingState::Moved {
                span: identifier.span,
            };
        }
    }

    fn drop_binding(
        &mut self,
        sources: &SourceMap,
        name: &str,
        span: ByteSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let identifier = IdentifierExpr {
            span,
            name: name.to_string(),
        };
        if !self.require_initialized(sources, &identifier, "drop", diagnostics) {
            return;
        }
        if let Some(binding) = self.bindings.get_mut(name) {
            binding.state = BindingState::Dropped { span };
        }
    }
}

#[derive(Debug, Clone)]
struct OwnedBinding {
    state: BindingState,
}

#[derive(Debug, Clone, Copy)]
enum BindingState {
    Initialized { span: ByteSpan },
    Moved { span: ByteSpan },
    Dropped { span: ByteSpan },
    Uninitialized { span: ByteSpan },
    MaybeInitialized { span: ByteSpan },
}

impl BindingState {
    fn join(left: Self, right: Self) -> Self {
        match (left, right) {
            (BindingState::Initialized { span }, BindingState::Initialized { .. }) => {
                BindingState::Initialized { span }
            }
            (BindingState::Moved { span }, BindingState::Moved { .. }) => {
                BindingState::Moved { span }
            }
            (BindingState::Dropped { span }, BindingState::Dropped { .. }) => {
                BindingState::Dropped { span }
            }
            (BindingState::Uninitialized { span }, BindingState::Uninitialized { .. }) => {
                BindingState::Uninitialized { span }
            }
            (
                BindingState::Moved { span }
                | BindingState::Dropped { span }
                | BindingState::Uninitialized { span },
                BindingState::Moved { .. }
                | BindingState::Dropped { .. }
                | BindingState::Uninitialized { .. },
            ) => BindingState::Uninitialized { span },
            (BindingState::MaybeInitialized { span }, _)
            | (_, BindingState::MaybeInitialized { span }) => {
                BindingState::MaybeInitialized { span }
            }
            (BindingState::Initialized { .. }, BindingState::Moved { span })
            | (BindingState::Moved { span }, BindingState::Initialized { .. })
            | (BindingState::Initialized { .. }, BindingState::Dropped { span })
            | (BindingState::Dropped { span }, BindingState::Initialized { .. })
            | (BindingState::Initialized { .. }, BindingState::Uninitialized { span })
            | (BindingState::Uninitialized { span }, BindingState::Initialized { .. }) => {
                BindingState::MaybeInitialized { span }
            }
        }
    }

    fn previous_action(self) -> &'static str {
        match self {
            BindingState::Moved { .. } => "moved",
            BindingState::Dropped { .. } => "dropped",
            BindingState::Uninitialized { .. } => "uninitialized",
            BindingState::MaybeInitialized { .. } => "maybe uninitialized",
            BindingState::Initialized { .. } => "initialized",
        }
    }

    fn previous_span(self) -> ByteSpan {
        match self {
            BindingState::Initialized { span }
            | BindingState::Moved { span }
            | BindingState::Dropped { span }
            | BindingState::Uninitialized { span }
            | BindingState::MaybeInitialized { span } => span,
        }
    }
}

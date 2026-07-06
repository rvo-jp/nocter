//! Type checking, ownership, borrowing, move, and drop checks.

use crate::ast::{
    ArrayLiteralExpr, AstFile, BinaryExpr, BinaryOperator, BindingKind, BindingStmt, Block,
    CallExpr, Expr, FailStmt, ForRangeStmt, FunctionDecl, IfIsStmt, IfLetStmt, IfStmt, IndexExpr,
    Item, LiteralExpr, MemberExpr, Parameter, ProgramDecl, ReturnStmt, Stmt, SwitchArm, SwitchStmt,
    TypeConversionExpr, TypeExpr, UnaryExpr, UnaryOperator, WhileLetStmt, WhileStmt,
};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::resolve::{
    EnumVariantSignature, FunctionSignature, ParameterSignature, ResolveOutput, TypeSymbol,
    TypeSymbolKind,
};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub fn check(sources: &SourceMap, ast: &AstFile, resolved: &ResolveOutput) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_program_entry(sources, ast, &mut diagnostics);
    check_call_expressions(sources, ast, resolved, &mut diagnostics);
    check_return_types(sources, ast, resolved, &mut diagnostics);

    diagnostics
}

fn check_program_entry(sources: &SourceMap, ast: &AstFile, diagnostics: &mut Vec<Diagnostic>) {
    let programs: Vec<&ProgramDecl> = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Program(program) => Some(program),
            _ => None,
        })
        .collect();

    match programs.as_slice() {
        [] => {
            if let Some(main) = find_main_function(ast) {
                diagnostics.push(main_is_not_entry_diagnostic(sources, main));
            } else {
                diagnostics.push(missing_program_diagnostic(sources, ast.span));
            }
        }
        [program] => {
            if !is_valid_program_return_type(&program.return_type) {
                diagnostics.push(invalid_program_return_type_diagnostic(
                    sources,
                    program.return_type.span(),
                ));
            }
        }
        [first, second, ..] => {
            diagnostics.push(duplicate_program_diagnostic(
                sources,
                first.span,
                second.span,
            ));
        }
    }
}

fn find_main_function(ast: &AstFile) -> Option<&FunctionDecl> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == "main" => Some(function),
        _ => None,
    })
}

fn is_valid_program_return_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "i32" || reference.name == "void")
}

fn check_return_types(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Program(program) if is_valid_program_return_type(&program.return_type) => {
                let context = ReturnContext::new(
                    CallableKind::Program,
                    type_expr_to_type(&program.return_type, resolved),
                    program.return_type.span(),
                );
                let mut environment = TypeEnvironment::default();
                check_block_returns(
                    sources,
                    &program.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                );
            }
            Item::Function(function) => {
                let context = ReturnContext::new(
                    CallableKind::Function(function.name.clone()),
                    type_expr_to_type(&function.return_type, resolved),
                    function.return_type.span(),
                );
                let mut environment =
                    environment_for_parameters(&function.parameters.parameters, resolved);
                check_block_returns(
                    sources,
                    &function.body,
                    &context,
                    resolved,
                    diagnostics,
                    &mut environment,
                );
            }
            _ => {}
        }
    }
}

fn check_block_returns(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
) {
    check_block_return_statements(sources, block, context, resolved, diagnostics, environment);

    if context.requires_explicit_return() && !block_guarantees_return(block) {
        diagnostics.push(missing_return_diagnostic(sources, block.span, context));
    }
}

fn check_block_return_statements(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
) {
    for statement in &block.statements {
        check_statement_returns(
            sources,
            statement,
            context,
            resolved,
            diagnostics,
            environment,
        );
    }
}

fn check_statement_returns(
    sources: &SourceMap,
    statement: &Stmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_for_nested_returns(
                    sources,
                    expression,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                );
            }
            check_return_statement(
                sources,
                statement,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Stmt::Fail(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_fail_statement(
                sources,
                statement,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Stmt::Binding(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.initializer,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            if let Some(else_block) = &statement.else_block {
                check_optional_let_else_statement(
                    sources,
                    statement,
                    &initializer_type,
                    diagnostics,
                );
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
            }
            let binding_type = continuing_binding_type(statement, initializer_type, resolved);
            environment.define(statement.name.clone(), binding_type);
        }
        Stmt::Try(statement) => {
            check_try_propagation(
                sources,
                statement.span,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Stmt::TryCatch(statement) => {
            check_try_catch_operand(
                sources,
                statement.span,
                &statement.expression,
                resolved,
                environment,
                diagnostics,
            );
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut catch_environment = environment_for_catch(
                statement.error_name.clone(),
                &statement.expression,
                resolved,
                environment,
            );
            check_block_return_statements(
                sources,
                &statement.catch_block,
                context,
                resolved,
                diagnostics,
                &mut catch_environment,
            );
        }
        Stmt::If(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.condition,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut then_environment = environment.clone();
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
            }
        }
        Stmt::IfIs(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
            }
        }
        Stmt::IfLet(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.initializer,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut then_environment =
                environment_for_if_let_binding(statement, resolved, environment);
            check_block_return_statements(
                sources,
                &statement.then_block,
                context,
                resolved,
                diagnostics,
                &mut then_environment,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    else_block,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
            }
        }
        Stmt::Switch(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            for arm in &statement.arms {
                let mut arm_environment = environment_for_switch_arm(arm, resolved, environment);
                check_block_return_statements(
                    sources,
                    &arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                );
            }
            if let Some(else_arm) = &statement.else_arm {
                let mut else_environment = environment.clone();
                check_block_return_statements(
                    sources,
                    &else_arm.body,
                    context,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                );
            }
        }
        Stmt::While(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.condition,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut body_environment = environment.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
            );
        }
        Stmt::WhileLet(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.initializer,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut body_environment =
                environment_for_while_let_binding(statement, resolved, environment);
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
            );
        }
        Stmt::ForRange(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.start,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_expression_for_nested_returns(
                sources,
                &statement.end,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
            );
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            check_block_return_statements(
                sources,
                &statement.body,
                context,
                resolved,
                diagnostics,
                &mut body_environment,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
        Stmt::Expression(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
    }
}

fn check_expression_for_nested_returns(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
) {
    match expression {
        Expr::Try(expression) => {
            check_try_propagation(
                sources,
                expression.span,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::TryCatch(expression) => {
            check_try_catch_operand(
                sources,
                expression.span,
                &expression.expression,
                resolved,
                environment,
                diagnostics,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
            let mut catch_environment = environment_for_catch(
                expression.error_name.clone(),
                &expression.expression,
                resolved,
                environment,
            );
            check_block_return_statements(
                sources,
                &expression.catch_block,
                context,
                resolved,
                diagnostics,
                &mut catch_environment,
            );
        }
        Expr::Binary(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.left,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.right,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::Unary(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.operand,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::TypeConversion(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::Call(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.callee,
                context,
                resolved,
                diagnostics,
                environment,
            );
            for argument in &expression.arguments {
                check_expression_for_nested_returns(
                    sources,
                    argument,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                );
            }
        }
        Expr::Member(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.object,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::Index(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.object,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.index,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression_for_nested_returns(
                    sources,
                    element,
                    context,
                    resolved,
                    diagnostics,
                    environment,
                );
            }
        }
        Expr::Group(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::OptionalDefault(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.value,
                context,
                resolved,
                diagnostics,
                environment,
            );
            check_expression_for_nested_returns(
                sources,
                &expression.default,
                context,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_return_statement(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let expected = context.success_type();

    match (&statement.expression, expected) {
        (None, Type::Void) => {}
        (None, Type::Unknown) | (None, Type::Unresolved(_)) => {}
        (None, _) => diagnostics.push(missing_return_value_diagnostic(sources, statement, context)),
        (Some(expression), Type::Void) => {
            diagnostics.push(unexpected_return_value_diagnostic(
                sources, expression, context,
            ));
        }
        (Some(expression), expected) => {
            let actual = expression_type(expression, resolved, environment);
            if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
                return;
            }

            if !is_expression_assignable(expected, expression, resolved, environment) {
                diagnostics.push(return_type_mismatch_diagnostic(
                    sources, expression, expected, &actual, context,
                ));
            }
        }
    }
}

fn check_fail_statement(
    sources: &SourceMap,
    statement: &FailStmt,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let Type::Fallible {
        error: expected, ..
    } = &context.declared_type
    else {
        if !context.declared_type.is_unknown_or_unresolved() {
            diagnostics.push(fail_in_non_fallible_context_diagnostic(
                sources, statement, context,
            ));
        }
        return;
    };

    let actual = expression_type(&statement.expression, resolved, environment);
    if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
        return;
    }

    if !is_expression_assignable(expected, &statement.expression, resolved, environment) {
        diagnostics.push(fail_type_mismatch_diagnostic(
            sources, statement, expected, &actual, context,
        ));
    }
}

fn check_call_expressions(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Program(program) => {
                let mut environment = TypeEnvironment::default();
                check_block_calls(
                    sources,
                    &program.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    0,
                );
            }
            Item::Function(function) => {
                let mut environment =
                    environment_for_parameters(&function.parameters.parameters, resolved);
                check_block_calls(
                    sources,
                    &function.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    0,
                );
            }
            Item::Use(_)
            | Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }
}

fn check_block_calls(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    loop_depth: usize,
) {
    for statement in &block.statements {
        check_statement_calls(
            sources,
            statement,
            resolved,
            diagnostics,
            environment,
            loop_depth,
        );
    }
}

fn check_statement_calls(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    loop_depth: usize,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression_calls(
                    sources,
                    expression,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
        }
        Stmt::Fail(statement) => {
            check_expression_calls(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
        Stmt::Binding(statement) => {
            check_expression_calls(
                sources,
                &statement.initializer,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            let initializer_type = expression_type(&statement.initializer, resolved, environment);
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_calls(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    loop_depth,
                );
            }
            check_binding_annotation(
                sources,
                statement,
                &initializer_type,
                resolved,
                diagnostics,
                environment,
            );
            let binding_type = continuing_binding_type(statement, initializer_type, resolved);
            environment.define(statement.name.clone(), binding_type);
        }
        Stmt::Try(statement) => {
            check_expression_calls(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
        Stmt::TryCatch(statement) => {
            check_expression_calls(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            let mut catch_environment = environment_for_catch(
                statement.error_name.clone(),
                &statement.expression,
                resolved,
                environment,
            );
            check_block_calls(
                sources,
                &statement.catch_block,
                resolved,
                diagnostics,
                &mut catch_environment,
                loop_depth,
            );
        }
        Stmt::If(statement) => {
            check_expression_calls(
                sources,
                &statement.condition,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_if_condition(sources, statement, resolved, diagnostics, environment);

            let mut then_environment = environment.clone();
            check_block_calls(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                loop_depth,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_calls(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    loop_depth,
                );
            }
        }
        Stmt::IfIs(statement) => {
            check_expression_calls(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_if_is_statement(sources, statement, resolved, diagnostics, environment);

            let mut then_environment =
                environment_for_if_is_binding(statement, resolved, environment);
            check_block_calls(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                loop_depth,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_calls(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    loop_depth,
                );
            }
        }
        Stmt::IfLet(statement) => {
            check_expression_calls(
                sources,
                &statement.initializer,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_if_let_initializer(sources, statement, resolved, diagnostics, environment);

            let mut then_environment =
                environment_for_if_let_binding(statement, resolved, environment);
            check_block_calls(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                loop_depth,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_calls(
                    sources,
                    else_block,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    loop_depth,
                );
            }
        }
        Stmt::Switch(statement) => {
            check_expression_calls(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_switch_statement(sources, statement, resolved, diagnostics, environment);

            for arm in &statement.arms {
                let mut arm_environment = environment_for_switch_arm(arm, resolved, environment);
                check_block_calls(
                    sources,
                    &arm.body,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    loop_depth,
                );
            }
            if let Some(else_arm) = &statement.else_arm {
                let mut else_environment = environment.clone();
                check_block_calls(
                    sources,
                    &else_arm.body,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    loop_depth,
                );
            }
        }
        Stmt::While(statement) => {
            check_expression_calls(
                sources,
                &statement.condition,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_while_condition(sources, statement, resolved, diagnostics, environment);

            let mut body_environment = environment.clone();
            check_block_calls(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                loop_depth + 1,
            );
        }
        Stmt::WhileLet(statement) => {
            check_expression_calls(
                sources,
                &statement.initializer,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_while_let_initializer(sources, statement, resolved, diagnostics, environment);

            let mut body_environment =
                environment_for_while_let_binding(statement, resolved, environment);
            check_block_calls(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                loop_depth + 1,
            );
        }
        Stmt::ForRange(statement) => {
            check_expression_calls(
                sources,
                &statement.start,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_calls(
                sources,
                &statement.end,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_for_range_bounds(sources, statement, resolved, diagnostics, environment);

            let mut body_environment =
                environment_for_for_range_binding(statement, resolved, environment);
            check_block_calls(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                loop_depth + 1,
            );
        }
        Stmt::Loop(statement) => {
            let mut body_environment = environment.clone();
            check_block_calls(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                loop_depth + 1,
            );
        }
        Stmt::Break(statement) => {
            if loop_depth == 0 {
                diagnostics.push(loop_control_outside_loop_diagnostic(
                    sources,
                    statement.span,
                    "break",
                ));
            }
        }
        Stmt::Continue(statement) => {
            if loop_depth == 0 {
                diagnostics.push(loop_control_outside_loop_diagnostic(
                    sources,
                    statement.span,
                    "continue",
                ));
            }
        }
        Stmt::Expression(statement) => {
            check_expression_calls(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
    }
}

fn check_expression_calls(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    loop_depth: usize,
) {
    match expression {
        Expr::Try(expression) => {
            check_expression_calls(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
        Expr::TryCatch(expression) => {
            check_expression_calls(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            let mut catch_environment = environment_for_catch(
                expression.error_name.clone(),
                &expression.expression,
                resolved,
                environment,
            );
            check_block_calls(
                sources,
                &expression.catch_block,
                resolved,
                diagnostics,
                &mut catch_environment,
                loop_depth,
            );
        }
        Expr::Binary(expression) => {
            check_expression_calls(
                sources,
                &expression.left,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_calls(
                sources,
                &expression.right,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_binary_expression(sources, expression, resolved, diagnostics, environment);
        }
        Expr::Unary(expression) => {
            check_expression_calls(
                sources,
                &expression.operand,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_unary_expression(sources, expression, resolved, diagnostics, environment);
        }
        Expr::TypeConversion(expression) => {
            check_expression_calls(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_type_conversion_expression(
                sources,
                expression,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::Call(expression) => {
            if !is_enum_variant_call(expression, resolved) {
                check_expression_calls(
                    sources,
                    &expression.callee,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
            for argument in &expression.arguments {
                check_expression_calls(
                    sources,
                    argument,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
            check_enum_variant_call(sources, expression, resolved, diagnostics, environment);

            if let Some(signature) = resolved.function_signature_for_call(expression) {
                check_known_function_call(
                    sources,
                    expression,
                    signature,
                    resolved,
                    diagnostics,
                    environment,
                );
            }
        }
        Expr::Member(expression) => {
            check_expression_calls(
                sources,
                &expression.object,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_enum_variant_member(sources, expression, resolved, diagnostics);
        }
        Expr::Index(expression) => {
            check_expression_calls(
                sources,
                &expression.object,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_calls(
                sources,
                &expression.index,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_index_expression(sources, expression, resolved, diagnostics, environment);
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression_calls(
                    sources,
                    element,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
            check_array_literal_elements(sources, expression, resolved, diagnostics, environment);
        }
        Expr::Group(expression) => {
            check_expression_calls(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
        Expr::OptionalDefault(expression) => {
            check_expression_calls(
                sources,
                &expression.value,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_calls(
                sources,
                &expression.default,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_known_function_call(
    sources: &SourceMap,
    call: &CallExpr,
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    if call.arguments.len() != signature.parameters.len() {
        diagnostics.push(argument_count_mismatch_diagnostic(
            sources,
            call,
            signature.parameters.len(),
            call.arguments.len(),
            resolved,
        ));
        return;
    }

    for (index, (argument, parameter)) in call
        .arguments
        .iter()
        .zip(signature.parameters.iter())
        .enumerate()
    {
        let expected = type_expr_to_type(&parameter.ty, resolved);
        let actual = expression_type(argument, resolved, environment);
        if actual.is_unknown_or_unresolved() || expected.is_unknown_or_unresolved() {
            continue;
        }

        if !is_expression_assignable(&expected, argument, resolved, environment) {
            diagnostics.push(argument_type_mismatch_diagnostic(
                sources, index, argument, parameter, &expected, &actual,
            ));
        }
    }
}

fn block_guarantees_return(block: &Block) -> bool {
    block
        .statements
        .last()
        .is_some_and(statement_guarantees_return)
}

fn statement_guarantees_return(statement: &Stmt) -> bool {
    match statement {
        Stmt::Return(_) | Stmt::Fail(_) => true,
        Stmt::If(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&statement.then_block) && block_guarantees_return(else_block)
        }),
        Stmt::IfIs(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&statement.then_block) && block_guarantees_return(else_block)
        }),
        Stmt::IfLet(statement) => statement.else_block.as_ref().is_some_and(|else_block| {
            block_guarantees_return(&statement.then_block) && block_guarantees_return(else_block)
        }),
        Stmt::Switch(statement) => statement.else_arm.as_ref().is_some_and(|else_arm| {
            statement
                .arms
                .iter()
                .all(|arm| block_guarantees_return(&arm.body))
                && block_guarantees_return(&else_arm.body)
        }),
        Stmt::Loop(statement) => block_guarantees_return(&statement.body),
        Stmt::Binding(_)
        | Stmt::Try(_)
        | Stmt::TryCatch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::WhileLet(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Expression(_) => false,
    }
}

fn environment_for_parameters(
    parameters: &[Parameter],
    resolved: &ResolveOutput,
) -> TypeEnvironment {
    let mut environment = TypeEnvironment::default();
    for parameter in parameters {
        environment.define(
            parameter.name.clone(),
            type_expr_to_type(&parameter.ty, resolved),
        );
    }
    environment
}

fn environment_for_catch(
    error_name: String,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut catch_environment = environment.clone();
    let error_type = match expression_type(expression, resolved, environment) {
        Type::Fallible { error, .. } => *error,
        _ => Type::Unknown,
    };
    catch_environment.define(error_name, error_type);
    catch_environment
}

fn environment_for_if_let_binding(
    statement: &IfLetStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut then_environment = environment.clone();
    then_environment.define(
        statement.name.clone(),
        if_let_binding_type(statement, resolved, environment),
    );
    then_environment
}

fn environment_for_if_is_binding(
    statement: &IfIsStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut then_environment = environment.clone();
    if let Some(payload) = &statement.payload {
        then_environment.define(
            payload.name.clone(),
            enum_pattern_payload_type(&statement.enum_name, &statement.variant_name, resolved)
                .unwrap_or(Type::Unknown),
        );
    }
    then_environment
}

fn if_let_binding_type(
    statement: &IfLetStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression_type(&statement.initializer, resolved, environment) {
        Type::Optional(inner) => *inner,
        Type::Unknown => Type::Unknown,
        _ => Type::Unknown,
    }
}

fn check_if_let_initializer(
    sources: &SourceMap,
    statement: &IfLetStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let initializer_type = expression_type(&statement.initializer, resolved, environment);
    if initializer_type.is_unknown() || matches!(initializer_type, Type::Optional(_)) {
        return;
    }

    diagnostics.push(optional_if_let_non_optional_diagnostic(
        sources,
        statement,
        &initializer_type,
    ));
}

fn environment_for_while_let_binding(
    statement: &WhileLetStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut body_environment = environment.clone();
    body_environment.define(
        statement.name.clone(),
        while_let_binding_type(statement, resolved, environment),
    );
    body_environment
}

fn while_let_binding_type(
    statement: &WhileLetStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression_type(&statement.initializer, resolved, environment) {
        Type::Optional(inner) => *inner,
        Type::Unknown => Type::Unknown,
        _ => Type::Unknown,
    }
}

fn check_while_condition(
    sources: &SourceMap,
    statement: &WhileStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let condition_type = expression_type(&statement.condition, resolved, environment);
    if condition_type.is_unknown_or_unresolved() || is_bool_type(&condition_type) {
        return;
    }

    diagnostics.push(while_condition_type_mismatch_diagnostic(
        sources,
        &statement.condition,
        &condition_type,
    ));
}

fn check_while_let_initializer(
    sources: &SourceMap,
    statement: &WhileLetStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let initializer_type = expression_type(&statement.initializer, resolved, environment);
    if initializer_type.is_unknown() || matches!(initializer_type, Type::Optional(_)) {
        return;
    }

    diagnostics.push(optional_while_let_non_optional_diagnostic(
        sources,
        statement,
        &initializer_type,
    ));
}

fn environment_for_switch_arm(
    arm: &SwitchArm,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut arm_environment = environment.clone();
    if let Some(payload) = &arm.payload {
        arm_environment.define(
            payload.name.clone(),
            switch_arm_payload_type(arm, resolved).unwrap_or(Type::Unknown),
        );
    }
    arm_environment
}

fn switch_arm_payload_type(arm: &SwitchArm, resolved: &ResolveOutput) -> Option<Type> {
    enum_pattern_payload_type(&arm.enum_name, &arm.variant_name, resolved)
}

fn enum_pattern_payload_type(
    enum_name: &str,
    variant_name: &str,
    resolved: &ResolveOutput,
) -> Option<Type> {
    let symbol = resolved.type_symbol_by_name(enum_name)?;
    if symbol.kind != TypeSymbolKind::Enum {
        return None;
    }

    let variant = symbol
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)?;
    let [payload] = variant.payload.as_slice() else {
        return None;
    };

    Some(type_expr_to_type(&payload.ty, resolved))
}

fn check_switch_statement(
    sources: &SourceMap,
    statement: &SwitchStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target_type = expression_type(&statement.expression, resolved, environment);
    if target_type.is_unknown_or_unresolved() {
        return;
    }

    let target_symbol = enum_type_symbol_for_type(&target_type, resolved);
    if target_symbol.is_none() {
        diagnostics.push(switch_target_type_mismatch_diagnostic(
            sources,
            statement,
            &target_type,
        ));
    }

    for arm in &statement.arms {
        check_switch_arm_pattern(sources, arm, target_symbol, resolved, diagnostics);
    }
}

fn check_if_is_statement(
    sources: &SourceMap,
    statement: &IfIsStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target_type = expression_type(&statement.expression, resolved, environment);
    if target_type.is_unknown_or_unresolved() {
        return;
    }

    let target_symbol = enum_type_symbol_for_type(&target_type, resolved);
    if target_symbol.is_none() {
        diagnostics.push(if_is_target_type_mismatch_diagnostic(
            sources,
            statement,
            &target_type,
        ));
    }

    check_if_is_pattern(sources, statement, target_symbol, resolved, diagnostics);
}

fn check_if_is_pattern(
    sources: &SourceMap,
    statement: &IfIsStmt,
    target_symbol: Option<&TypeSymbol>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(pattern_symbol) = resolved.type_symbol_by_name(&statement.enum_name) else {
        diagnostics.push(if_is_unknown_enum_diagnostic(sources, statement));
        return;
    };

    if pattern_symbol.kind != TypeSymbolKind::Enum {
        diagnostics.push(if_is_non_enum_diagnostic(
            sources,
            statement,
            pattern_symbol,
        ));
        return;
    }

    if let Some(target_symbol) = target_symbol
        && target_symbol.canonical_name != pattern_symbol.canonical_name
    {
        diagnostics.push(if_is_enum_mismatch_diagnostic(
            sources,
            statement,
            target_symbol,
            pattern_symbol,
        ));
        return;
    }

    let Some(variant) = pattern_symbol
        .variants
        .iter()
        .find(|variant| variant.name == statement.variant_name)
    else {
        diagnostics.push(if_is_unknown_variant_diagnostic(
            sources,
            statement,
            pattern_symbol,
        ));
        return;
    };

    let provided_payload_count = usize::from(statement.payload.is_some());
    if variant.payload.len() != provided_payload_count {
        diagnostics.push(if_is_payload_mismatch_diagnostic(
            sources,
            statement,
            pattern_symbol,
            variant.payload.len(),
            provided_payload_count,
        ));
    }
}

fn check_switch_arm_pattern(
    sources: &SourceMap,
    arm: &SwitchArm,
    target_symbol: Option<&TypeSymbol>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(pattern_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
        diagnostics.push(switch_arm_unknown_enum_diagnostic(sources, arm));
        return;
    };

    if pattern_symbol.kind != TypeSymbolKind::Enum {
        diagnostics.push(switch_arm_non_enum_diagnostic(sources, arm, pattern_symbol));
        return;
    }

    if let Some(target_symbol) = target_symbol
        && target_symbol.canonical_name != pattern_symbol.canonical_name
    {
        diagnostics.push(switch_arm_enum_mismatch_diagnostic(
            sources,
            arm,
            target_symbol,
            pattern_symbol,
        ));
        return;
    }

    let Some(variant) = pattern_symbol
        .variants
        .iter()
        .find(|variant| variant.name == arm.variant_name)
    else {
        diagnostics.push(switch_arm_unknown_variant_diagnostic(
            sources,
            arm,
            pattern_symbol,
        ));
        return;
    };

    let provided_payload_count = usize::from(arm.payload.is_some());
    if variant.payload.len() != provided_payload_count {
        diagnostics.push(switch_arm_payload_mismatch_diagnostic(
            sources,
            arm,
            pattern_symbol,
            variant.payload.len(),
            provided_payload_count,
        ));
    }
}

fn enum_type_symbol_for_type<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a TypeSymbol> {
    let Type::Named(canonical_name) = ty else {
        return None;
    };

    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Enum)
}

fn is_enum_variant_call(call: &CallExpr, resolved: &ResolveOutput) -> bool {
    enum_member_for_call(call)
        .and_then(|member| enum_symbol_for_member(member, resolved))
        .is_some()
}

fn enum_variant_member_type(member: &MemberExpr, resolved: &ResolveOutput) -> Option<Type> {
    enum_symbol_for_member(member, resolved)
        .map(|symbol| Type::Named(symbol.canonical_name.clone()))
}

fn enum_variant_call_type(call: &CallExpr, resolved: &ResolveOutput) -> Option<Type> {
    enum_member_for_call(call).and_then(|member| enum_variant_member_type(member, resolved))
}

fn check_enum_variant_member(
    sources: &SourceMap,
    member: &MemberExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(enum_symbol) = enum_symbol_for_member(member, resolved) else {
        return;
    };

    let Some(variant) = enum_variant_for_member(member, enum_symbol) else {
        diagnostics.push(enum_variant_unknown_diagnostic(
            sources,
            member,
            enum_symbol,
        ));
        return;
    };

    if !variant.payload.is_empty() {
        diagnostics.push(enum_variant_payload_count_mismatch_diagnostic(
            sources,
            member.member_span,
            enum_symbol,
            variant,
            variant.payload.len(),
            0,
        ));
    }
}

fn check_enum_variant_call(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let Some(member) = enum_member_for_call(call) else {
        return;
    };
    let Some(enum_symbol) = enum_symbol_for_member(member, resolved) else {
        return;
    };

    let Some(variant) = enum_variant_for_member(member, enum_symbol) else {
        diagnostics.push(enum_variant_unknown_diagnostic(
            sources,
            member,
            enum_symbol,
        ));
        return;
    };

    if variant.payload.is_empty() && call.arguments.is_empty() {
        diagnostics.push(enum_variant_payloadless_call_diagnostic(
            sources,
            call,
            enum_symbol,
            variant,
        ));
        return;
    }

    if variant.payload.len() != call.arguments.len() {
        diagnostics.push(enum_variant_payload_count_mismatch_diagnostic(
            sources,
            call.arguments_span,
            enum_symbol,
            variant,
            variant.payload.len(),
            call.arguments.len(),
        ));
        return;
    }

    for (index, (argument, parameter)) in call
        .arguments
        .iter()
        .zip(variant.payload.iter())
        .enumerate()
    {
        let expected = type_expr_to_type(&parameter.ty, resolved);
        let actual = expression_type(argument, resolved, environment);
        if expected.is_unknown_or_unresolved() || actual.is_unknown_or_unresolved() {
            continue;
        }

        if !is_expression_assignable(&expected, argument, resolved, environment) {
            diagnostics.push(enum_variant_payload_type_mismatch_diagnostic(
                sources,
                argument,
                enum_symbol,
                variant,
                index,
                &expected,
                &actual,
            ));
        }
    }
}

fn enum_symbol_for_member<'a>(
    member: &MemberExpr,
    resolved: &'a ResolveOutput,
) -> Option<&'a TypeSymbol> {
    let Expr::Identifier(enum_name) = member.object.as_ref() else {
        return None;
    };

    resolved
        .type_symbol_by_name(&enum_name.name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Enum)
}

fn enum_variant_for_member<'a>(
    member: &MemberExpr,
    enum_symbol: &'a TypeSymbol,
) -> Option<&'a EnumVariantSignature> {
    enum_symbol
        .variants
        .iter()
        .find(|variant| variant.name == member.member)
}

fn enum_member_for_call(call: &CallExpr) -> Option<&MemberExpr> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };

    Some(member)
}

fn environment_for_for_range_binding(
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut body_environment = environment.clone();
    body_environment.define(
        statement.name.clone(),
        for_range_binding_type(statement, resolved, environment),
    );
    body_environment
}

fn for_range_binding_type(
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let start_type = expression_type(&statement.start, resolved, environment);
    let end_type = expression_type(&statement.end, resolved, environment);

    if start_type.is_unknown_or_unresolved() || end_type.is_unknown_or_unresolved() {
        return Type::Unknown;
    }

    if is_integer_type(&start_type) && same_known_type(&start_type, &end_type) {
        return start_type;
    }

    if is_integer_type(&start_type)
        && is_integer_literal_expr(&statement.end)
        && is_expression_assignable(&start_type, &statement.end, resolved, environment)
    {
        return start_type;
    }

    if is_integer_type(&end_type)
        && is_integer_literal_expr(&statement.start)
        && is_expression_assignable(&end_type, &statement.start, resolved, environment)
    {
        return end_type;
    }

    Type::Unknown
}

fn check_for_range_bounds(
    sources: &SourceMap,
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let start_type = expression_type(&statement.start, resolved, environment);
    let end_type = expression_type(&statement.end, resolved, environment);

    if start_type.is_unknown_or_unresolved() || end_type.is_unknown_or_unresolved() {
        return;
    }

    if is_integer_type(&start_type)
        && is_integer_type(&end_type)
        && integer_operands_match(
            &start_type,
            &statement.start,
            &end_type,
            &statement.end,
            resolved,
            environment,
        )
    {
        return;
    }

    diagnostics.push(for_range_bound_type_mismatch_diagnostic(
        sources,
        statement,
        &start_type,
        &end_type,
    ));
}

fn check_optional_let_else_statement(
    sources: &SourceMap,
    statement: &BindingStmt,
    initializer_type: &Type,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !initializer_type.is_unknown() && !matches!(initializer_type, Type::Optional(_)) {
        diagnostics.push(optional_let_else_non_optional_diagnostic(
            sources,
            statement,
            initializer_type,
        ));
    }

    if let Some(else_block) = &statement.else_block
        && !block_guarantees_return(else_block)
    {
        diagnostics.push(optional_let_else_fallthrough_diagnostic(
            sources, statement, else_block,
        ));
    }
}

fn continuing_binding_type(
    statement: &BindingStmt,
    initializer_type: Type,
    resolved: &ResolveOutput,
) -> Type {
    let inferred = if statement.else_block.is_some() {
        match initializer_type {
            Type::Optional(inner) => *inner,
            Type::Unknown => Type::Unknown,
            _ => Type::Unknown,
        }
    } else {
        initializer_type
    };

    if let Some(ty) = &statement.ty {
        return type_expr_to_type(ty, resolved);
    }

    inferred
}

fn optional_default_type(value_type: Type, default_type: Type) -> Type {
    let Type::Optional(inner) = value_type else {
        return default_type;
    };

    if default_type.is_unknown() || is_assignable(&inner, &default_type) {
        *inner
    } else {
        default_type
    }
}

fn check_if_condition(
    sources: &SourceMap,
    statement: &IfStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let condition_type = expression_type(&statement.condition, resolved, environment);
    if condition_type.is_unknown_or_unresolved() || is_bool_type(&condition_type) {
        return;
    }

    diagnostics.push(if_condition_type_mismatch_diagnostic(
        sources,
        &statement.condition,
        &condition_type,
    ));
}

fn check_binary_expression(
    sources: &SourceMap,
    expression: &BinaryExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let left_type = expression_type(&expression.left, resolved, environment);
    let right_type = expression_type(&expression.right, resolved, environment);

    if left_type.is_unknown_or_unresolved() || right_type.is_unknown_or_unresolved() {
        return;
    }

    match expression.operator {
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder => {
            if !arithmetic_operands_match(
                &left_type,
                &expression.left,
                &right_type,
                &expression.right,
                resolved,
                environment,
            ) {
                diagnostics.push(arithmetic_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            }
        }
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            if !shift_operands_match(&left_type, &right_type) {
                diagnostics.push(shift_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            } else if is_negative_integer_literal_expr(&expression.right) {
                diagnostics.push(negative_shift_count_diagnostic(sources, expression));
            }
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            if !equality_operands_match(
                &left_type,
                &expression.left,
                &right_type,
                &expression.right,
                resolved,
                environment,
            ) {
                diagnostics.push(equality_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            }
        }
        BinaryOperator::Less
        | BinaryOperator::LessEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterEqual => {
            if !ordered_comparison_operands_match(
                &left_type,
                &expression.left,
                &right_type,
                &expression.right,
                resolved,
                environment,
            ) {
                diagnostics.push(ordered_comparison_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            }
        }
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
            if !logical_operands_match(&left_type, &right_type) {
                diagnostics.push(logical_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &left_type,
                    &right_type,
                ));
            }
        }
    }
}

fn check_unary_expression(
    sources: &SourceMap,
    expression: &UnaryExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let operand_type = expression_type(&expression.operand, resolved, environment);
    if operand_type.is_unknown_or_unresolved() {
        return;
    }

    match expression.operator {
        UnaryOperator::LogicalNot => {
            if !is_bool_type(&operand_type) {
                diagnostics.push(logical_not_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &operand_type,
                ));
            }
        }
        UnaryOperator::Negate => {
            if !is_signed_integer_type(&operand_type) {
                diagnostics.push(numeric_negate_operand_type_mismatch_diagnostic(
                    sources,
                    expression,
                    &operand_type,
                ));
            }
        }
    }
}

fn check_type_conversion_expression(
    sources: &SourceMap,
    expression: &TypeConversionExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let source_type = expression_type(&expression.expression, resolved, environment);
    let target_type = type_expr_to_type(&expression.ty, resolved);
    if source_type.is_unknown_or_unresolved() || target_type.is_unknown_or_unresolved() {
        return;
    }

    if !is_lossless_integer_conversion(
        &source_type,
        &expression.expression,
        &target_type,
        resolved,
        environment,
    ) {
        diagnostics.push(type_conversion_not_lossless_diagnostic(
            sources,
            expression,
            &source_type,
            &target_type,
        ));
    }
}

fn check_binding_annotation(
    sources: &SourceMap,
    statement: &BindingStmt,
    initializer_type: &Type,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let Some(annotation) = &statement.ty else {
        return;
    };

    let binding_type = type_expr_to_type(annotation, resolved);
    let expected_initializer = if statement.else_block.is_some() {
        Type::Optional(Box::new(binding_type.clone()))
    } else {
        binding_type.clone()
    };

    if initializer_type.is_unknown_or_unresolved()
        || expected_initializer.is_unknown_or_unresolved()
    {
        return;
    }

    if !is_expression_assignable(
        &expected_initializer,
        &statement.initializer,
        resolved,
        environment,
    ) {
        diagnostics.push(binding_type_mismatch_diagnostic(
            sources,
            statement,
            &binding_type,
            initializer_type,
        ));
    }
}

fn check_array_literal_elements(
    sources: &SourceMap,
    array: &ArrayLiteralExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let mut first_known: Option<(&Expr, Type)> = None;

    for element in &array.elements {
        let element_type = expression_type(element, resolved, environment);
        if element_type.is_unknown_or_unresolved() {
            continue;
        }

        let Some((first_element, first_type)) = &first_known else {
            first_known = Some((element, element_type));
            continue;
        };

        if !same_known_type(first_type, &element_type) {
            diagnostics.push(array_literal_element_type_mismatch_diagnostic(
                sources,
                element,
                &element_type,
                first_element,
                first_type,
            ));
            return;
        }
    }
}

fn array_literal_type(
    array: &ArrayLiteralExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let element = infer_array_literal_element_type(&array.elements, resolved, environment)
        .unwrap_or(Type::Unknown);

    Type::Array {
        element: Box::new(element),
        length: array.elements.len().to_string(),
    }
}

fn infer_array_literal_element_type(
    elements: &[Expr],
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    let mut inferred: Option<Type> = None;

    for element in elements {
        let element_type = expression_type(element, resolved, environment);
        if element_type.is_unknown_or_unresolved() {
            continue;
        }

        match &inferred {
            Some(current) if !same_known_type(current, &element_type) => return None,
            Some(_) => {}
            None => inferred = Some(element_type),
        }
    }

    inferred
}

fn check_index_expression(
    sources: &SourceMap,
    index: &IndexExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target = expression_type(&index.object, resolved, environment);
    if !target.is_unknown_or_unresolved() && !is_indexable_type(&target) {
        diagnostics.push(index_target_type_mismatch_diagnostic(
            sources, index, &target,
        ));
    }

    let index_type = expression_type(&index.index, resolved, environment);
    if !index_type.is_unknown_or_unresolved() && !is_integer_type(&index_type) {
        diagnostics.push(index_value_type_mismatch_diagnostic(
            sources,
            index,
            &index_type,
        ));
    }
}

fn index_expression_type(
    index: &IndexExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression_type(&index.object, resolved, environment) {
        Type::Array { element, .. } | Type::View { element, .. } => *element,
        Type::Str => Type::Primitive("u8".to_string()),
        _ => Type::Unknown,
    }
}

fn is_indexable_type(ty: &Type) -> bool {
    matches!(ty, Type::Array { .. } | Type::View { .. } | Type::Str)
}

fn type_expr_to_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Type {
    match ty {
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "i32" => Type::I32,
            "bool" | "i8" | "i16" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" => {
                Type::Primitive(reference.name.clone())
            }
            "str" => Type::Str,
            "error" => Type::Error,
            "void" => Type::Void,
            "never" => Type::Never,
            name => resolved
                .type_symbol_by_name(name)
                .map(|symbol| Type::Named(symbol.canonical_name.clone()))
                .unwrap_or_else(|| Type::Unresolved(name.to_string())),
        },
        TypeExpr::Generic(_) | TypeExpr::Pointer(_) | TypeExpr::Borrow(_) => {
            type_expr_display(ty, resolved)
                .map(Type::Named)
                .unwrap_or_else(|| Type::Unresolved(type_expr_display_lossy(ty)))
        }
        TypeExpr::View(ty) => Type::View {
            is_readwrite: ty.is_readwrite,
            element: Box::new(type_expr_to_type(&ty.element, resolved)),
        },
        TypeExpr::Array(ty) => Type::Array {
            element: Box::new(type_expr_to_type(&ty.element, resolved)),
            length: ty.length.value.clone(),
        },
        TypeExpr::Optional(ty) => Type::Optional(Box::new(type_expr_to_type(&ty.inner, resolved))),
        TypeExpr::Fallible(ty) => Type::Fallible {
            success: Box::new(type_expr_to_type(&ty.success, resolved)),
            error: Box::new(type_expr_to_type(&ty.error, resolved)),
        },
    }
}

fn type_expr_display(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<String> {
    match ty {
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize"
            | "isize" | "str" | "error" | "void" | "never" => Some(reference.name.clone()),
            name => resolved
                .type_symbol_by_name(name)
                .map(|symbol| symbol.canonical_name.clone()),
        },
        TypeExpr::Generic(generic) => {
            let arguments = generic
                .arguments
                .iter()
                .map(|argument| type_expr_display(argument, resolved))
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            let name = resolved
                .type_symbol_by_name(&generic.name)
                .map(|symbol| symbol.canonical_name.clone())?;
            Some(format!("{name}<{arguments}>"))
        }
        TypeExpr::Pointer(pointer) => {
            Some(format!("*{}", type_expr_display(&pointer.inner, resolved)?))
        }
        TypeExpr::Borrow(borrow) if borrow.is_readwrite => {
            Some(format!("&+{}", type_expr_display(&borrow.inner, resolved)?))
        }
        TypeExpr::Borrow(borrow) => {
            Some(format!("&{}", type_expr_display(&borrow.inner, resolved)?))
        }
        TypeExpr::View(view) if view.is_readwrite => Some(format!(
            "[+{}]",
            type_expr_display(&view.element, resolved)?
        )),
        TypeExpr::View(view) => Some(format!("[{}]", type_expr_display(&view.element, resolved)?)),
        TypeExpr::Array(array) => Some(format!(
            "[{}; {}]",
            type_expr_display(&array.element, resolved)?,
            array.length.value
        )),
        TypeExpr::Optional(optional) => Some(format!(
            "{}?",
            type_expr_display(&optional.inner, resolved)?
        )),
        TypeExpr::Fallible(fallible) => Some(format!(
            "{}!",
            type_expr_display(&fallible.success, resolved)?
        )),
    }
}

fn type_expr_display_lossy(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Reference(reference) => reference.name.clone(),
        TypeExpr::Generic(generic) => {
            let arguments = generic
                .arguments
                .iter()
                .map(type_expr_display_lossy)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", generic.name)
        }
        TypeExpr::Pointer(pointer) => format!("*{}", type_expr_display_lossy(&pointer.inner)),
        TypeExpr::Borrow(borrow) if borrow.is_readwrite => {
            format!("&+{}", type_expr_display_lossy(&borrow.inner))
        }
        TypeExpr::Borrow(borrow) => format!("&{}", type_expr_display_lossy(&borrow.inner)),
        TypeExpr::View(view) if view.is_readwrite => {
            format!("[+{}]", type_expr_display_lossy(&view.element))
        }
        TypeExpr::View(view) => format!("[{}]", type_expr_display_lossy(&view.element)),
        TypeExpr::Array(array) => {
            format!(
                "[{}; {}]",
                type_expr_display_lossy(&array.element),
                array.length.value
            )
        }
        TypeExpr::Optional(optional) => format!("{}?", type_expr_display_lossy(&optional.inner)),
        TypeExpr::Fallible(fallible) => format!("{}!", type_expr_display_lossy(&fallible.success)),
    }
}

fn expression_type(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression {
        Expr::IntegerLiteral(_) => Type::I32,
        Expr::StringLiteral(_) => Type::Str,
        Expr::BoolLiteral(_) => Type::Primitive("bool".to_string()),
        Expr::NoneLiteral(_) => Type::None,
        Expr::ArrayLiteral(expression) => array_literal_type(expression, resolved, environment),
        Expr::Binary(expression) => binary_expression_type(expression, resolved, environment),
        Expr::Unary(expression) => match expression.operator {
            UnaryOperator::LogicalNot => Type::Primitive("bool".to_string()),
            UnaryOperator::Negate => expression_type(&expression.operand, resolved, environment),
        },
        Expr::TypeConversion(expression) => type_expr_to_type(&expression.ty, resolved),
        Expr::Try(expression) => {
            expression_type(&expression.expression, resolved, environment).into_success_type()
        }
        Expr::TryCatch(expression) => {
            expression_type(&expression.expression, resolved, environment).into_success_type()
        }
        Expr::Call(expression) => {
            enum_variant_call_type(expression, resolved).unwrap_or_else(|| {
                resolved
                    .function_signature_for_call(expression)
                    .map(|signature| type_expr_to_type(&signature.return_type, resolved))
                    .unwrap_or(Type::Unknown)
            })
        }
        Expr::Group(expression) => expression_type(&expression.expression, resolved, environment),
        Expr::Index(expression) => index_expression_type(expression, resolved, environment),
        Expr::OptionalDefault(expression) => {
            let value_type = expression_type(&expression.value, resolved, environment);
            let default_type = expression_type(&expression.default, resolved, environment);
            optional_default_type(value_type, default_type)
        }
        Expr::Identifier(expression) => environment
            .get(&expression.name)
            .cloned()
            .unwrap_or(Type::Unknown),
        Expr::Member(expression) => {
            enum_variant_member_type(expression, resolved).unwrap_or(Type::Unknown)
        }
    }
}

fn check_try_propagation(
    sources: &SourceMap,
    try_span: ByteSpan,
    expression: &Expr,
    context: &ReturnContext,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let attempted = expression_type(expression, resolved, environment);
    let Type::Fallible {
        error: attempted_error,
        ..
    } = attempted
    else {
        if !attempted.is_unknown() {
            diagnostics.push(try_on_non_fallible_diagnostic(
                sources, try_span, &attempted,
            ));
        }
        return;
    };

    let Type::Fallible {
        error: current_error,
        ..
    } = &context.declared_type
    else {
        diagnostics.push(try_in_non_fallible_context_diagnostic(
            sources,
            try_span,
            context,
            &attempted_error,
        ));
        return;
    };

    if !same_known_type(current_error, &attempted_error) {
        diagnostics.push(try_error_type_mismatch_diagnostic(
            sources,
            try_span,
            context,
            current_error,
            &attempted_error,
        ));
    }
}

fn check_try_catch_operand(
    sources: &SourceMap,
    try_span: ByteSpan,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let attempted = expression_type(expression, resolved, environment);
    if attempted.is_unknown() || matches!(attempted, Type::Fallible { .. }) {
        return;
    }

    diagnostics.push(try_on_non_fallible_diagnostic(
        sources, try_span, &attempted,
    ));
}

fn is_assignable(expected: &Type, actual: &Type) -> bool {
    if actual == &Type::Never {
        return true;
    }

    match (expected, actual) {
        (Type::Optional(_), Type::None) => true,
        (Type::Optional(expected_inner), Type::Optional(actual_inner)) => {
            is_assignable(expected_inner, actual_inner)
        }
        (Type::Optional(inner), actual) => is_assignable(inner, actual),
        _ => expected == actual,
    }
}

fn is_expression_assignable(
    expected: &Type,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if matches!(expected, Type::Unknown | Type::Unresolved(_)) {
        return true;
    }

    match (expected, expression) {
        (Type::Optional(_), Expr::NoneLiteral(_)) => true,
        (Type::Optional(inner), _) => {
            let actual = expression_type(expression, resolved, environment);
            is_assignable(expected, &actual)
                || is_expression_assignable(inner, expression, resolved, environment)
        }
        (_, Expr::IntegerLiteral(literal)) if is_integer_type(expected) => {
            integer_literal_fits_type(literal, expected)
        }
        (_, Expr::Unary(unary))
            if unary.operator == UnaryOperator::Negate
                && integer_literal_expr_value(&unary.operand).is_some() =>
        {
            negative_integer_literal_fits_type(unary, expected)
        }
        (Type::Array { element, length }, Expr::ArrayLiteral(literal)) => {
            array_length_matches(length, literal.elements.len())
                && literal.elements.iter().all(|element_expr| {
                    is_expression_assignable(element, element_expr, resolved, environment)
                })
        }
        (_, Expr::Group(group)) => {
            is_expression_assignable(expected, &group.expression, resolved, environment)
        }
        _ => {
            let actual = expression_type(expression, resolved, environment);
            is_assignable(expected, &actual)
        }
    }
}

fn array_length_matches(expected: &str, actual: usize) -> bool {
    integer_literal_value(expected).is_some_and(|value| value == actual as u128)
}

fn is_integer_type(ty: &Type) -> bool {
    matches!(ty, Type::I32)
        || matches!(ty, Type::Primitive(name) if integer_type_max(name).is_some())
}

fn is_signed_integer_type(ty: &Type) -> bool {
    matches!(ty, Type::I32)
        || matches!(ty, Type::Primitive(name) if signed_integer_type_min_abs(name).is_some())
}

fn is_bool_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(name) if name == "bool")
}

fn is_str_type(ty: &Type) -> bool {
    matches!(ty, Type::Str)
}

fn equality_operands_match(
    left_type: &Type,
    left: &Expr,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if is_bool_type(left_type) || is_bool_type(right_type) {
        return is_bool_type(left_type) && is_bool_type(right_type);
    }

    if is_str_type(left_type) || is_str_type(right_type) {
        return is_str_type(left_type) && is_str_type(right_type);
    }

    integer_operands_match(left_type, left, right_type, right, resolved, environment)
}

fn arithmetic_operands_match(
    left_type: &Type,
    left: &Expr,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    is_integer_type(left_type)
        && is_integer_type(right_type)
        && integer_operands_match(left_type, left, right_type, right, resolved, environment)
}

fn ordered_comparison_operands_match(
    left_type: &Type,
    left: &Expr,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    is_integer_type(left_type)
        && is_integer_type(right_type)
        && integer_operands_match(left_type, left, right_type, right, resolved, environment)
}

fn shift_operands_match(left_type: &Type, right_type: &Type) -> bool {
    is_integer_type(left_type) && is_integer_type(right_type)
}

fn is_lossless_integer_conversion(
    source_type: &Type,
    source: &Expr,
    target_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if !is_integer_type(target_type) {
        return false;
    }

    if is_integer_literal_expr(source) {
        return is_expression_assignable(target_type, source, resolved, environment);
    }

    let Some(source_range) = integer_type_range(source_type) else {
        return false;
    };
    let Some(target_range) = integer_type_range(target_type) else {
        return false;
    };

    target_range.min <= source_range.min && source_range.max <= target_range.max
}

fn integer_operands_match(
    left_type: &Type,
    left: &Expr,
    right_type: &Type,
    right: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    (is_integer_type(left_type)
        && is_integer_type(right_type)
        && same_known_type(left_type, right_type))
        || (is_integer_type(left_type)
            && is_integer_literal_expr(right)
            && is_expression_assignable(left_type, right, resolved, environment))
        || (is_integer_type(right_type)
            && is_integer_literal_expr(left)
            && is_expression_assignable(right_type, left, resolved, environment))
}

fn logical_operands_match(left_type: &Type, right_type: &Type) -> bool {
    is_bool_type(left_type) && is_bool_type(right_type)
}

fn binary_expression_type(
    expression: &BinaryExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression.operator {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterEqual
        | BinaryOperator::LogicalAnd
        | BinaryOperator::LogicalOr => Type::Primitive("bool".to_string()),
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            shift_expression_type(expression, resolved, environment)
        }
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Remainder => {
            arithmetic_expression_type(expression, resolved, environment)
        }
    }
}

fn shift_expression_type(
    expression: &BinaryExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let left_type = expression_type(&expression.left, resolved, environment);
    if is_integer_type(&left_type) {
        left_type
    } else {
        Type::Unknown
    }
}

fn arithmetic_expression_type(
    expression: &BinaryExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let left_type = expression_type(&expression.left, resolved, environment);
    let right_type = expression_type(&expression.right, resolved, environment);

    if left_type.is_unknown_or_unresolved() || right_type.is_unknown_or_unresolved() {
        return Type::Unknown;
    }

    if is_integer_type(&left_type)
        && is_integer_type(&right_type)
        && same_known_type(&left_type, &right_type)
    {
        return left_type;
    }

    if is_integer_type(&left_type)
        && is_integer_literal_expr(&expression.right)
        && is_expression_assignable(&left_type, &expression.right, resolved, environment)
    {
        return left_type;
    }

    if is_integer_type(&right_type)
        && is_integer_literal_expr(&expression.left)
        && is_expression_assignable(&right_type, &expression.left, resolved, environment)
    {
        return right_type;
    }

    Type::Unknown
}

fn is_integer_literal_expr(expression: &Expr) -> bool {
    match expression {
        Expr::IntegerLiteral(_) => true,
        Expr::Unary(unary) if unary.operator == UnaryOperator::Negate => {
            is_integer_literal_expr(&unary.operand)
        }
        Expr::Group(group) => is_integer_literal_expr(&group.expression),
        _ => false,
    }
}

fn is_negative_integer_literal_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Negate => {
            integer_literal_expr_value(&unary.operand).is_some()
        }
        Expr::Group(group) => is_negative_integer_literal_expr(&group.expression),
        _ => false,
    }
}

fn integer_literal_fits_type(literal: &LiteralExpr, ty: &Type) -> bool {
    let Some(value) = integer_literal_value(&literal.value) else {
        return false;
    };
    let Some(max) = integer_type_max(&ty.display()) else {
        return false;
    };
    value <= max
}

fn negative_integer_literal_fits_type(expression: &UnaryExpr, ty: &Type) -> bool {
    if !is_signed_integer_type(ty) {
        return false;
    }

    let Some(value) = integer_literal_expr_value(&expression.operand) else {
        return false;
    };
    let Some(min_abs) = signed_integer_type_min_abs(&ty.display()) else {
        return false;
    };
    value <= min_abs
}

fn integer_literal_expr_value(expression: &Expr) -> Option<u128> {
    match expression {
        Expr::IntegerLiteral(literal) => integer_literal_value(&literal.value),
        Expr::Group(group) => integer_literal_expr_value(&group.expression),
        _ => None,
    }
}

fn integer_type_max(name: &str) -> Option<u128> {
    match name {
        "i8" => Some(i8::MAX as u128),
        "i16" => Some(i16::MAX as u128),
        "i32" => Some(i32::MAX as u128),
        "i64" | "isize" => Some(i64::MAX as u128),
        "u8" => Some(u8::MAX as u128),
        "u16" => Some(u16::MAX as u128),
        "u32" => Some(u32::MAX as u128),
        "u64" | "usize" => Some(u64::MAX as u128),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerRange {
    min: i128,
    max: i128,
}

fn integer_type_range(ty: &Type) -> Option<IntegerRange> {
    integer_type_range_by_name(&ty.display())
}

fn integer_type_range_by_name(name: &str) -> Option<IntegerRange> {
    match name {
        "i8" => Some(IntegerRange {
            min: i8::MIN as i128,
            max: i8::MAX as i128,
        }),
        "i16" => Some(IntegerRange {
            min: i16::MIN as i128,
            max: i16::MAX as i128,
        }),
        "i32" => Some(IntegerRange {
            min: i32::MIN as i128,
            max: i32::MAX as i128,
        }),
        "i64" | "isize" => Some(IntegerRange {
            min: i64::MIN as i128,
            max: i64::MAX as i128,
        }),
        "u8" => Some(IntegerRange {
            min: 0,
            max: u8::MAX as i128,
        }),
        "u16" => Some(IntegerRange {
            min: 0,
            max: u16::MAX as i128,
        }),
        "u32" => Some(IntegerRange {
            min: 0,
            max: u32::MAX as i128,
        }),
        "u64" | "usize" => Some(IntegerRange {
            min: 0,
            max: u64::MAX as i128,
        }),
        _ => None,
    }
}

fn signed_integer_type_min_abs(name: &str) -> Option<u128> {
    match name {
        "i8" => Some(i8::MAX as u128 + 1),
        "i16" => Some(i16::MAX as u128 + 1),
        "i32" => Some(i32::MAX as u128 + 1),
        "i64" | "isize" => Some(i64::MAX as u128 + 1),
        _ => None,
    }
}

fn integer_literal_value(text: &str) -> Option<u128> {
    let (base, digits) = if let Some(rest) = text.strip_prefix("0x") {
        (16, rest)
    } else if let Some(rest) = text.strip_prefix("0b") {
        (2, rest)
    } else {
        (10, text)
    };
    let digits = digits.replace('_', "");
    u128::from_str_radix(&digits, base).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Type {
    I32,
    Primitive(String),
    Str,
    Error,
    Void,
    Never,
    None,
    View {
        is_readwrite: bool,
        element: Box<Type>,
    },
    Array {
        element: Box<Type>,
        length: String,
    },
    Optional(Box<Type>),
    Fallible {
        success: Box<Type>,
        error: Box<Type>,
    },
    Named(String),
    Unresolved(String),
    Unknown,
}

impl Type {
    fn display(&self) -> String {
        match self {
            Type::I32 => "i32".to_string(),
            Type::Primitive(name) => name.clone(),
            Type::Str => "str".to_string(),
            Type::Error => "error".to_string(),
            Type::Void => "void".to_string(),
            Type::Never => "never".to_string(),
            Type::None => "none".to_string(),
            Type::View {
                is_readwrite: true,
                element,
            } => format!("[+{}]", element.display()),
            Type::View {
                is_readwrite: false,
                element,
            } => format!("[{}]", element.display()),
            Type::Array { element, length } => format!("[{}; {}]", element.display(), length),
            Type::Optional(inner) => format!("{}?", inner.display()),
            Type::Fallible { success, .. } => format!("{}!", success.display()),
            Type::Named(name) => name.clone(),
            Type::Unresolved(name) => name.clone(),
            Type::Unknown => "<unknown>".to_string(),
        }
    }

    fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
    }

    fn is_unknown_or_unresolved(&self) -> bool {
        match self {
            Type::Unknown | Type::Unresolved(_) => true,
            Type::View { element, .. } => element.is_unknown_or_unresolved(),
            Type::Array { element, .. } => element.is_unknown_or_unresolved(),
            Type::Optional(inner) => inner.is_unknown_or_unresolved(),
            Type::Fallible { success, error } => {
                success.is_unknown_or_unresolved() || error.is_unknown_or_unresolved()
            }
            Type::I32
            | Type::Primitive(_)
            | Type::Str
            | Type::Error
            | Type::Void
            | Type::Never
            | Type::None
            | Type::Named(_) => false,
        }
    }

    fn success_type(&self) -> &Type {
        match self {
            Type::Fallible { success, .. } => success,
            _ => self,
        }
    }

    fn into_success_type(self) -> Type {
        match self {
            Type::Fallible { success, .. } => *success,
            _ => self,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct TypeEnvironment {
    bindings: HashMap<String, Type>,
}

impl TypeEnvironment {
    fn define(&mut self, name: String, ty: Type) {
        self.bindings.insert(name, ty);
    }

    fn get(&self, name: &str) -> Option<&Type> {
        self.bindings.get(name)
    }
}

fn same_known_type(left: &Type, right: &Type) -> bool {
    !left.is_unknown() && !right.is_unknown() && left == right
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReturnContext {
    kind: CallableKind,
    declared_type: Type,
    return_type_span: ByteSpan,
}

impl ReturnContext {
    fn new(kind: CallableKind, declared_type: Type, return_type_span: ByteSpan) -> Self {
        Self {
            kind,
            declared_type,
            return_type_span,
        }
    }

    fn success_type(&self) -> &Type {
        self.declared_type.success_type()
    }

    fn requires_explicit_return(&self) -> bool {
        let success_type = self.success_type();
        !matches!(
            success_type,
            Type::Void | Type::Unknown | Type::Unresolved(_)
        )
    }

    fn subject(&self) -> String {
        match &self.kind {
            CallableKind::Program => "`program`".to_string(),
            CallableKind::Function(name) => format!("function `{name}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CallableKind {
    Program,
    Function(String),
}

fn missing_program_diagnostic(sources: &SourceMap, span: ByteSpan) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0300",
        "executable root file must define exactly one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("add `program(): i32 { ... }` or `program(): void { ... }`".to_string());
    diagnostic
}

fn main_is_not_entry_diagnostic(sources: &SourceMap, function: &FunctionDecl) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0301",
        "`func main` is an ordinary function; Nocter executable entry uses `program`",
    );
    diagnostic.primary_span = sources.span_to_json(function.name_span).ok().map(Box::new);
    diagnostic.help = Some(
        "replace the entry declaration with `program(): i32 { ... }` or `program(): void { ... }`"
            .to_string(),
    );
    diagnostic
}

fn duplicate_program_diagnostic(
    sources: &SourceMap,
    first_span: ByteSpan,
    second_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0302",
        "executable root file must not define more than one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(second_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first `program` entry is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep exactly one top-level `program` declaration".to_string());
    diagnostic
}

fn invalid_program_return_type_diagnostic(
    sources: &SourceMap,
    return_type_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0303",
        "`program` return type must be `i32` or `void` in v0",
    );
    diagnostic.primary_span = sources.span_to_json(return_type_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `program(): i32` for an exit status or `program(): void` for status 0".to_string(),
    );
    diagnostic
}

fn missing_return_value_diagnostic(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
) -> Diagnostic {
    let expected = context.success_type();
    let mut diagnostic = Diagnostic::error(
        "E0310",
        format!(
            "`return` has no value, but {} returns `{}`",
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement.span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!("return a value of type `{}`", expected.display()));
    diagnostic
}

fn unexpected_return_value_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0311",
        format!(
            "`return` has a value, but {} returns `void`",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("remove the returned value or change the return type".to_string());
    diagnostic
}

fn return_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    expected: &Type,
    actual: &Type,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0312",
        format!(
            "`return` value has type `{}`, but {} returns `{}`",
            actual.display(),
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!("return a value of type `{}`", expected.display()));
    diagnostic
}

fn fail_in_non_fallible_context_diagnostic(
    sources: &SourceMap,
    statement: &FailStmt,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0333",
        format!(
            "`fail` is used in {}, but its return type is not fallible",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement.span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("use `fail` only inside a function returning `T!`".to_string());
    diagnostic
}

fn fail_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &FailStmt,
    expected: &Type,
    actual: &Type,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0334",
        format!(
            "`fail` value has type `{}`, but {} fails with `{}`",
            actual.display(),
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!(
        "fail with a value of type `{}`",
        expected.display()
    ));
    diagnostic
}

fn missing_return_diagnostic(
    sources: &SourceMap,
    block_span: ByteSpan,
    context: &ReturnContext,
) -> Diagnostic {
    let expected = context.success_type();
    let mut diagnostic = Diagnostic::error(
        "E0313",
        format!(
            "{} may reach the end without returning `{}`",
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(block_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!(
        "add a `return` with a value of type `{}`",
        expected.display()
    ));
    diagnostic
}

fn argument_count_mismatch_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    expected: usize,
    actual: usize,
    resolved: &ResolveOutput,
) -> Diagnostic {
    let function_name = resolved
        .symbol_for_call(call)
        .map(|symbol| symbol.name.as_str())
        .unwrap_or("<unknown>");
    let mut diagnostic = Diagnostic::error(
        "E0320",
        format!(
            "function `{function_name}` expects {expected} argument(s), but call provides {actual}"
        ),
    );
    diagnostic.primary_span = sources.span_to_json(call.arguments_span).ok().map(Box::new);
    if let Some(symbol) = resolved.symbol_for_call(call)
        && let Ok(span) = sources.span_to_json(symbol.declaration_span)
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("function `{}` is declared here", symbol.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some("pass exactly the parameters declared by the function".to_string());
    diagnostic
}

fn argument_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: usize,
    argument: &Expr,
    parameter: &ParameterSignature,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0321",
        format!(
            "argument {} has type `{}`, but parameter `{}` expects `{}`",
            index + 1,
            actual.display(),
            parameter.name,
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(parameter.ty.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("parameter `{}` is declared here", parameter.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!("pass a value of type `{}`", expected.display()));
    diagnostic
}

fn binding_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0342",
        format!(
            "`{keyword}` binding `{}` is annotated as `{}`, but the initializer has type `{}`",
            statement.name,
            expected.display(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    if let Some(annotation) = &statement.ty
        && let Ok(span) = sources.span_to_json(annotation.span())
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("binding `{}` is annotated here", statement.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "change the initializer or annotate `{}` as `{}`",
        statement.name,
        actual.display()
    ));
    diagnostic
}

fn array_literal_element_type_mismatch_diagnostic(
    sources: &SourceMap,
    element: &Expr,
    element_type: &Type,
    first_element: &Expr,
    first_type: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0343",
        format!(
            "array literal element has type `{}`, but earlier elements have type `{}`",
            element_type.display(),
            first_type.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(element.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_element.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "array element type was inferred as `{}` here",
                first_type.display()
            ),
            span: Some(span),
        });
    }
    diagnostic.help = Some("make every array element have the same type".to_string());
    diagnostic
}

fn index_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: &IndexExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0344",
        format!(
            "index expression target has type `{}`, but indexing requires `[T; N]`, `[T]`, `[+T]`, or `str`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(index.object.span()).ok().map(Box::new);
    diagnostic.help = Some("index an array, view, or string value".to_string());
    diagnostic
}

fn index_value_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: &IndexExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0345",
        format!(
            "index expression uses `{}` as the index, but indexes must be integer values",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(index.index_span).ok().map(Box::new);
    diagnostic.help = Some("use an integer value as the index".to_string());
    diagnostic
}

fn if_condition_type_mismatch_diagnostic(
    sources: &SourceMap,
    condition: &Expr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0346",
        format!(
            "`if` condition has type `{}`, but conditions must be `bool`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(condition.span()).ok().map(Box::new);
    diagnostic.help = Some("use a `bool` expression as the condition".to_string());
    diagnostic
}

fn while_condition_type_mismatch_diagnostic(
    sources: &SourceMap,
    condition: &Expr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0357",
        format!(
            "`while` condition has type `{}`, but conditions must be `bool`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(condition.span()).ok().map(Box::new);
    diagnostic.help = Some("use a `bool` expression as the condition".to_string());
    diagnostic
}

fn loop_control_outside_loop_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    keyword: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0359",
        format!("`{keyword}` can only be used inside a loop"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(format!("move `{keyword}` inside a loop body"));
    diagnostic
}

fn switch_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &SwitchStmt,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0361",
        format!(
            "`switch` target has type `{}`, but `switch` requires an enum value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("switch on a value whose type is an enum".to_string());
    diagnostic
}

fn switch_arm_unknown_enum_diagnostic(sources: &SourceMap, arm: &SwitchArm) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!("`switch` arm refers to unknown enum `{}`", arm.enum_name),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("use a visible enum type in the arm pattern".to_string());
    diagnostic
}

fn switch_arm_non_enum_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`switch` arm refers to `{}`, but that type is `{}`",
            arm.enum_name,
            type_symbol_kind_name(symbol.kind)
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("use an enum type in the arm pattern".to_string());
    diagnostic
}

fn switch_arm_enum_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    expected: &TypeSymbol,
    actual: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0363",
        format!(
            "`switch` arm uses enum `{}`, but the target enum is `{}`",
            actual.canonical_name, expected.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help =
        Some("make every arm use the same enum type as the switch target".to_string());
    diagnostic
}

fn switch_arm_unknown_variant_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0364",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, arm.variant_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(arm.variant_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

fn switch_arm_payload_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    enum_symbol: &TypeSymbol,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0365",
        format!(
            "`{}.{}` has {} payload value(s), but the switch arm provides {} binding(s)",
            enum_symbol.canonical_name, arm.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.span).ok().map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; v0 supports either no payload or one payload binding"
            .to_string(),
    );
    diagnostic
}

fn if_is_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0361",
        format!(
            "`if is` target has type `{}`, but `if is` requires an enum value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use `if value is Enum.variant` with an enum value".to_string());
    diagnostic
}

fn if_is_unknown_enum_diagnostic(sources: &SourceMap, statement: &IfIsStmt) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`if is` pattern refers to unknown enum `{}`",
            statement.enum_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a visible enum type in the pattern".to_string());
    diagnostic
}

fn if_is_non_enum_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`if is` pattern refers to `{}`, but that type is `{}`",
            statement.enum_name,
            type_symbol_kind_name(symbol.kind)
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use an enum type in the pattern".to_string());
    diagnostic
}

fn if_is_enum_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    expected: &TypeSymbol,
    actual: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0363",
        format!(
            "`if is` pattern uses enum `{}`, but the target enum is `{}`",
            actual.canonical_name, expected.canonical_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("make the pattern use the same enum type as the target".to_string());
    diagnostic
}

fn if_is_unknown_variant_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0364",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, statement.variant_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.variant_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

fn if_is_payload_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    enum_symbol: &TypeSymbol,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0365",
        format!(
            "`{}.{}` has {} payload value(s), but the if-is pattern provides {} binding(s)",
            enum_symbol.canonical_name, statement.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.pattern_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; v0 supports either no payload or one payload binding"
            .to_string(),
    );
    diagnostic
}

fn enum_variant_unknown_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0366",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, member.member
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

fn enum_variant_payload_count_mismatch_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0367",
        format!(
            "`{}.{}` expects {} payload value(s), but construction provides {}",
            enum_symbol.canonical_name, variant.name, expected, actual
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(variant.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "variant is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("construct the variant with the payload values declared by the enum".to_string());
    diagnostic
}

fn enum_variant_payloadless_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0367",
        format!(
            "`{}.{}` has no payload and must be constructed without `()`",
            enum_symbol.canonical_name, variant.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(call.arguments_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(variant.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "payloadless variant is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "write `{}.{}` instead",
        enum_symbol.canonical_name, variant.name
    ));
    diagnostic
}

fn enum_variant_payload_type_mismatch_diagnostic(
    sources: &SourceMap,
    argument: &Expr,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
    index: usize,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0368",
        format!(
            "`{}.{}` payload {} has type `{}`, but the variant expects `{}`",
            enum_symbol.canonical_name,
            variant.name,
            index + 1,
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument.span()).ok().map(Box::new);
    if let Some(parameter) = variant.payload.get(index)
        && let Ok(span) = sources.span_to_json(parameter.ty.span())
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("payload `{}` is declared here", parameter.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "pass a payload value of type `{}`",
        expected.display()
    ));
    diagnostic
}

fn type_symbol_kind_name(kind: TypeSymbolKind) -> &'static str {
    match kind {
        TypeSymbolKind::Alias => "type alias",
        TypeSymbolKind::Struct => "struct",
        TypeSymbolKind::Enum => "enum",
    }
}

fn for_range_bound_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &ForRangeStmt,
    start_type: &Type,
    end_type: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0360",
        format!(
            "`for` range bounds have types `{}` and `{}`, but range `for` requires matching integer bounds",
            start_type.display(),
            end_type.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.range_span)
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(statement.start.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("range start has type `{}`", start_type.display()),
            span: Some(span),
        });
    }
    if let Ok(span) = sources.span_to_json(statement.end.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("range end has type `{}`", end_type.display()),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("use integer bounds with the same type, or an integer literal that fits the other bound type".to_string());
    diagnostic
}

fn equality_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0347",
        format!(
            "operator `{}` compares `{}` with `{}`, but equality operands must use the same supported equality type",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "compare `bool`, integer, `str`, or supported payloadless enum values of the same type"
            .to_string(),
    );
    diagnostic
}

fn arithmetic_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0352",
        format!(
            "operator `{}` combines `{}` with `{}`, but integer arithmetic requires matching integer operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use integer operands with the same type".to_string());
    diagnostic
}

fn shift_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0353",
        format!(
            "operator `{}` shifts `{}` by `{}`, but shift operands must be integer values",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("shift an integer value by an integer count".to_string());
    diagnostic
}

fn negative_shift_count_diagnostic(sources: &SourceMap, expression: &BinaryExpr) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0354",
        format!(
            "operator `{}` uses a negative shift count",
            expression.operator.spelling()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.right.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a non-negative shift count".to_string());
    diagnostic
}

fn type_conversion_not_lossless_diagnostic(
    sources: &SourceMap,
    expression: &TypeConversionExpr,
    source: &Type,
    target: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0355",
        format!(
            "`as` conversion from `{}` to `{}` is not a lossless integer conversion",
            source.display(),
            target.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.as_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `as` only when every source value can be represented by the target type".to_string(),
    );
    diagnostic
}

fn ordered_comparison_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0348",
        format!(
            "operator `{}` compares `{}` with `{}`, but ordered comparison requires matching integer operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("compare integer values with the same type".to_string());
    diagnostic
}

fn logical_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0349",
        format!(
            "operator `{}` combines `{}` with `{}`, but logical operators require `bool` operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use `bool` expressions on both sides".to_string());
    diagnostic
}

fn logical_not_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &UnaryExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0350",
        format!(
            "operator `{}` uses `{}`, but logical not requires a `bool` operand",
            expression.operator.spelling(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a `bool` expression after `!`".to_string());
    diagnostic
}

fn numeric_negate_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &UnaryExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0351",
        format!(
            "operator `{}` uses `{}`, but numeric negation requires a signed integer operand",
            expression.operator.spelling(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a signed integer value after `-`".to_string());
    diagnostic
}

fn try_on_non_fallible_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0330",
        format!(
            "fallible handling requires a fallible expression, but this expression has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    diagnostic.help = Some(
        "remove postfix `?` or `catch`, or call a function whose return type is `T!`".to_string(),
    );
    diagnostic
}

fn try_in_non_fallible_context_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    context: &ReturnContext,
    attempted_error: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0331",
        format!(
            "postfix `?` would fail with `{}`, but {} is not fallible",
            attempted_error.display(),
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help =
        Some("add `catch error { ... }` or make the current callable return `T!`".to_string());
    diagnostic
}

fn try_error_type_mismatch_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    context: &ReturnContext,
    current_error: &Type,
    attempted_error: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0332",
        format!(
            "postfix `?` would fail with `{}`, but {} fails with `{}`",
            attempted_error.display(),
            context.subject(),
            current_error.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("handle the failure with `catch`".to_string());
    diagnostic
}

fn optional_if_let_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &IfLetStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0356",
        format!(
            "`if {keyword}` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "use an initializer whose type is `T?`, or use a regular `if` condition instead of `if {keyword}`"
    ));
    diagnostic
}

fn optional_while_let_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &WhileLetStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0358",
        format!(
            "`while {keyword}` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "use an initializer whose type is `T?`, or use a regular `while` condition instead of `while {keyword}`"
    ));
    diagnostic
}

fn optional_let_else_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0340",
        format!(
            "`{keyword} ... else` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "remove `else`, or use an initializer whose type is `T?` for `{keyword} ... else`"
    ));
    diagnostic
}

fn optional_let_else_fallthrough_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    else_block: &Block,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0341",
        format!("`{keyword} ... else` requires an `else` block that cannot fall through"),
    );
    diagnostic.primary_span = sources.span_to_json(else_block.span).ok().map(Box::new);
    diagnostic.help = Some(
        "end the `else` block with `return` or `fail` in parser/check v0; later phases will add `break`, `continue`, and `never` support"
            .to_string(),
    );
    diagnostic
}

fn binding_keyword(kind: BindingKind) -> &'static str {
    match kind {
        BindingKind::Let => "let",
        BindingKind::Var => "var",
    }
}

fn add_declared_return_note(
    sources: &SourceMap,
    diagnostic: &mut Diagnostic,
    context: &ReturnContext,
) {
    if let Ok(span) = sources.span_to_json(context.return_type_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "{} declares return type `{}`",
                context.subject(),
                context.declared_type.display()
            ),
            span: Some(span),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::resolve::resolve;
    use crate::source::SourceMap;

    fn check_text(text: &str) -> Vec<Diagnostic> {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ast = parsed.ast.unwrap();
        let resolved = resolve(&sources, &ast);
        let mut diagnostics = resolved.diagnostics.clone();
        diagnostics.extend(check(&sources, &ast, &resolved));
        diagnostics
    }

    #[test]
    fn accepts_program_i32() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_program_void() {
        let diagnostics = check_text(
            r#"program(): void {
    return
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_main_without_program() {
        let diagnostics = check_text(
            r#"func main(): i32 {
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0301");
    }

    #[test]
    fn diagnoses_invalid_program_return_type() {
        let diagnostics = check_text(
            r#"program(): u64 {
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0303");
    }

    #[test]
    fn diagnoses_duplicate_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

program(): void {
    return
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0302");
    }

    #[test]
    fn diagnoses_string_return_from_i32_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return "hello"
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn diagnoses_bare_return_from_i32_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0310");
    }

    #[test]
    fn diagnoses_value_return_from_void_program() {
        let diagnostics = check_text(
            r#"program(): void {
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0311");
    }

    #[test]
    fn diagnoses_missing_return_from_i32_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value = 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0313");
    }

    #[test]
    fn accepts_str_function_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func title(): str {
    return "hello"
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn displays_fixed_array_return_type() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return "nope"
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("[u8; 4]"));
    }

    #[test]
    fn accepts_contextual_fixed_array_literal_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return [0x7F, 0x45, 0x4C, 0x46]
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_fixed_array_literal_length_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func header(): [u8; 4] {
    return [0x7F, 0x45, 0x4C]
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("[i32; 3]"));
        assert!(diagnostics[0].message.contains("[u8; 4]"));
    }

    #[test]
    fn accepts_contextual_fixed_array_literal_binding() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    return 0
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_array_literal_element_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let items = [1, "two"]
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0343");
        assert!(diagnostics[0].message.contains("str"));
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn diagnoses_binding_annotation_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let byte: u8 = 300
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0342");
        assert!(diagnostics[0].message.contains("u8"));
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn accepts_fixed_array_index_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func first(): u8 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    return header[0]
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_view_index_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func first(bytes: [u8]): u8 {
    return bytes[0]
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_str_index_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func first(): u8 {
    return "hello"[0]
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_bool_function_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func enabled(): bool {
    return true
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_if_else_return_as_terminal_statement() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if true {
        return 0
    } else {
        return 1
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_if_condition_from_bool_binding() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let enabled = true
    if enabled {
        return 0
    }
    return 1
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_if_condition_from_comparison() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let count = 1
    if count > 0 {
        return 0
    }
    return 1
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_equality_comparison_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func same(): bool {
    return true == false
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_contextual_integer_literal_comparison() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func is_zero(byte: u8): bool {
    return byte == 0
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_logical_expression_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func enabled(left: bool, right: bool, count: i32): bool {
    return left && count > 0 || right
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_logical_not_expression_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func disabled(enabled: bool): bool {
    return !enabled
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_numeric_negate_expression_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func negative(value: i32): i32 {
    return -value
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_contextual_negative_integer_literal_binding() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value: i64 = -1
    return 0
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_arithmetic_expression_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func calc(left: i32, right: i32): i32 {
    return left + right * 2 - 4 / 2 % 2
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_contextual_integer_literal_arithmetic() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func add_one(byte: u8): u8 {
    return byte + 1
}

func add_one_reversed(byte: u8): u8 {
    return 1 + byte
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_lossless_integer_type_conversion() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let literal = 10 as u8
    return 0
}

func widen_small(value: u8): u16 {
    return value as u16
}

func widen_large(value: u32): u64 {
    return value as u64
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_shift_expression_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func shift_left(value: u64, count: u8): u64 {
    return value << count
}

func shift_right(value: i32): i32 {
    return value >> 1
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_if_condition_from_logical_expression() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let count = 1
    let ready = true
    if count > 0 && ready {
        return 0
    }
    return 1
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_non_bool_if_condition() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if 1 {
        return 0
    }
    return 1
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0346");
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn diagnoses_if_without_else_as_non_terminal() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if true {
        return 0
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0313");
    }

    #[test]
    fn diagnoses_equality_operand_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let same = 1 == "1"
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0347");
        assert!(diagnostics[0].message.contains("i32"));
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn diagnoses_ordered_comparison_on_non_integer_operands() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let less = true < false
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0348");
        assert!(diagnostics[0].message.contains("bool"));
    }

    #[test]
    fn diagnoses_ordered_comparison_integer_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func less(left: u8, right: u16): bool {
    return left < right
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0348");
        assert!(diagnostics[0].message.contains("u8"));
        assert!(diagnostics[0].message.contains("u16"));
    }

    #[test]
    fn diagnoses_arithmetic_integer_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func calc(left: u8, right: u16): void {
    let invalid = left + right
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0352");
        assert!(diagnostics[0].message.contains("u8"));
        assert!(diagnostics[0].message.contains("u16"));
    }

    #[test]
    fn diagnoses_arithmetic_on_non_integer_operands() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let invalid = true + false
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0352");
        assert!(diagnostics[0].message.contains("bool"));
    }

    #[test]
    fn diagnoses_narrowing_integer_type_conversion() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func narrow(value: u64): void {
    let invalid = value as u8
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0355");
        assert!(diagnostics[0].message.contains("u64"));
        assert!(diagnostics[0].message.contains("u8"));
    }

    #[test]
    fn diagnoses_signed_to_unsigned_type_conversion() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func convert(value: i32): void {
    let invalid = value as u64
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0355");
        assert!(diagnostics[0].message.contains("i32"));
        assert!(diagnostics[0].message.contains("u64"));
    }

    #[test]
    fn diagnoses_non_integer_type_conversion() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let invalid = true as i32
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0355");
        assert!(diagnostics[0].message.contains("bool"));
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn diagnoses_shift_operand_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let invalid = 1 << false
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0353");
        assert!(diagnostics[0].message.contains("i32"));
        assert!(diagnostics[0].message.contains("bool"));
    }

    #[test]
    fn diagnoses_negative_shift_count() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let invalid = 1 << -1
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0354");
    }

    #[test]
    fn diagnoses_logical_operand_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let invalid = true && 1
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0349");
        assert!(diagnostics[0].message.contains("bool"));
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn diagnoses_logical_not_operand_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let invalid = !1
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0350");
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn diagnoses_numeric_negate_operand_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func negative(value: u8): u8 {
    return -value
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0351");
        assert!(diagnostics[0].message.contains("u8"));
    }

    #[test]
    fn diagnoses_negative_integer_literal_unsigned_binding() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value: u8 = -1
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0342");
        assert!(diagnostics[0].message.contains("u8"));
    }

    #[test]
    fn accepts_str_equality_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func same(): bool {
    return "a" == "b"
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_index_on_non_indexable_type() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let number = 1
    let byte = number[0]
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0344");
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn diagnoses_non_integer_index_value() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
    let byte = header["0"]
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0345");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn accepts_optional_none_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func lookup(): i32? {
    return none
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_annotated_optional_binding_from_optional_initializer() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value: i32? = maybe_answer()
    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_optional_let_else_extraction() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value = maybe_answer() else {
        return 1
    }

    return value
}

func maybe_answer(): i32? {
    return 42
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_optional_let_else_non_optional_initializer() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value = 1 else {
        return 1
    }

    return value
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0340");
    }

    #[test]
    fn diagnoses_optional_let_else_fallthrough() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value = maybe_answer() else {
        log_missing()
    }

    return value
}

func maybe_answer(): i32? {
    return 42
}

func log_missing(): void {
    return
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0341");
    }

    #[test]
    fn uses_optional_let_else_unwrapped_return_type() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value = maybe_title() else {
        return 1
    }

    return value
}

func maybe_title(): str? {
    return "hello"
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn accepts_optional_if_let_extraction() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if let value = maybe_answer() {
        return value
    } else {
        return 0
    }
}

func maybe_answer(): i32? {
    return 42
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_optional_if_var_extraction() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if var value = maybe_answer() {
        return value
    }

    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_optional_if_let_non_optional_initializer() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if let value = 1 {
        return value
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0356");
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn uses_optional_if_let_unwrapped_return_type() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if let value = maybe_title() {
        return value
    }

    return 0
}

func maybe_title(): str? {
    return "hello"
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn accepts_else_if_let_terminal_chain() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if false {
        return 0
    } else if let value = maybe_answer() {
        return value
    } else if var fallback = maybe_fallback() {
        return fallback
    } else {
        return 3
    }
}

func maybe_answer(): i32? {
    return none
}

func maybe_fallback(): i32? {
    return 2
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_else_if_let_non_optional_initializer() {
        let diagnostics = check_text(
            r#"program(): i32 {
    if false {
        return 0
    } else if let value = 1 {
        return value
    } else {
        return 2
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0356");
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn accepts_while_bool_condition() {
        let diagnostics = check_text(
            r#"program(): i32 {
    while ready() {
        tick()
    }

    return 0
}

func ready(): bool {
    return false
}

func tick(): void {
    return
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_optional_while_let_extraction() {
        let diagnostics = check_text(
            r#"program(): i32 {
    while let value = maybe_answer() {
        return value
    }

    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_optional_while_var_extraction() {
        let diagnostics = check_text(
            r#"program(): i32 {
    while var value = maybe_answer() {
        return value
    }

    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_non_bool_while_condition() {
        let diagnostics = check_text(
            r#"program(): i32 {
    while 1 {
        return 0
    }

    return 1
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0357");
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn diagnoses_optional_while_let_non_optional_initializer() {
        let diagnostics = check_text(
            r#"program(): i32 {
    while let value = 1 {
        return value
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0358");
        assert!(diagnostics[0].message.contains("i32"));
    }

    #[test]
    fn uses_optional_while_let_unwrapped_return_type() {
        let diagnostics = check_text(
            r#"program(): i32 {
    while let value = maybe_title() {
        return value
    }

    return 0
}

func maybe_title(): str? {
    return "hello"
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn accepts_break_and_continue_inside_loops() {
        let diagnostics = check_text(
            r#"program(): void {
    while ready() {
        break
    }

    while let value = maybe_answer() {
        continue
    }
}

func ready(): bool {
    return true
}

func maybe_answer(): i32? {
    return none
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_break_inside_loop_expression_catch_block() {
        let diagnostics = check_text(
            r#"program(): void {
    while ready() {
        let value = fallible() catch error {
            break
        }
    }
}

func ready(): bool {
    return true
}

func fallible(): i32! {
    return 1
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_break_and_continue_inside_loop_statement() {
        let diagnostics = check_text(
            r#"program(): void {
    loop {
        if ready() {
            break
        }

        continue
    }
}

func ready(): bool {
    return true
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_range_for_integer_bounds() {
        let diagnostics = check_text(
            r#"program(): i32 {
    for i in 0..<4 {
        return i
    }

    return 0
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_range_for_contextual_integer_literal_bound() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func first(limit: u64): u64 {
    for i in 0..<limit {
        return i
    }

    return 0
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_break_and_continue_inside_range_for() {
        let diagnostics = check_text(
            r#"program(): void {
    for i in 0..<4 {
        if ready() {
            break
        }

        continue
    }
}

func ready(): bool {
    return true
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_range_for_non_integer_bound() {
        let diagnostics = check_text(
            r#"program(): i32 {
    for i in "a"..<4 {
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0360");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn diagnoses_range_for_bound_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let start: u16 = 0
    let end: u8 = 4

    for i in start..<end {
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0360");
        assert!(diagnostics[0].message.contains("u16"));
        assert!(diagnostics[0].message.contains("u8"));
    }

    #[test]
    fn diagnoses_range_for_as_non_terminal_statement() {
        let diagnostics = check_text(
            r#"program(): i32 {
    for i in 0..<1 {
        return i
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0313");
    }

    #[test]
    fn accepts_loop_with_return_as_terminal_statement() {
        let diagnostics = check_text(
            r#"program(): i32 {
    loop {
        return 0
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_non_terminal_loop_with_break() {
        let diagnostics = check_text(
            r#"program(): i32 {
    loop {
        break
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0313");
    }

    #[test]
    fn diagnoses_break_outside_loop() {
        let diagnostics = check_text(
            r#"program(): void {
    break
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0359");
        assert!(diagnostics[0].message.contains("break"));
    }

    #[test]
    fn diagnoses_continue_outside_loop() {
        let diagnostics = check_text(
            r#"program(): void {
    continue
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0359");
        assert!(diagnostics[0].message.contains("continue"));
    }

    #[test]
    fn checks_success_type_of_fallible_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func run(): void! {
    return
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_fail_in_fallible_function() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func run(error: error): i32! {
    fail error
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_non_error_fail_value() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func run(): i32! {
    fail 1
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0334");
        assert!(diagnostics[0].message.contains("i32"));
        assert!(diagnostics[0].message.contains("error"));
    }

    #[test]
    fn diagnoses_fail_in_non_fallible_function() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func run(error: u64): i32 {
    fail error
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0333");
    }

    #[test]
    fn diagnoses_fail_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func run(error: str): i32! {
    fail error
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0334");
        assert!(diagnostics[0].message.contains("str"));
        assert!(diagnostics[0].message.contains("error"));
    }

    #[test]
    fn accepts_fail_as_terminal_branch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func run(error: error): i32! {
    if true {
        fail error
    } else {
        return 0
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_switch_over_enum() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func describe(error: AppError): str {
    switch error {
        is AppError.missing_path {
            return "missing"
        }

        is AppError.open_failed(path) {
            return path
        }
    }

    return "unknown"
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_switch_else_as_terminal_statement() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func describe(error: AppError): str {
    switch error {
        is AppError.missing_path {
            return "missing"
        }

        is AppError.open_failed(path) {
            return path
        }

        else {
            return "unknown"
        }
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_if_is_over_enum() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func describe(error: AppError): str {
    if error is AppError.open_failed(path) {
        return path
    } else if error is AppError.missing_path {
        return "missing"
    } else {
        return "unknown"
    }
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_if_is_non_enum_target() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    if 1 is AppError.missing_path {
        return 1
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0361");
    }

    #[test]
    fn diagnoses_if_is_enum_mismatch() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

enum OtherError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    if error is OtherError.missing_path {
        return 1
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0363");
    }

    #[test]
    fn diagnoses_if_is_unknown_variant() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    if error is AppError.open_failed {
        return 1
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0364");
    }

    #[test]
    fn diagnoses_if_is_payload_mismatch() {
        let diagnostics = check_text(
            r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    if error is AppError.open_failed {
        return 1
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0365");
    }

    #[test]
    fn diagnoses_switch_else_with_non_terminal_arm() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func describe(error: AppError): str {
    switch error {
        is AppError.missing_path {
            let message = "missing"
        }

        else {
            return "unknown"
        }
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0313");
    }

    #[test]
    fn accepts_payloadless_enum_variant_construction() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.missing_path
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_payload_enum_variant_construction() {
        let diagnostics = check_text(
            r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func make(path: str): AppError {
    return AppError.open_failed(path)
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_enum_variant_construction_in_fail() {
        let diagnostics = check_text(
            r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func run(path: str): void! {
    fail AppError.open_failed(path)
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0334");
    }

    #[test]
    fn diagnoses_unknown_enum_variant_construction() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.open_failed
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0366");
    }

    #[test]
    fn diagnoses_enum_variant_payload_count_mismatch() {
        let diagnostics = check_text(
            r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.open_failed
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0367");
    }

    #[test]
    fn diagnoses_payloadless_enum_variant_call() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.missing_path()
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0367");
    }

    #[test]
    fn diagnoses_enum_variant_payload_type_mismatch() {
        let diagnostics = check_text(
            r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func make(): AppError {
    return AppError.open_failed(1)
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0368");
        assert!(diagnostics[0].message.contains("i32"));
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn diagnoses_switch_non_enum_target() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    switch 1 {
        is AppError.missing_path {
            return 1
        }
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0361");
    }

    #[test]
    fn diagnoses_switch_arm_enum_mismatch() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

enum OtherError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is OtherError.missing_path {
            return 1
        }
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0363");
    }

    #[test]
    fn diagnoses_switch_unknown_variant() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.open_failed {
            return 1
        }
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0364");
    }

    #[test]
    fn diagnoses_switch_payload_mismatch() {
        let diagnostics = check_text(
            r#"enum AppError {
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.open_failed {
            return 1
        }
    }

    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0365");
    }

    #[test]
    fn diagnoses_switch_as_non_terminal_statement() {
        let diagnostics = check_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.missing_path {
            return 1
        }
    }
}
"#,
        );

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0313");
    }

    #[test]
    fn uses_same_file_function_call_return_type() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return title()
}

func title(): str {
    return "hello"
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn diagnoses_same_file_function_argument_count_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return answer()
}

func answer(value: i32): i32 {
    return 1
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0320");
    }

    #[test]
    fn diagnoses_same_file_function_argument_type_mismatch() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return length("hello")
}

func length(value: i32): i32 {
    return 1
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0321");
    }

    #[test]
    fn unwraps_catch_expression_success_type() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return answer() catch error {
        return 1
    }
}

func answer(): i32! {
    return 1
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_propagation_in_non_fallible_function() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return answer()?
}

func answer(): i32! {
    return 1
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0331");
    }

    #[test]
    fn diagnoses_catch_on_non_fallible_expression() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return answer() catch error {
        return 1
    }
}

func answer(): i32 {
    return 1
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0330");
    }
}

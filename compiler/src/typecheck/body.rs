use super::arrays::{check_array_literal_elements, check_index_expression};
use super::bindings::{check_binding_annotation, continuing_binding_type};
use super::calls::{
    check_known_function_call, check_method_receiver_call, method_member_for_call,
    resolved_call_signature, resolved_method_for_call,
};
use super::controls::{
    check_for_range_bounds, check_if_condition, check_if_let_initializer, check_while_condition,
    check_while_let_initializer,
};
use super::diagnostics::loop_control_outside_loop_diagnostic;
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_if_is_binding,
    environment_for_if_let_binding, environment_for_method, environment_for_parameters,
    environment_for_parameters_with_self_type, environment_for_switch_arm,
    environment_for_while_let_binding, impl_self_type,
};
use super::expressions::{check_error_member_expression, expression_type};
use super::fallible::check_force_unwrap_operand;
use super::model::{TypeEnvironment, binding_kind_is_mutable};
use super::operations::{
    check_binary_expression, check_type_conversion_expression, check_unary_expression,
};
use super::structs::{check_struct_literal_expression, check_struct_member_expression};
use super::variants::{
    check_enum_variant_call, check_enum_variant_member, check_if_is_statement,
    check_switch_statement, is_enum_variant_call,
};
use crate::ast::{AstFile, Block, Expr, ImplDecl, ImplMember, Item, Stmt};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_body_expressions(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Program(program) => {
                let mut environment = TypeEnvironment::default();
                check_block_expressions(
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
                check_block_expressions(
                    sources,
                    &function.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    0,
                );
            }
            Item::Impl(impl_) => {
                check_impl_member_expressions(sources, impl_, resolved, diagnostics);
            }
            Item::Use(_)
            | Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Trait(_) => {}
        }
    }
}

fn check_impl_member_expressions(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let self_type = impl_self_type(impl_, resolved);

    for member in &impl_.members {
        match member {
            ImplMember::Function(function) => {
                let mut environment = environment_for_parameters_with_self_type(
                    &function.parameters.parameters,
                    resolved,
                    self_type.clone(),
                );
                check_block_expressions(
                    sources,
                    &function.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    0,
                );
            }
            ImplMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut environment = environment_for_method(method, resolved, self_type.clone());
                check_block_expressions(sources, body, resolved, diagnostics, &mut environment, 0);
            }
        }
    }
}

fn check_block_expressions(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    loop_depth: usize,
) {
    for statement in &block.statements {
        check_statement_expressions(
            sources,
            statement,
            resolved,
            diagnostics,
            environment,
            loop_depth,
        );
    }
}

fn check_statement_expressions(
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
                check_expression_tree(
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
            check_expression_tree(
                sources,
                &statement.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
        Stmt::Binding(statement) => {
            check_expression_tree(
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
                check_block_expressions(
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
            let binding_type =
                continuing_binding_type(statement, initializer_type, resolved, environment);
            environment.define_binding(
                statement.name.clone(),
                binding_type,
                binding_kind_is_mutable(statement.kind),
            );
        }
        Stmt::If(statement) => {
            check_expression_tree(
                sources,
                &statement.condition,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_if_condition(sources, statement, resolved, diagnostics, environment);

            let mut then_environment = environment.clone();
            check_block_expressions(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                loop_depth,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_expressions(
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
            check_expression_tree(
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
            check_block_expressions(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                loop_depth,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_expressions(
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
            check_expression_tree(
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
            check_block_expressions(
                sources,
                &statement.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                loop_depth,
            );
            if let Some(else_block) = &statement.else_block {
                let mut else_environment = environment.clone();
                check_block_expressions(
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
            check_expression_tree(
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
                check_block_expressions(
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
                check_block_expressions(
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
            check_expression_tree(
                sources,
                &statement.condition,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_while_condition(sources, statement, resolved, diagnostics, environment);

            let mut body_environment = environment.clone();
            check_block_expressions(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                loop_depth + 1,
            );
        }
        Stmt::WhileLet(statement) => {
            check_expression_tree(
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
            check_block_expressions(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                loop_depth + 1,
            );
        }
        Stmt::ForRange(statement) => {
            check_expression_tree(
                sources,
                &statement.start,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_tree(
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
            check_block_expressions(
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
            check_block_expressions(
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
            check_expression_tree(
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

fn check_expression_tree(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &mut TypeEnvironment,
    loop_depth: usize,
) {
    match expression {
        Expr::Propagate(expression) => {
            check_expression_tree(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
        Expr::Force(expression) => {
            check_expression_tree(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_force_unwrap_operand(
                sources,
                expression.span,
                &expression.expression,
                resolved,
                environment,
                diagnostics,
            );
        }
        Expr::Catch(expression) => {
            check_expression_tree(
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
            check_block_expressions(
                sources,
                &expression.catch_block,
                resolved,
                diagnostics,
                &mut catch_environment,
                loop_depth,
            );
        }
        Expr::Binary(expression) => {
            check_expression_tree(
                sources,
                &expression.left,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_tree(
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
            check_expression_tree(
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
            check_expression_tree(
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
            if let Some(method) = method_member_for_call(expression)
                && resolved_method_for_call(resolved, expression, environment).is_some()
            {
                check_expression_tree(
                    sources,
                    &method.object,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            } else if !is_enum_variant_call(expression, resolved) {
                check_expression_tree(
                    sources,
                    &expression.callee,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
            for argument in &expression.arguments {
                check_expression_tree(
                    sources,
                    argument,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
            check_enum_variant_call(sources, expression, resolved, diagnostics, environment);

            if let Some(signature) = resolved_call_signature(resolved, expression, environment) {
                check_known_function_call(
                    sources,
                    expression,
                    &signature,
                    resolved,
                    diagnostics,
                    environment,
                );
            }
            check_method_receiver_call(sources, expression, resolved, diagnostics, environment);
        }
        Expr::Member(expression) => {
            check_expression_tree(
                sources,
                &expression.object,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_enum_variant_member(sources, expression, resolved, diagnostics);
            check_error_member_expression(sources, expression, resolved, diagnostics, environment);
            check_struct_member_expression(sources, expression, resolved, diagnostics, environment);
        }
        Expr::Index(expression) => {
            check_expression_tree(
                sources,
                &expression.object,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_tree(
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
                check_expression_tree(
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
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                check_expression_tree(
                    sources,
                    &field.value,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
            check_struct_literal_expression(
                sources,
                expression,
                resolved,
                diagnostics,
                environment,
            );
        }
        Expr::Group(expression) => {
            check_expression_tree(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
        }
        Expr::OptionalDefault(expression) => {
            check_expression_tree(
                sources,
                &expression.value,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_tree(
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

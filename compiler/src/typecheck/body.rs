use super::arrays::{check_array_literal_elements, check_index_expression};
use super::bindings::{check_binding_annotation, continuing_binding_type};
use super::calls::{
    check_known_function_call, check_method_receiver_call, check_unresolved_member_call,
    method_member_for_call, resolved_call_signature,
};
use super::controls::{
    check_for_range_bounds, check_if_condition, check_if_let_initializer, check_while_condition,
    check_while_let_initializer,
};
use super::diagnostics::{
    assignment_type_mismatch_diagnostic, immutable_assignment_diagnostic,
    loop_control_outside_loop_diagnostic, non_copy_struct_assignment_diagnostic,
    readwrite_borrow_requires_writable_place_diagnostic,
};
use super::environments::{
    environment_for_catch, environment_for_for_range_binding, environment_for_function,
    environment_for_if_is_binding, environment_for_if_let_binding, environment_for_method,
    environment_for_parameters_with_self_type, environment_for_pattern_conditional_arm,
    environment_for_switch_arm, environment_for_while_let_binding, impl_self_type,
};
use super::expressions::{
    check_error_member_expression, collection_len_call_type, expression_type,
};
use super::fallible::check_force_unwrap_operand;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::operations::{
    check_binary_expression, check_type_conversion_expression, check_unary_expression,
    is_expression_assignable,
};
use super::strings::check_interpolated_string_expression;
use super::structs::{check_struct_literal_expression, check_struct_member_expression};
use super::variants::{
    check_enum_variant_call, check_enum_variant_member, check_if_is_statement,
    check_pattern_conditional_expression, check_switch_statement, is_enum_variant_call,
};
use crate::ast::{
    AssignmentStmt, AstFile, Block, Expr, ImplDecl, ImplMember, InterpolatedStringPart, Item, Stmt,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::source::SourceMap;

pub(super) fn check_body_expressions(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let mut environment = environment_for_function(function, resolved);
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
            ImplMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut environment = environment_for_method(method, resolved, self_type.clone());
                check_block_expressions(sources, body, resolved, diagnostics, &mut environment, 0);
            }
            ImplMember::Drop(drop_) => {
                let mut environment = environment_for_parameters_with_self_type(
                    std::slice::from_ref(&drop_.binding),
                    resolved,
                    self_type.clone(),
                );
                check_block_expressions(
                    sources,
                    &drop_.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    0,
                );
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
        Stmt::Assignment(statement) => {
            check_expression_tree(
                sources,
                &statement.target,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_expression_tree(
                sources,
                &statement.value,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_assignment_statement(sources, statement, resolved, diagnostics, environment);
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
        Stmt::Drop(_) => {}
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

fn check_assignment_statement(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    if let Some(name) = assignment_target_root_name(&statement.target)
        && environment.get(name).is_some()
        && !environment.is_mutable_binding(name)
        && !assignment_targets_readwrite_borrow_field(&statement.target, resolved, environment)
    {
        diagnostics.push(immutable_assignment_diagnostic(sources, statement, name));
    }

    let target_type = expression_type(&statement.target, resolved, environment);
    if target_type.is_unknown_or_unresolved() || target_type.first_unsized_part().is_some() {
        return;
    }

    let value_type = expression_type(&statement.value, resolved, environment);
    if value_type.is_unknown_or_unresolved() {
        return;
    }

    if !is_expression_assignable(&target_type, &statement.value, resolved, environment) {
        diagnostics.push(assignment_type_mismatch_diagnostic(
            sources,
            statement,
            &target_type,
            &value_type,
        ));
        return;
    }

    if let Some((source_name, type_name)) =
        non_copy_struct_identifier_assignment(&statement.value, resolved, environment)
    {
        diagnostics.push(non_copy_struct_assignment_diagnostic(
            sources,
            statement,
            source_name,
            type_name,
        ));
    }
}

fn assignment_target_root_name(expression: &Expr) -> Option<&str> {
    match expression {
        Expr::Identifier(identifier) => Some(identifier.name.as_str()),
        Expr::Member(member) => assignment_target_root_name(&member.object),
        Expr::Group(group) => assignment_target_root_name(&group.expression),
        _ => None,
    }
}

fn assignment_targets_readwrite_borrow_field(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    match expression {
        Expr::Member(member) => {
            let object_type = expression_type(&member.object, resolved, environment);
            matches!(object_type, Type::Named(name) if name.starts_with("&+"))
        }
        Expr::Group(group) => {
            assignment_targets_readwrite_borrow_field(&group.expression, resolved, environment)
        }
        _ => false,
    }
}

fn non_copy_struct_identifier_assignment<'a>(
    expression: &'a Expr,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<(&'a str, &'a str)> {
    match expression {
        Expr::Identifier(identifier) => {
            let value_type = expression_type(expression, resolved, environment);
            non_copy_struct_type_name(&value_type, resolved)
                .map(|type_name| (identifier.name.as_str(), type_name))
        }
        Expr::Group(group) => {
            non_copy_struct_identifier_assignment(&group.expression, resolved, environment)
        }
        _ => None,
    }
}

fn non_copy_struct_type_name<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a str> {
    let Type::Named(canonical_name) = ty else {
        return None;
    };
    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Struct && !symbol.is_copy)
        .map(|symbol| symbol.canonical_name.as_str())
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
                expression.operator_span,
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
        Expr::Borrow(expression) => {
            check_expression_tree(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            if expression.is_readwrite
                && !borrow_operand_is_writable_place(&expression.expression, environment)
            {
                diagnostics.push(readwrite_borrow_requires_writable_place_diagnostic(
                    sources, expression,
                ));
            }
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
            if let Some(method) = method_member_for_call(expression) {
                check_expression_tree(
                    sources,
                    &method.object,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            } else {
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
            if collection_len_call_type(expression, resolved, environment).is_none()
                && !is_enum_variant_call(expression, resolved)
            {
                check_unresolved_member_call(
                    sources,
                    expression,
                    resolved,
                    diagnostics,
                    environment,
                );
            }
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
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    check_expression_tree(
                        sources,
                        &part.expression,
                        resolved,
                        diagnostics,
                        environment,
                        loop_depth,
                    );
                }
            }
            check_interpolated_string_expression(
                sources,
                expression,
                resolved,
                diagnostics,
                environment,
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
        Expr::PatternConditional(expression) => {
            check_expression_tree(
                sources,
                &expression.target,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            for arm in &expression.arms {
                let mut arm_environment =
                    environment_for_pattern_conditional_arm(arm, resolved, environment);
                check_expression_tree(
                    sources,
                    &arm.expression,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    loop_depth,
                );
            }
            check_expression_tree(
                sources,
                &expression.fallback,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_pattern_conditional_expression(
                sources,
                expression,
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

fn borrow_operand_is_writable_place(expression: &Expr, environment: &TypeEnvironment) -> bool {
    match expression {
        Expr::Identifier(identifier) => environment.is_mutable_binding(&identifier.name),
        Expr::Group(group) => borrow_operand_is_writable_place(&group.expression, environment),
        _ => false,
    }
}

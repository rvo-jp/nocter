use super::allocation::type_is_aborting_allocator_capability;
use super::arrays::{check_array_literal_elements, check_index_expression};
use super::bindings::{
    check_binding_annotation, check_binding_initializer_copyability, continuing_binding_type,
};
use super::calls::{
    check_known_function_call, check_method_receiver_call, check_unresolved_member_call,
    method_member_for_call, resolved_call_signature,
};
use super::controls::{check_for_range_bounds, check_if_condition, check_while_condition};
use super::copyability::{implicit_non_copy_owned_value_source, non_copy_owned_type_kind};
use super::diagnostics::{
    assignment_type_mismatch_diagnostic, compound_assignment_operand_type_mismatch_diagnostic,
    immutable_assignment_diagnostic, loop_control_outside_loop_diagnostic,
    non_copy_struct_assignment_diagnostic, non_writable_assignment_target_diagnostic,
    readwrite_borrow_requires_writable_place_diagnostic, region_allocator_capability_diagnostic,
    self_move_assignment_diagnostic,
};
use super::environments::{
    environment_for_catch, environment_for_collection_for_binding,
    environment_for_for_range_binding, environment_for_function, environment_for_if_is_binding,
    environment_for_interface_method, environment_for_literal_pack_binding, environment_for_method,
    environment_for_parameters_in_method_owner, environment_for_switch_arm,
};
use super::expressions::{check_error_member_expression, expression_type};
use super::fallible::check_force_unwrap_operand;
use super::literals::{
    check_literal_pack_for_statement, check_typed_sequence_literal, check_typed_string_literal,
    check_unconstrained_literal_initializer, literal_expression_type_with_expected,
};
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::operations::{
    check_binary_expression, check_otherwise_expression, check_type_conversion_expression,
    check_unary_expression, compound_assignment_operands_match, is_expression_assignable,
};
use super::places::expression_is_writable_place;
use super::strings::check_interpolated_string_expression;
use super::structs::{check_struct_literal_expression, check_struct_member_expression};
use super::variants::{
    check_enum_variant_call, check_enum_variant_member, check_if_is_statement,
    check_match_expression, check_switch_statement, is_enum_variant_call,
};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, AstFile, Block, ConformanceDecl, ConformanceMember, Expr,
    InstanceDecl, InstanceMember, InterpolatedStringPart, Item, Stmt, UnaryOperator,
};
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
            Item::Function(function) => {
                let Some(body) = &function.body else {
                    continue;
                };
                let mut environment = environment_for_function(function, resolved);
                check_block_expressions(sources, body, resolved, diagnostics, &mut environment, 0);
            }
            Item::Test(test) => {
                let mut environment = TypeEnvironment::default();
                check_block_expressions(
                    sources,
                    &test.body,
                    resolved,
                    diagnostics,
                    &mut environment,
                    0,
                );
            }
            Item::Instance(instance) => {
                check_instance_member_expressions(sources, instance, resolved, diagnostics)
            }
            Item::Conformance(conformance) => {
                check_conformance_member_expressions(sources, conformance, resolved, diagnostics)
            }
            Item::Interface(interface) => {
                for method in &interface.methods {
                    let Some(body) = &method.body else {
                        continue;
                    };
                    let mut environment =
                        environment_for_interface_method(method, resolved, interface);
                    check_block_expressions(
                        sources,
                        body,
                        resolved,
                        diagnostics,
                        &mut environment,
                        0,
                    );
                }
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    let Some(body) = &function.body else {
                        continue;
                    };
                    let mut environment = environment_for_function(function, resolved);
                    check_block_expressions(
                        sources,
                        body,
                        resolved,
                        diagnostics,
                        &mut environment,
                        0,
                    );
                }
                for (_, literal) in construct.literals() {
                    check_literal_body_expressions(sources, literal, resolved, diagnostics);
                }
            }
            Item::Coerce(coerce) => check_instance_member_expressions(
                sources,
                &coerce.callable_instance(),
                resolved,
                diagnostics,
            ),
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }
}

fn check_literal_body_expressions(
    sources: &SourceMap,
    literal: &crate::ast::LiteralDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(body) = &literal.body else {
        return;
    };
    let mut environment = super::environments::environment_for_literal(literal, resolved);
    check_block_expressions(sources, body, resolved, diagnostics, &mut environment, 0);
}

fn check_instance_member_expressions(
    sources: &SourceMap,
    instance: &InstanceDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for member in &instance.members {
        match member {
            InstanceMember::Method(method) => {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut environment = environment_for_method(method, resolved, instance);
                check_block_expressions(sources, body, resolved, diagnostics, &mut environment, 0);
            }
            InstanceMember::Drop(drop_) => {
                let mut environment = environment_for_parameters_in_method_owner(
                    std::slice::from_ref(&drop_.binding),
                    resolved,
                    instance,
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

fn check_conformance_member_expressions(
    sources: &SourceMap,
    conformance: &ConformanceDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for member in &conformance.members {
        let ConformanceMember::Method(method) = member else {
            continue;
        };
        let Some(body) = &method.body else { continue };
        let mut environment = environment_for_method(method, resolved, conformance);
        check_block_expressions(sources, body, resolved, diagnostics, &mut environment, 0);
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
    if let Some(result) = &block.result {
        check_expression_tree(
            sources,
            result,
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
        Stmt::Import(_) | Stmt::FromImport(_) => {}
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
            let expected = statement.ty.as_ref().map(|ty| {
                super::type_expr::type_expr_to_type_in_environment(ty, resolved, environment)
            });
            check_unconstrained_literal_initializer(
                sources,
                &statement.initializer,
                expected.is_some(),
                resolved,
                diagnostics,
            );
            let initializer_type = literal_expression_type_with_expected(
                &statement.initializer,
                expected.as_ref(),
                resolved,
                environment,
            );
            check_binding_annotation(
                sources,
                statement,
                &initializer_type,
                resolved,
                diagnostics,
                environment,
            );
            check_binding_initializer_copyability(
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
                let mut arm_environment =
                    environment_for_switch_arm(arm, &statement.expression, resolved, environment);
                check_block_expressions(
                    sources,
                    &arm.body,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    loop_depth,
                );
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                let mut else_environment = environment.clone();
                check_block_expressions(
                    sources,
                    &wildcard_arm.body,
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
        Stmt::CollectionFor(statement) => {
            check_expression_tree(
                sources,
                &statement.source,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            let item_type = match super::iteration::resolve_collection_iteration(
                statement,
                resolved,
                environment,
            ) {
                Ok(plan) => plan.item_type,
                Err(error) => {
                    diagnostics.push(super::iteration::collection_iteration_diagnostic(
                        sources, statement, error,
                    ));
                    Type::Unknown
                }
            };
            let mut body_environment =
                environment_for_collection_for_binding(statement, item_type, environment);
            check_block_expressions(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                loop_depth + 1,
            );
        }
        Stmt::LiteralPackFor(statement) => {
            check_literal_pack_for_statement(sources, statement, environment, diagnostics);
            let mut body_environment = environment_for_literal_pack_binding(statement, environment);
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
        Stmt::Region(statement) => {
            check_expression_tree(
                sources,
                &statement.allocator,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            let allocator_type = expression_type(&statement.allocator, resolved, environment);
            if !allocator_type.is_unknown_or_unresolved()
                && !type_is_aborting_allocator_capability(&allocator_type, resolved)
            {
                diagnostics.push(region_allocator_capability_diagnostic(
                    sources,
                    statement.allocator.span(),
                    &allocator_type,
                ));
            }
            let mut body_environment = environment.clone();
            body_environment.define(statement.name.clone(), allocator_type);
            check_block_expressions(
                sources,
                &statement.body,
                resolved,
                diagnostics,
                &mut body_environment,
                loop_depth,
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
    if !expression_is_writable_place(&statement.target, resolved, environment) {
        if let Some(name) = assignment_target_root_name(&statement.target)
            && environment.get(name).is_some()
        {
            diagnostics.push(immutable_assignment_diagnostic(sources, statement, name));
        } else {
            diagnostics.push(non_writable_assignment_target_diagnostic(
                sources, statement,
            ));
        }
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

    if statement.operator != AssignmentOperator::Assign
        && !compound_assignment_operands_match(
            statement.operator,
            &target_type,
            &statement.target,
            &value_type,
            &statement.value,
            resolved,
            environment,
        )
    {
        diagnostics.push(compound_assignment_operand_type_mismatch_diagnostic(
            sources,
            statement,
            &target_type,
            &value_type,
        ));
        return;
    }

    if non_copy_owned_type_kind(&target_type, resolved).is_some()
        && let Some(target_name) = assignment_target_root_name(&statement.target)
        && let Some(source_name) = assignment_move_source_name(statement)
        && target_name == source_name
    {
        diagnostics.push(self_move_assignment_diagnostic(
            sources,
            statement,
            target_name,
        ));
        return;
    }

    if let Some(source) =
        implicit_non_copy_owned_value_source(&statement.value, resolved, environment)
    {
        diagnostics.push(non_copy_struct_assignment_diagnostic(
            sources,
            statement,
            &source.source_name,
            &source.type_name,
            source.kind,
        ));
    }
}

fn assignment_move_source_name(statement: &AssignmentStmt) -> Option<&str> {
    if statement.operator != AssignmentOperator::Assign {
        return None;
    }
    let Expr::Unary(unary) = unwrap_group(&statement.value) else {
        return None;
    };
    if unary.operator != UnaryOperator::Move {
        return None;
    }
    let Expr::Identifier(identifier) = unary.operand.as_ref() else {
        return None;
    };
    Some(identifier.name.as_str())
}

fn assignment_target_root_name(expression: &Expr) -> Option<&str> {
    match expression {
        Expr::Identifier(identifier) => Some(identifier.name.as_str()),
        Expr::Member(member) => assignment_target_root_name(&member.object),
        Expr::Index(index) => assignment_target_root_name(&index.object),
        Expr::Group(group) => assignment_target_root_name(&group.expression),
        _ => None,
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
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
        Expr::Closure(closure) => {
            let mut closure_environment =
                super::closures::environment_for_closure(closure, resolved, environment);
            check_block_expressions(
                sources,
                &closure.body,
                resolved,
                diagnostics,
                &mut closure_environment,
                0,
            );
        }
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
                && !expression_is_writable_place(&expression.expression, resolved, environment)
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

            if let Some(contract) =
                super::callables::callable_contract_for_call(expression, resolved, environment)
            {
                super::callables::check_callable_call(
                    sources,
                    expression,
                    &contract,
                    resolved,
                    diagnostics,
                    environment,
                );
            } else if let Some(signature) =
                resolved_call_signature(resolved, expression, environment)
            {
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
            if !is_enum_variant_call(expression, resolved)
                && super::callables::callable_contract_for_call(expression, resolved, environment)
                    .is_none()
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
        Expr::TypedSequenceLiteral(expression) => {
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
            if let Some(using) = &expression.using {
                check_expression_tree(
                    sources,
                    &using.allocator,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
            check_typed_sequence_literal(sources, expression, resolved, environment, diagnostics);
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                check_expression_tree(
                    sources,
                    &using.allocator,
                    resolved,
                    diagnostics,
                    environment,
                    loop_depth,
                );
            }
            check_typed_string_literal(sources, expression, resolved, environment, diagnostics);
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
        Expr::Otherwise(expression) => {
            check_expression_tree(
                sources,
                &expression.value,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            let mut fallback_environment = environment.clone();
            check_block_expressions(
                sources,
                &expression.fallback,
                resolved,
                diagnostics,
                &mut fallback_environment,
                loop_depth,
            );
            check_otherwise_expression(sources, expression, resolved, diagnostics, environment);
        }
        Expr::If(expression) => {
            check_expression_tree(
                sources,
                &expression.condition,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_if_condition(sources, expression, resolved, diagnostics, environment);

            let mut then_environment = environment.clone();
            check_block_expressions(
                sources,
                &expression.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                loop_depth,
            );
            if let Some(else_block) = &expression.else_block {
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
        Expr::IfIs(expression) => {
            check_expression_tree(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_if_is_statement(sources, expression, resolved, diagnostics, environment);

            let mut then_environment =
                environment_for_if_is_binding(expression, resolved, environment);
            check_block_expressions(
                sources,
                &expression.then_block,
                resolved,
                diagnostics,
                &mut then_environment,
                loop_depth,
            );
            if let Some(else_block) = &expression.else_block {
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
        Expr::Match(expression) => {
            check_expression_tree(
                sources,
                &expression.expression,
                resolved,
                diagnostics,
                environment,
                loop_depth,
            );
            check_match_expression(sources, expression, resolved, diagnostics, environment);

            for arm in &expression.arms {
                let mut arm_environment =
                    environment_for_switch_arm(arm, &expression.expression, resolved, environment);
                check_block_expressions(
                    sources,
                    &arm.body,
                    resolved,
                    diagnostics,
                    &mut arm_environment,
                    loop_depth,
                );
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                let mut else_environment = environment.clone();
                check_block_expressions(
                    sources,
                    &wildcard_arm.body,
                    resolved,
                    diagnostics,
                    &mut else_environment,
                    loop_depth,
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

use super::super::aggregates::{aggregate_type_layout, supported_aggregate_copy_layout};
use super::super::context::{AggregateFieldKind, LoweringContext};
use super::super::literals::{lower_i32_literal, lower_u8_literal, lower_usize_literal};
use super::aggregate_call_field;
use crate::ast::{
    BinaryExpr, BinaryOperator, Expr, InterpolatedStringPart, TypeConversionExpr, TypeExpr,
    UnaryOperator,
};
use crate::ir::Type;

pub(super) fn short_circuit_bool_expression_contains_call(binary: &BinaryExpr) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
    ) && (expression_contains_call(&binary.left) || expression_contains_call(&binary.right))
}

pub(in crate::ir::lower) fn expression_contains_call(expression: &Expr) -> bool {
    match expression {
        Expr::Call(_) => true,
        Expr::Unary(unary) => expression_contains_call(&unary.operand),
        Expr::Binary(binary) => {
            expression_contains_call(&binary.left) || expression_contains_call(&binary.right)
        }
        Expr::Group(group) => expression_contains_call(&group.expression),
        Expr::TypeConversion(conversion) => expression_contains_call(&conversion.expression),
        Expr::Propagate(propagation) => expression_contains_call(&propagation.expression),
        Expr::Force(force) => expression_contains_call(&force.expression),
        Expr::Catch(catch) => expression_contains_call(&catch.expression),
        Expr::Borrow(borrow) => expression_contains_call(&borrow.expression),
        Expr::Member(member) => expression_contains_call(&member.object),
        Expr::Index(index) => {
            expression_contains_call(&index.object) || expression_contains_call(&index.index)
        }
        Expr::ArrayLiteral(array) => array.elements.iter().any(expression_contains_call),
        Expr::StructLiteral(struct_literal) => struct_literal
            .fields
            .iter()
            .any(|field| expression_contains_call(&field.value)),
        Expr::InterpolatedString(interpolated) => interpolated.parts.iter().any(|part| {
            matches!(
                part,
                InterpolatedStringPart::Expression(part)
                    if expression_contains_call(&part.expression)
            )
        }),
        Expr::OptionalDefault(optional_default) => {
            expression_contains_call(&optional_default.value)
                || expression_contains_call(&optional_default.default)
        }
        Expr::PatternConditional(pattern_conditional) => {
            expression_contains_call(&pattern_conditional.target)
                || pattern_conditional
                    .arms
                    .iter()
                    .any(|arm| expression_contains_call(&arm.expression))
                || expression_contains_call(&pattern_conditional.fallback)
        }
        _ => false,
    }
}

pub(in crate::ir::lower) fn expression_contains_interpolated_string(expression: &Expr) -> bool {
    match expression {
        Expr::InterpolatedString(_) => true,
        Expr::Unary(unary) => expression_contains_interpolated_string(&unary.operand),
        Expr::Binary(binary) => {
            expression_contains_interpolated_string(&binary.left)
                || expression_contains_interpolated_string(&binary.right)
        }
        Expr::Group(group) => expression_contains_interpolated_string(&group.expression),
        Expr::TypeConversion(conversion) => {
            expression_contains_interpolated_string(&conversion.expression)
        }
        Expr::Propagate(propagation) => {
            expression_contains_interpolated_string(&propagation.expression)
        }
        Expr::Force(force) => expression_contains_interpolated_string(&force.expression),
        Expr::Catch(catch) => expression_contains_interpolated_string(&catch.expression),
        Expr::Borrow(borrow) => expression_contains_interpolated_string(&borrow.expression),
        Expr::Call(call) => {
            expression_contains_interpolated_string(&call.callee)
                || call
                    .arguments
                    .iter()
                    .any(expression_contains_interpolated_string)
        }
        Expr::Member(member) => expression_contains_interpolated_string(&member.object),
        Expr::Index(index) => {
            expression_contains_interpolated_string(&index.object)
                || expression_contains_interpolated_string(&index.index)
        }
        Expr::ArrayLiteral(array) => array
            .elements
            .iter()
            .any(expression_contains_interpolated_string),
        Expr::StructLiteral(struct_literal) => struct_literal
            .fields
            .iter()
            .any(|field| expression_contains_interpolated_string(&field.value)),
        Expr::OptionalDefault(optional_default) => {
            expression_contains_interpolated_string(&optional_default.value)
                || expression_contains_interpolated_string(&optional_default.default)
        }
        Expr::PatternConditional(pattern_conditional) => {
            expression_contains_interpolated_string(&pattern_conditional.target)
                || pattern_conditional
                    .arms
                    .iter()
                    .any(|arm| expression_contains_interpolated_string(&arm.expression))
                || expression_contains_interpolated_string(&pattern_conditional.fallback)
        }
        _ => false,
    }
}

pub(super) fn bool_comparison_contains_call(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && expressions_are_lowerable_bool_comparison_operands_with_calls(
        &binary.left,
        &binary.right,
        context,
    )
}

pub(super) fn bool_comparison_needs_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && expressions_are_lowerable_bool_values(&binary.left, &binary.right, context)
        && !expressions_are_lowerable_bool_comparison_operands(&binary.left, &binary.right, context)
        && (expression_is_aggregate_field_kind(&binary.left, AggregateFieldKind::Bool, context)
            || expression_is_aggregate_field_kind(&binary.right, AggregateFieldKind::Bool, context))
}

pub(super) fn i32_comparison_needs_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    is_i32_comparison_operator(binary.operator)
        && expressions_are_lowerable_i32_expressions(&binary.left, &binary.right, context)
        && !expressions_are_lowerable_i32_values(&binary.left, &binary.right, context)
}

pub(super) fn usize_comparison_needs_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    is_i32_comparison_operator(binary.operator)
        && expressions_are_lowerable_usize_expressions(&binary.left, &binary.right, context)
        && !expressions_are_lowerable_usize_values(&binary.left, &binary.right, context)
}

pub(super) fn u8_comparison_is_lowerable(binary: &BinaryExpr, context: &LoweringContext) -> bool {
    is_i32_comparison_operator(binary.operator)
        && expressions_are_lowerable_u8_expressions(&binary.left, &binary.right, context)
        && (expression_is_known_u8_expression(&binary.left, context)
            || expression_is_known_u8_expression(&binary.right, context))
}

pub(in crate::ir::lower) fn expression_is_lowerable_bool_binding(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::BoolLiteral(_) => true,
        Expr::Identifier(identifier) => context.bool_location(&identifier.name).is_some(),
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::Bool, context)
        }
        Expr::Unary(unary) => {
            unary.operator == UnaryOperator::LogicalNot
                && expression_is_lowerable_bool_binding(&unary.operand, context)
        }
        Expr::Binary(binary) => {
            expression_is_lowerable_comparison_binding(binary, context)
                || (is_bool_logical_operator(binary.operator)
                    && expression_is_lowerable_bool_binding(&binary.left, context)
                    && expression_is_lowerable_bool_binding(&binary.right, context))
        }
        Expr::Group(group) => expression_is_lowerable_bool_binding(&group.expression, context),
        _ => false,
    }
}

pub(in crate::ir::lower) fn expression_is_unsupported_bool_comparison_binding(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::Binary(binary) => {
            is_bool_equality_operator(binary.operator)
                && expressions_are_lowerable_bool_values(&binary.left, &binary.right, context)
                && !expressions_are_lowerable_bool_comparison_operands(
                    &binary.left,
                    &binary.right,
                    context,
                )
        }
        Expr::Group(group) => {
            expression_is_unsupported_bool_comparison_binding(&group.expression, context)
        }
        _ => false,
    }
}

pub(super) fn expressions_are_lowerable_bool_comparison_operands(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_comparison_operand(left, context)
        && expression_is_lowerable_bool_comparison_operand(right, context)
}

pub(super) fn expressions_are_lowerable_bool_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_binding(left, context)
        && expression_is_lowerable_bool_binding(right, context)
}

pub(super) fn expressions_are_lowerable_usize_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_usize_value(left, context)
        && expression_is_lowerable_usize_value(right, context)
}

pub(super) fn is_i32_binary_operator(operator: BinaryOperator) -> bool {
    is_integer_binary_operator(operator)
}

pub(super) fn is_usize_binary_operator(operator: BinaryOperator) -> bool {
    is_integer_binary_operator(operator)
}

fn is_integer_binary_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Remainder
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
    )
}

fn is_i32_comparison_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual
    )
}

fn expression_is_lowerable_comparison_binding(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    if is_i32_comparison_operator(binary.operator) && u8_comparison_is_lowerable(binary, context) {
        return true;
    }

    if is_i32_comparison_operator(binary.operator)
        && (expressions_are_lowerable_i32_values(&binary.left, &binary.right, context)
            || expressions_are_lowerable_i32_expressions(&binary.left, &binary.right, context))
    {
        return true;
    }

    if is_i32_comparison_operator(binary.operator)
        && (expressions_are_lowerable_usize_values(&binary.left, &binary.right, context)
            || expressions_are_lowerable_usize_expressions(&binary.left, &binary.right, context))
    {
        return true;
    }

    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && (expressions_are_lowerable_bool_comparison_operands(&binary.left, &binary.right, context)
        || expressions_are_lowerable_bool_comparison_operands_with_calls(
            &binary.left,
            &binary.right,
            context,
        )
        || bool_comparison_needs_temporaries(binary, context))
}

fn expressions_are_lowerable_i32_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_i32_value(left, context)
        && expression_is_lowerable_i32_value(right, context)
}

fn expressions_are_lowerable_i32_expressions(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_i32_expression(left, context)
        && expression_is_lowerable_i32_expression(right, context)
}

fn expressions_are_lowerable_usize_expressions(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_usize_expression(left, context)
        && expression_is_lowerable_usize_expression(right, context)
}

fn expressions_are_lowerable_u8_expressions(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_u8_expression(left, context)
        && expression_is_lowerable_u8_expression(right, context)
}

fn expression_is_lowerable_u8_expression(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.call_return_type(&context.call_target(call, &identifier.name))
                == Some(&Type::U8)
        }
        Expr::Index(index) => {
            expression_is_lowerable_byte_index_object(&index.object, context)
                && expression_is_lowerable_usize_expression(&index.index, context)
        }
        Expr::TypeConversion(conversion) if type_conversion_target_is(conversion, "u8") => {
            expression_is_lowerable_u8_expression(&conversion.expression, context)
        }
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::U8, context)
        }
        Expr::Group(group) => expression_is_lowerable_u8_expression(&group.expression, context),
        _ => expression_is_lowerable_u8_value(expression, context),
    }
}

fn expression_is_known_u8_expression(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Identifier(identifier) => context.u8_location(&identifier.name).is_some(),
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.call_return_type(&context.call_target(call, &identifier.name))
                == Some(&Type::U8)
        }
        Expr::Index(index) => expression_is_lowerable_byte_index_object(&index.object, context),
        Expr::TypeConversion(conversion) if type_conversion_target_is(conversion, "u8") => true,
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::U8, context)
        }
        Expr::Group(group) => expression_is_known_u8_expression(&group.expression, context),
        _ => false,
    }
}

fn expression_is_lowerable_u8_value(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Identifier(identifier) => context.u8_location(&identifier.name).is_some(),
        Expr::Group(group) => expression_is_lowerable_u8_value(&group.expression, context),
        _ => lower_u8_literal(expression).is_ok(),
    }
}

fn expression_is_lowerable_byte_index_object(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::StringLiteral(_) => true,
        Expr::Identifier(identifier) => {
            context.str_location(&identifier.name).is_some()
                || context.slice_location(&identifier.name).is_some()
        }
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            matches!(
                context.call_return_type(&context.call_target(call, &identifier.name)),
                Some(Type::Str | Type::Slice { .. })
            )
        }
        Expr::Group(group) => expression_is_lowerable_byte_index_object(&group.expression, context),
        _ => false,
    }
}

fn type_conversion_target_is(conversion: &TypeConversionExpr, name: &str) -> bool {
    matches!(&conversion.ty, TypeExpr::Reference(reference) if reference.name == name)
}

fn expression_is_lowerable_usize_expression(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.call_return_type(&context.call_target(call, &identifier.name))
                == Some(&Type::Usize)
        }
        Expr::Binary(binary) if is_usize_binary_operator(binary.operator) => {
            expression_is_lowerable_usize_expression(&binary.left, context)
                && expression_is_lowerable_usize_expression(&binary.right, context)
        }
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::Usize, context)
        }
        Expr::Group(group) => expression_is_lowerable_usize_expression(&group.expression, context),
        _ => expression_is_lowerable_usize_value(expression, context),
    }
}

fn expression_is_lowerable_usize_value(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Identifier(identifier) => context.usize_location(&identifier.name).is_some(),
        Expr::Group(group) => expression_is_lowerable_usize_value(&group.expression, context),
        _ => lower_usize_literal(expression).is_ok(),
    }
}

fn expression_is_lowerable_i32_expression(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.call_return_type(&context.call_target(call, &identifier.name))
                == Some(&Type::I32)
        }
        Expr::Binary(binary) if is_i32_binary_operator(binary.operator) => {
            expression_is_lowerable_i32_expression(&binary.left, context)
                && expression_is_lowerable_i32_expression(&binary.right, context)
        }
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::I32, context)
        }
        Expr::Group(group) => expression_is_lowerable_i32_expression(&group.expression, context),
        _ => expression_is_lowerable_i32_value(expression, context),
    }
}

fn expression_is_aggregate_field_kind(
    expression: &Expr,
    kind: AggregateFieldKind,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::Member(member) => aggregate_member_field_kind(member, context)
            .is_some_and(|field_kind| field_kind == kind),
        Expr::Group(group) => expression_is_aggregate_field_kind(&group.expression, kind, context),
        _ => false,
    }
}

fn aggregate_member_field_kind(
    member: &crate::ast::MemberExpr,
    context: &LoweringContext,
) -> Option<AggregateFieldKind> {
    let (root, mut fields) = aggregate_member_root_and_path(&member.object)?;
    fields.push(member.member.as_str());
    let field_path = fields.join(".");
    match root {
        AggregateMemberRoot::Identifier(identifier_name) => context
            .aggregate_field(identifier_name, &field_path)
            .map(|field| field.kind),
        AggregateMemberRoot::Call(call) => aggregate_call_field_kind(call, &field_path, context),
        AggregateMemberRoot::FallibleCall(call) => {
            aggregate_fallible_call_field_kind(call, &field_path, context)
        }
    }
}

enum AggregateMemberRoot<'a> {
    Identifier(&'a str),
    Call(&'a crate::ast::CallExpr),
    FallibleCall(&'a crate::ast::CallExpr),
}

fn aggregate_member_root_and_path<'a>(
    expression: &'a Expr,
) -> Option<(AggregateMemberRoot<'a>, Vec<&'a str>)> {
    match expression {
        Expr::Identifier(identifier) => Some((
            AggregateMemberRoot::Identifier(&identifier.name),
            Vec::new(),
        )),
        Expr::Call(call) => Some((AggregateMemberRoot::Call(call), Vec::new())),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return None;
            };
            Some((AggregateMemberRoot::FallibleCall(call), Vec::new()))
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return None;
            };
            Some((AggregateMemberRoot::FallibleCall(call), Vec::new()))
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return None;
            };
            Some((AggregateMemberRoot::FallibleCall(call), Vec::new()))
        }
        Expr::Member(member) => {
            let (root, mut fields) = aggregate_member_root_and_path(&member.object)?;
            fields.push(member.member.as_str());
            Some((root, fields))
        }
        Expr::Group(group) => aggregate_member_root_and_path(&group.expression),
        _ => None,
    }
}

fn aggregate_call_field_kind(
    call: &crate::ast::CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<AggregateFieldKind> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return None;
    };
    let target = context.call_target(call, &identifier.name);
    let layout = aggregate_type_layout(context.call_return_type(&target)?)?;
    if !supported_aggregate_copy_layout(layout) {
        return None;
    }

    aggregate_call_field(call, member_name, context).map(|field| field.kind)
}

fn aggregate_fallible_call_field_kind(
    call: &crate::ast::CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<AggregateFieldKind> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return None;
    };
    let target = context.call_target(call, &identifier.name);
    let layout = match context.call_return_type(&target)? {
        Type::Fallible(success_type) => aggregate_type_layout(success_type.as_ref())?,
        _ => return None,
    };
    if !supported_aggregate_copy_layout(layout) {
        return None;
    }

    aggregate_call_field(call, member_name, context).map(|field| field.kind)
}

fn expression_is_lowerable_i32_value(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Identifier(identifier) => context.i32_location(&identifier.name).is_some(),
        Expr::Group(group) => expression_is_lowerable_i32_value(&group.expression, context),
        _ => lower_i32_literal(expression).is_ok(),
    }
}

fn expression_is_lowerable_bool_comparison_operand(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::BoolLiteral(_) => true,
        Expr::Identifier(identifier) => context.bool_location(&identifier.name).is_some(),
        Expr::Group(group) => {
            expression_is_lowerable_bool_comparison_operand(&group.expression, context)
        }
        _ => false,
    }
}

fn expressions_are_lowerable_bool_comparison_operands_with_calls(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_comparison_operand_or_call(left, context)
        && expression_is_lowerable_bool_comparison_operand_or_call(right, context)
        && (expression_contains_call(left) || expression_contains_call(right))
}

fn expression_is_lowerable_bool_comparison_operand_or_call(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_comparison_operand(expression, context)
        || expression_is_direct_bool_returning_call(expression, context)
}

fn expression_is_direct_bool_returning_call(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.call_return_type(&context.call_target(call, &identifier.name))
                == Some(&Type::Bool)
        }
        Expr::Group(group) => expression_is_direct_bool_returning_call(&group.expression, context),
        _ => false,
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn is_bool_logical_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
    )
}

fn is_bool_equality_operator(operator: BinaryOperator) -> bool {
    matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
}

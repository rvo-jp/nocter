use super::*;

pub(in crate::ir::lower::expressions) fn bool_comparison_contains_call(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && expressions_are_lowerable_bool_expressions(&binary.left, &binary.right, context)
        && (expression_contains_call(&binary.left) || expression_contains_call(&binary.right))
}

pub(in crate::ir::lower::expressions) fn bool_comparison_needs_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && expressions_are_lowerable_bool_values(&binary.left, &binary.right, context)
        && !expressions_are_lowerable_bool_comparison_operands(&binary.left, &binary.right, context)
}

pub(in crate::ir::lower::expressions) fn i32_comparison_needs_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    is_i32_comparison_operator(binary.operator)
        && expressions_are_lowerable_i32_expressions(&binary.left, &binary.right, context)
        && !expressions_are_lowerable_i32_values(&binary.left, &binary.right, context)
}

pub(in crate::ir::lower::expressions) fn usize_comparison_needs_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    is_i32_comparison_operator(binary.operator)
        && expressions_are_lowerable_usize_expressions(&binary.left, &binary.right, context)
        && !expressions_are_lowerable_usize_values(&binary.left, &binary.right, context)
}

pub(in crate::ir::lower::expressions) fn u8_comparison_is_lowerable(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    is_i32_comparison_operator(binary.operator)
        && expressions_are_lowerable_u8_expressions(&binary.left, &binary.right, context)
        && (expression_is_known_u8_expression(&binary.left, context)
            || expression_is_known_u8_expression(&binary.right, context))
}

pub(in crate::ir::lower::expressions) fn str_comparison_is_lowerable(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && expressions_are_lowerable_str_expressions(&binary.left, &binary.right, context)
}

pub(in crate::ir::lower::expressions) fn str_comparison_needs_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    str_comparison_is_lowerable(binary, context)
        && (expression_contains_call(&binary.left)
            || expression_contains_call(&binary.right)
            || !expressions_are_lowerable_str_values(&binary.left, &binary.right, context))
}

pub(in crate::ir::lower::expressions) fn u8_comparison_needs_temporaries(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    u8_comparison_is_lowerable(binary, context)
        && !expressions_are_lowerable_u8_values(&binary.left, &binary.right, context)
}

pub(in crate::ir::lower) fn expression_is_lowerable_bool_binding(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    match expression {
        Expr::BoolLiteral(_) => true,
        Expr::Identifier(identifier) => {
            context.bool_location(&identifier.name).is_some()
                || identifier_is_borrow_or_closure_scalar(identifier, Type::Bool, context)
        }
        Expr::Call(call) => direct_call_return_type(call, context) == Some(&Type::Bool),
        Expr::Index(index) => {
            expression_is_lowerable_bool_slice_index_object(&index.object, context)
                && expression_is_lowerable_usize_expression(&index.index, context)
        }
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

pub(in crate::ir::lower::expressions) fn expressions_are_lowerable_bool_comparison_operands(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_comparison_operand(left, context)
        && expression_is_lowerable_bool_comparison_operand(right, context)
}

pub(in crate::ir::lower::expressions) fn expressions_are_lowerable_bool_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_binding(left, context)
        && expression_is_lowerable_bool_binding(right, context)
}

pub(in crate::ir::lower::expressions) fn expressions_are_lowerable_usize_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_usize_value(left, context)
        && expression_is_lowerable_usize_value(right, context)
}

pub(in crate::ir::lower::expressions) fn is_i32_binary_operator(operator: BinaryOperator) -> bool {
    is_integer_binary_operator(operator)
}

pub(in crate::ir::lower::expressions) fn is_usize_binary_operator(
    operator: BinaryOperator,
) -> bool {
    is_integer_binary_operator(operator)
}

pub(in crate::ir::lower::expressions) fn is_u8_binary_operator(operator: BinaryOperator) -> bool {
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

    if str_comparison_is_lowerable(binary, context) {
        return true;
    }

    matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && expressions_are_lowerable_bool_expressions(&binary.left, &binary.right, context)
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

fn expressions_are_lowerable_u8_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_u8_value(left, context)
        && expression_is_lowerable_u8_value(right, context)
}

fn expressions_are_lowerable_bool_expressions(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_bool_expression(left, context)
        && expression_is_lowerable_bool_expression(right, context)
}

fn expression_is_lowerable_bool_expression(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::BoolLiteral(_) => true,
        Expr::Identifier(identifier) => {
            context.bool_location(&identifier.name).is_some()
                || identifier_is_borrow_or_closure_scalar(identifier, Type::Bool, context)
        }
        Expr::Call(call) => direct_call_return_type(call, context) == Some(&Type::Bool),
        Expr::Index(index) => {
            expression_is_lowerable_bool_slice_index_object(&index.object, context)
                && expression_is_lowerable_usize_expression(&index.index, context)
        }
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::Bool, context)
        }
        Expr::Unary(unary) => {
            unary.operator == UnaryOperator::LogicalNot
                && expression_is_lowerable_bool_expression(&unary.operand, context)
        }
        Expr::Binary(binary) => expression_is_lowerable_bool_binary(binary, context),
        Expr::Group(group) => expression_is_lowerable_bool_expression(&group.expression, context),
        _ => false,
    }
}

fn expression_is_lowerable_bool_binary(binary: &BinaryExpr, context: &LoweringContext) -> bool {
    match binary.operator {
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
            expression_is_lowerable_bool_expression(&binary.left, context)
                && expression_is_lowerable_bool_expression(&binary.right, context)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            u8_comparison_is_lowerable(binary, context)
                || expressions_are_lowerable_i32_values(&binary.left, &binary.right, context)
                || expressions_are_lowerable_i32_expressions(&binary.left, &binary.right, context)
                || expressions_are_lowerable_usize_values(&binary.left, &binary.right, context)
                || expressions_are_lowerable_usize_expressions(&binary.left, &binary.right, context)
                || str_comparison_is_lowerable(binary, context)
                || expressions_are_lowerable_bool_expressions(&binary.left, &binary.right, context)
        }
        _ if is_i32_comparison_operator(binary.operator) => {
            u8_comparison_is_lowerable(binary, context)
                || expressions_are_lowerable_i32_values(&binary.left, &binary.right, context)
                || expressions_are_lowerable_i32_expressions(&binary.left, &binary.right, context)
                || expressions_are_lowerable_usize_values(&binary.left, &binary.right, context)
                || expressions_are_lowerable_usize_expressions(&binary.left, &binary.right, context)
        }
        _ => false,
    }
}

fn expression_is_lowerable_u8_expression(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Identifier(identifier) => {
            context.u8_location(&identifier.name).is_some()
                || identifier_is_borrow_or_closure_scalar(identifier, Type::U8, context)
        }
        Expr::Binary(binary) if is_u8_binary_operator(binary.operator) => {
            expression_is_lowerable_u8_expression(&binary.left, context)
                && expression_is_lowerable_u8_expression(&binary.right, context)
        }
        Expr::Call(call) => direct_call_return_type(call, context) == Some(&Type::U8),
        Expr::Index(index) => {
            expression_is_lowerable_byte_index_object(&index.object, context)
                && expression_is_lowerable_usize_expression(&index.index, context)
        }
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::U8) =>
        {
            expression_is_lowerable_u8_expression(&conversion.expression, context)
        }
        Expr::Member(member) if context.payloadless_enum_variant_tag(member).is_some() => true,
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::U8, context)
        }
        Expr::Group(group) => expression_is_lowerable_u8_expression(&group.expression, context),
        _ => expression_is_lowerable_u8_value(expression, context),
    }
}

fn expression_is_known_u8_expression(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::ByteLiteral(_) => true,
        Expr::Identifier(identifier) => {
            context.u8_location(&identifier.name).is_some()
                || identifier_is_borrow_or_closure_scalar(identifier, Type::U8, context)
        }
        Expr::Call(call) => direct_call_return_type(call, context) == Some(&Type::U8),
        Expr::Index(index) => expression_is_lowerable_byte_index_object(&index.object, context),
        Expr::TypeConversion(conversion)
            if type_conversion_target_is(conversion, context, Type::U8) =>
        {
            true
        }
        Expr::Member(member) if context.payloadless_enum_variant_tag(member).is_some() => true,
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
        Expr::Member(member) => context.payloadless_enum_variant_tag(member).is_some(),
        Expr::Group(group) => expression_is_lowerable_u8_value(&group.expression, context),
        _ => lower_u8_literal(expression).is_ok(),
    }
}

fn expressions_are_lowerable_str_expressions(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_str_expression(left, context)
        && expression_is_lowerable_str_expression(right, context)
}

fn expressions_are_lowerable_str_values(
    left: &Expr,
    right: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_is_lowerable_str_value(left, context)
        && expression_is_lowerable_str_value(right, context)
}

fn expression_is_lowerable_str_expression(expression: &Expr, context: &LoweringContext) -> bool {
    if context.expression_ir_type(expression) == Some(Type::Str) {
        return true;
    }
    match expression {
        Expr::Call(call) => direct_call_return_type(call, context) == Some(&Type::Str),
        Expr::Propagate(propagation) => {
            expression_is_lowerable_outcome_str_expression(&propagation.expression, context)
        }
        Expr::Force(force) => {
            expression_is_lowerable_outcome_str_expression(&force.expression, context)
        }
        Expr::Catch(catch) => {
            expression_is_lowerable_outcome_str_expression(&catch.expression, context)
        }
        Expr::Index(index) => {
            expression_is_lowerable_str_slice_index_object(&index.object, context)
                && expression_is_lowerable_usize_expression(&index.index, context)
        }
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::Str, context)
        }
        Expr::Group(group) => expression_is_lowerable_str_expression(&group.expression, context),
        _ => expression_is_lowerable_str_value(expression, context),
    }
}

fn expression_is_lowerable_outcome_str_expression(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    let Expr::Call(call) = unwrap_group(expression) else {
        return false;
    };
    direct_call_return_type(call, context)
        .and_then(Type::single_outcome)
        .is_some_and(|(_, payload)| payload == &Type::Str)
}

fn expression_is_lowerable_str_value(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Identifier(identifier) => context.str_location(&identifier.name).is_some(),
        Expr::Group(group) => expression_is_lowerable_str_value(&group.expression, context),
        _ => lower_str_literal(expression).is_ok(),
    }
}

fn expression_is_lowerable_byte_index_object(expression: &Expr, context: &LoweringContext) -> bool {
    if context.expression_ir_type(expression) == Some(Type::Str)
        || context.expression_slice_element_kind(expression) == Some(TypecheckSliceElementKind::U8)
    {
        return true;
    }
    match expression {
        Expr::StringLiteral(_) => true,
        Expr::Identifier(identifier) => {
            context.str_location(&identifier.name).is_some()
                || identifier_slice_element_kind(identifier, context)
                    == Some(TypecheckSliceElementKind::U8)
        }
        Expr::Call(call) => {
            direct_call_return_type(call, context) == Some(&Type::Str)
                || call_return_slice_element_type(call, context) == Some(Type::U8)
        }
        Expr::Member(member) => aggregate_member_field_kind(member, context).is_some_and(|kind| {
            kind == AggregateFieldKind::Str
                || matches!(
                    kind,
                    AggregateFieldKind::Slice(info)
                        if info.element_kind == TypecheckSliceElementKind::U8
                )
        }),
        Expr::Group(group) => expression_is_lowerable_byte_index_object(&group.expression, context),
        _ => false,
    }
}

fn expression_is_lowerable_slice_index_object(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    if context.expression_slice_element_kind(expression) == Some(TypecheckSliceElementKind::Usize) {
        return true;
    }
    match expression {
        Expr::Identifier(identifier) => {
            identifier_slice_element_kind(identifier, context)
                == Some(TypecheckSliceElementKind::Usize)
        }
        Expr::Call(call) => call_return_slice_element_type(call, context) == Some(Type::Usize),
        Expr::Member(member) => expression_is_aggregate_slice_field_element_kind(
            member,
            TypecheckSliceElementKind::Usize,
            context,
        ),
        Expr::Group(group) => {
            expression_is_lowerable_slice_index_object(&group.expression, context)
        }
        _ => false,
    }
}

fn expression_is_lowerable_i32_slice_index_object(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    if context.expression_slice_element_kind(expression) == Some(TypecheckSliceElementKind::I32) {
        return true;
    }
    match expression {
        Expr::Identifier(identifier) => {
            identifier_slice_element_kind(identifier, context)
                == Some(TypecheckSliceElementKind::I32)
        }
        Expr::Call(call) => call_return_slice_element_type(call, context) == Some(Type::I32),
        Expr::Member(member) => expression_is_aggregate_slice_field_element_kind(
            member,
            TypecheckSliceElementKind::I32,
            context,
        ),
        Expr::Group(group) => {
            expression_is_lowerable_i32_slice_index_object(&group.expression, context)
        }
        _ => false,
    }
}

fn expression_is_lowerable_bool_slice_index_object(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    if context.expression_slice_element_kind(expression) == Some(TypecheckSliceElementKind::Bool) {
        return true;
    }
    match expression {
        Expr::Identifier(identifier) => {
            identifier_slice_element_kind(identifier, context)
                == Some(TypecheckSliceElementKind::Bool)
        }
        Expr::Call(call) => call_return_slice_element_type(call, context) == Some(Type::Bool),
        Expr::Member(member) => expression_is_aggregate_slice_field_element_kind(
            member,
            TypecheckSliceElementKind::Bool,
            context,
        ),
        Expr::Group(group) => {
            expression_is_lowerable_bool_slice_index_object(&group.expression, context)
        }
        _ => false,
    }
}

fn expression_is_lowerable_str_slice_index_object(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    if context.expression_slice_element_kind(expression) == Some(TypecheckSliceElementKind::Str) {
        return true;
    }
    match expression {
        Expr::Identifier(identifier) => {
            identifier_slice_element_kind(identifier, context)
                == Some(TypecheckSliceElementKind::Str)
        }
        Expr::Call(call) => call_return_slice_element_type(call, context) == Some(Type::Str),
        Expr::Member(member) => expression_is_aggregate_slice_field_element_kind(
            member,
            TypecheckSliceElementKind::Str,
            context,
        ),
        Expr::Group(group) => {
            expression_is_lowerable_str_slice_index_object(&group.expression, context)
        }
        _ => false,
    }
}

fn expression_is_aggregate_slice_field_element_kind(
    member: &crate::ast::MemberExpr,
    expected: TypecheckSliceElementKind,
    context: &LoweringContext,
) -> bool {
    aggregate_member_field_kind(member, context).is_some_and(|kind| {
        matches!(
            kind,
            AggregateFieldKind::Slice(info) if info.element_kind == expected
        )
    })
}

fn identifier_slice_element_kind(
    identifier: &crate::ast::IdentifierExpr,
    context: &LoweringContext,
) -> Option<TypecheckSliceElementKind> {
    context.slice_element_kind(&identifier.name)
}

fn call_return_slice_element_type(call: &CallExpr, context: &LoweringContext) -> Option<Type> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    view_element_type_from_type_expr(&return_type, resolved)
}

fn type_conversion_target_is(
    conversion: &TypeConversionExpr,
    context: &LoweringContext,
    expected: Type,
) -> bool {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return false;
    };
    scalar_or_view_type_from_type_expr(&conversion.ty, resolved) == Some(expected)
}

fn expression_is_lowerable_usize_expression(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Identifier(identifier) => {
            context.usize_location(&identifier.name).is_some()
                || identifier_is_borrow_or_closure_scalar(identifier, Type::Usize, context)
        }
        Expr::Call(call) => {
            primitive_current_allocation_state_call(call, context)
                || primitive_current_allocation_kind_call(call, context)
                || direct_call_return_type(call, context) == Some(&Type::Usize)
        }
        Expr::Binary(binary) if is_usize_binary_operator(binary.operator) => {
            expression_is_lowerable_usize_expression(&binary.left, context)
                && expression_is_lowerable_usize_expression(&binary.right, context)
        }
        Expr::Index(index) => {
            expression_is_lowerable_slice_index_object(&index.object, context)
                && expression_is_lowerable_usize_expression(&index.index, context)
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
        Expr::Identifier(identifier) => {
            context.i32_location(&identifier.name).is_some()
                || context
                    .borrow_parameter(&identifier.name)
                    .is_some_and(|borrow| borrow.inner == Type::I32)
                || context
                    .borrow_local(&identifier.name)
                    .is_some_and(|(_, _, inner)| inner == &Type::I32)
                || context
                    .closure_capture_field(&identifier.name)
                    .is_some_and(|field| {
                        matches!(
                            &field.kind,
                            AggregateFieldKind::I32
                                | AggregateFieldKind::Borrow {
                                    inner: Type::I32,
                                    ..
                                }
                        )
                    })
        }
        Expr::Call(call) => direct_call_return_type(call, context) == Some(&Type::I32),
        Expr::Binary(binary) if is_i32_binary_operator(binary.operator) => {
            expression_is_lowerable_i32_expression(&binary.left, context)
                && expression_is_lowerable_i32_expression(&binary.right, context)
        }
        Expr::Index(index) => {
            expression_is_lowerable_i32_slice_index_object(&index.object, context)
                && expression_is_lowerable_usize_expression(&index.index, context)
        }
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::I32, context)
        }
        Expr::Group(group) => expression_is_lowerable_i32_expression(&group.expression, context),
        _ => expression_is_lowerable_i32_value(expression, context),
    }
}

pub(in crate::ir::lower::expressions) fn expression_is_aggregate_field_kind(
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
    super::super::aggregate_member_field_kind_from_member(member, context)
        .ok()
        .flatten()
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
        Expr::Unary(unary) => {
            unary.operator == UnaryOperator::LogicalNot
                && expression_is_lowerable_bool_comparison_operand(&unary.operand, context)
        }
        Expr::Binary(binary) => match binary.operator {
            BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
                expression_is_lowerable_bool_comparison_operand(&binary.left, context)
                    && expression_is_lowerable_bool_comparison_operand(&binary.right, context)
            }
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                expressions_are_lowerable_bool_comparison_operands(
                    &binary.left,
                    &binary.right,
                    context,
                )
            }
            _ => false,
        },
        Expr::Group(group) => {
            expression_is_lowerable_bool_comparison_operand(&group.expression, context)
        }
        _ => false,
    }
}

fn identifier_is_borrow_or_closure_scalar(
    identifier: &crate::ast::IdentifierExpr,
    expected: Type,
    context: &LoweringContext,
) -> bool {
    context
        .borrow_parameter(&identifier.name)
        .is_some_and(|borrow| borrow.inner == expected)
        || context
            .borrow_local(&identifier.name)
            .is_some_and(|(_, _, inner)| inner == &expected)
        || context
            .closure_capture_field(&identifier.name)
            .is_some_and(|field| match (&field.kind, &expected) {
                (AggregateFieldKind::U8, Type::U8)
                | (AggregateFieldKind::Usize, Type::Usize)
                | (AggregateFieldKind::Bool, Type::Bool) => true,
                (AggregateFieldKind::Borrow { inner, .. }, expected) => inner == expected,
                _ => false,
            })
}

fn direct_call_return_type<'a>(call: &CallExpr, context: &'a LoweringContext) -> Option<&'a Type> {
    let (target, _) = context.direct_call_target_and_name(call)?;
    context.call_return_type(&target)
}

pub(in crate::ir::lower::expressions) fn unwrap_group(expression: &Expr) -> &Expr {
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

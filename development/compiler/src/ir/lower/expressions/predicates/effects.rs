use super::*;

pub(in crate::ir::lower) fn short_circuit_bool_expression_needs_branch(
    binary: &BinaryExpr,
    context: &LoweringContext,
) -> bool {
    matches!(
        binary.operator,
        BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
    ) && (expression_contains_call(&binary.left)
        || expression_contains_call(&binary.right)
        || bool_expression_needs_temporaries(&binary.left, context)
        || bool_expression_needs_temporaries(&binary.right, context))
}

fn bool_expression_needs_temporaries(expression: &Expr, context: &LoweringContext) -> bool {
    match unwrap_group(expression) {
        Expr::Member(_) => {
            expression_is_aggregate_field_kind(expression, AggregateFieldKind::Bool, context)
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            bool_expression_needs_temporaries(&unary.operand, context)
        }
        Expr::Binary(binary) => {
            short_circuit_bool_expression_needs_branch(binary, context)
                || bool_comparison_contains_call(binary, context)
                || bool_comparison_needs_temporaries(binary, context)
                || str_comparison_needs_temporaries(binary, context)
                || u8_comparison_needs_temporaries(binary, context)
                || i32_comparison_needs_temporaries(binary, context)
                || usize_comparison_needs_temporaries(binary, context)
        }
        Expr::Group(group) => bool_expression_needs_temporaries(&group.expression, context),
        _ => false,
    }
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
        Expr::Otherwise(otherwise) => {
            expression_contains_call(&otherwise.value) || block_contains_call(&otherwise.fallback)
        }
        Expr::If(statement) => {
            expression_contains_call(&statement.condition)
                || block_contains_call(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_call)
        }
        Expr::IfIs(statement) => {
            expression_contains_call(&statement.expression)
                || block_contains_call(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_call)
        }
        Expr::Match(statement) => {
            expression_contains_call(&statement.expression)
                || statement
                    .arms
                    .iter()
                    .any(|arm| block_contains_call(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_contains_call(&arm.body))
        }
        _ => false,
    }
}

fn block_contains_call(block: &crate::ast::Block) -> bool {
    block.statements.iter().any(statement_contains_call)
        || block
            .result
            .as_deref()
            .is_some_and(expression_contains_call)
}

fn statement_contains_call(statement: &crate::ast::Stmt) -> bool {
    match statement {
        crate::ast::Stmt::Import(_) | crate::ast::Stmt::FromImport(_) => false,
        crate::ast::Stmt::Return(statement) => statement
            .expression
            .as_ref()
            .is_some_and(expression_contains_call),
        crate::ast::Stmt::Binding(statement) => expression_contains_call(&statement.initializer),
        crate::ast::Stmt::Assignment(statement) => {
            expression_contains_call(&statement.target)
                || expression_contains_call(&statement.value)
        }
        crate::ast::Stmt::If(statement) => {
            expression_contains_call(&statement.condition)
                || block_contains_call(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_call)
        }
        crate::ast::Stmt::IfIs(statement) => {
            expression_contains_call(&statement.expression)
                || block_contains_call(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_call)
        }
        crate::ast::Stmt::Switch(statement) => {
            expression_contains_call(&statement.expression)
                || statement
                    .arms
                    .iter()
                    .any(|arm| block_contains_call(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_contains_call(&arm.body))
        }
        crate::ast::Stmt::ForRange(statement) => {
            expression_contains_call(&statement.start)
                || expression_contains_call(&statement.end)
                || block_contains_call(&statement.body)
        }
        crate::ast::Stmt::While(statement) => {
            expression_contains_call(&statement.condition) || block_contains_call(&statement.body)
        }
        crate::ast::Stmt::Loop(statement) => block_contains_call(&statement.body),
        crate::ast::Stmt::Region(statement) => {
            expression_contains_call(&statement.allocator) || block_contains_call(&statement.body)
        }
        crate::ast::Stmt::Expression(statement) => expression_contains_call(&statement.expression),
        crate::ast::Stmt::Break(_) | crate::ast::Stmt::Continue(_) | crate::ast::Stmt::Drop(_) => {
            false
        }
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
        Expr::Otherwise(otherwise) => {
            expression_contains_interpolated_string(&otherwise.value)
                || block_contains_interpolated_string(&otherwise.fallback)
        }
        Expr::If(statement) => {
            expression_contains_interpolated_string(&statement.condition)
                || block_contains_interpolated_string(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_interpolated_string)
        }
        Expr::IfIs(statement) => {
            expression_contains_interpolated_string(&statement.expression)
                || block_contains_interpolated_string(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_interpolated_string)
        }
        Expr::Match(statement) => {
            expression_contains_interpolated_string(&statement.expression)
                || statement
                    .arms
                    .iter()
                    .any(|arm| block_contains_interpolated_string(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_contains_interpolated_string(&arm.body))
        }
        _ => false,
    }
}

fn block_contains_interpolated_string(block: &crate::ast::Block) -> bool {
    block
        .statements
        .iter()
        .any(statement_contains_interpolated_string)
        || block
            .result
            .as_deref()
            .is_some_and(expression_contains_interpolated_string)
}

fn statement_contains_interpolated_string(statement: &crate::ast::Stmt) -> bool {
    match statement {
        crate::ast::Stmt::Import(_) | crate::ast::Stmt::FromImport(_) => false,
        crate::ast::Stmt::Return(statement) => statement
            .expression
            .as_ref()
            .is_some_and(expression_contains_interpolated_string),
        crate::ast::Stmt::Binding(statement) => {
            expression_contains_interpolated_string(&statement.initializer)
        }
        crate::ast::Stmt::Assignment(statement) => {
            expression_contains_interpolated_string(&statement.target)
                || expression_contains_interpolated_string(&statement.value)
        }
        crate::ast::Stmt::If(statement) => {
            expression_contains_interpolated_string(&statement.condition)
                || block_contains_interpolated_string(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_interpolated_string)
        }
        crate::ast::Stmt::IfIs(statement) => {
            expression_contains_interpolated_string(&statement.expression)
                || block_contains_interpolated_string(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_contains_interpolated_string)
        }
        crate::ast::Stmt::Switch(statement) => {
            expression_contains_interpolated_string(&statement.expression)
                || statement
                    .arms
                    .iter()
                    .any(|arm| block_contains_interpolated_string(&arm.body))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_contains_interpolated_string(&arm.body))
        }
        crate::ast::Stmt::ForRange(statement) => {
            expression_contains_interpolated_string(&statement.start)
                || expression_contains_interpolated_string(&statement.end)
                || block_contains_interpolated_string(&statement.body)
        }
        crate::ast::Stmt::While(statement) => {
            expression_contains_interpolated_string(&statement.condition)
                || block_contains_interpolated_string(&statement.body)
        }
        crate::ast::Stmt::Loop(statement) => block_contains_interpolated_string(&statement.body),
        crate::ast::Stmt::Region(statement) => {
            expression_contains_interpolated_string(&statement.allocator)
                || block_contains_interpolated_string(&statement.body)
        }
        crate::ast::Stmt::Expression(statement) => {
            expression_contains_interpolated_string(&statement.expression)
        }
        crate::ast::Stmt::Break(_) | crate::ast::Stmt::Continue(_) | crate::ast::Stmt::Drop(_) => {
            false
        }
    }
}

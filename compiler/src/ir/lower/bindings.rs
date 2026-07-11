use super::context::LoweringContext;
use super::expressions::{
    expression_contains_interpolated_string, expression_is_lowerable_bool_binding,
    expression_is_unsupported_bool_comparison_binding, lower_bool_expression_to_location,
    lower_i32_expression_to_location, lower_usize_expression_to_location,
};
use crate::ast::{BinaryOperator, BindingKind, BindingStmt, Expr, TypeExpr, UnaryOperator};
use crate::diagnostics::Diagnostic;
use crate::ir::{Instruction, Type};

pub(super) fn lower_let_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if statement.kind != BindingKind::Let {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower immutable `let` bindings",
        ));
    }

    if statement.else_block.is_some() {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower optional `let ... else` bindings",
        ));
    }

    if expression_contains_interpolated_string(&statement.initializer) {
        return Err(unsupported_interpolated_string_diagnostic());
    }

    match scalar_binding_kind(statement, context)? {
        ScalarBindingKind::I32 => lower_i32_let_binding(statement, context),
        ScalarBindingKind::Usize => lower_usize_let_binding(statement, context),
        ScalarBindingKind::Bool => lower_bool_let_binding(statement, context),
    }
}

fn lower_i32_let_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_i32_local_location()?;
    let instructions =
        lower_i32_expression_to_location(&statement.initializer, destination, context)?;
    context.define_i32_local(statement.name.clone());
    Ok(instructions)
}

fn lower_usize_let_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_usize_local_location()?;
    let instructions =
        lower_usize_expression_to_location(&statement.initializer, destination, context)?;
    context.define_usize_local(statement.name.clone());
    Ok(instructions)
}

fn lower_bool_let_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_bool_local_location()?;
    let instructions =
        lower_bool_expression_to_location(&statement.initializer, destination, context, "E8008")?;
    context.define_bool_local(statement.name.clone());
    Ok(instructions)
}

fn scalar_binding_kind(
    statement: &BindingStmt,
    context: &LoweringContext,
) -> Result<ScalarBindingKind, Vec<Diagnostic>> {
    match &statement.ty {
        Some(ty) if is_i32_type(ty) => Ok(ScalarBindingKind::I32),
        Some(ty) if is_usize_type(ty) => Ok(ScalarBindingKind::Usize),
        Some(ty) if is_bool_type(ty) => Ok(ScalarBindingKind::Bool),
        Some(_) => Err(unsupported_binding_diagnostic(
            "IR v0 can only lower local bindings annotated as `i32`, `usize`, or `bool`",
        )),
        None if expression_is_lowerable_bool_binding(&statement.initializer, context) => {
            Ok(ScalarBindingKind::Bool)
        }
        None if expression_is_bool_returning_call(&statement.initializer, context) => {
            Ok(ScalarBindingKind::Bool)
        }
        None if expression_is_unsupported_bool_comparison_binding(
            &statement.initializer,
            context,
        ) =>
        {
            Ok(ScalarBindingKind::Bool)
        }
        None => Ok(ScalarBindingKind::I32),
    }
}

fn expression_is_bool_returning_call(expression: &Expr, context: &LoweringContext) -> bool {
    match expression {
        Expr::Call(call) => {
            let Expr::Identifier(identifier) = call.callee.as_ref() else {
                return false;
            };
            context.call_return_type(&context.call_target(call, &identifier.name))
                == Some(&Type::Bool)
        }
        Expr::Unary(unary) => {
            unary.operator == UnaryOperator::LogicalNot
                && expression_is_bool_returning_call(&unary.operand, context)
        }
        Expr::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr
            ) =>
        {
            expression_is_bool_returning_call(&binary.left, context)
                && expression_is_bool_returning_call(&binary.right, context)
                || expression_is_lowerable_bool_binding(&binary.left, context)
                    && expression_is_bool_returning_call(&binary.right, context)
                || expression_is_bool_returning_call(&binary.left, context)
                    && expression_is_lowerable_bool_binding(&binary.right, context)
        }
        Expr::Group(group) => expression_is_bool_returning_call(&group.expression, context),
        _ => false,
    }
}

fn is_i32_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "i32")
}

fn is_usize_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "usize")
}

fn is_bool_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "bool")
}

fn unsupported_binding_diagnostic(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error("E8008", message)]
}

fn unsupported_interpolated_string_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 cannot lower interpolated string construction until explicit std/string allocation and std/fmt.append_* lowering are implemented",
    )]
}

enum ScalarBindingKind {
    I32,
    Usize,
    Bool,
}

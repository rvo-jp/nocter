use super::context::LoweringContext;
use super::expressions::{
    expression_is_lowerable_bool_binding, lower_bool_value, lower_i32_expression_to_location,
};
use crate::ast::{BindingKind, BindingStmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::Instruction;

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

    match scalar_binding_kind(statement, context)? {
        ScalarBindingKind::I32 => lower_i32_let_binding(statement, context),
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

fn lower_bool_let_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let destination = context.next_bool_local_location()?;
    let value = lower_bool_value(&statement.initializer, context, "E8008")?;
    context.define_bool_local(statement.name.clone());
    Ok(vec![Instruction::SetBool { destination, value }])
}

fn scalar_binding_kind(
    statement: &BindingStmt,
    context: &LoweringContext,
) -> Result<ScalarBindingKind, Vec<Diagnostic>> {
    match &statement.ty {
        Some(ty) if is_i32_type(ty) => Ok(ScalarBindingKind::I32),
        Some(ty) if is_bool_type(ty) => Ok(ScalarBindingKind::Bool),
        Some(_) => Err(unsupported_binding_diagnostic(
            "IR v0 can only lower local bindings annotated as `i32` or `bool`",
        )),
        None if expression_is_lowerable_bool_binding(&statement.initializer, context) => {
            Ok(ScalarBindingKind::Bool)
        }
        None => Ok(ScalarBindingKind::I32),
    }
}

fn is_i32_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "i32")
}

fn is_bool_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "bool")
}

fn unsupported_binding_diagnostic(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error("E8008", message)]
}

enum ScalarBindingKind {
    I32,
    Bool,
}

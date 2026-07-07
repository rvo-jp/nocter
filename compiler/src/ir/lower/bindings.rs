use super::expressions::{I32ExpressionContext, lower_i32_expression_to_location};
use crate::ast::{BindingKind, BindingStmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::Instruction;

pub(super) fn lower_i32_let_binding(
    statement: &BindingStmt,
    context: &mut I32ExpressionContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if statement.kind != BindingKind::Let {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower immutable `let` i32 bindings",
        ));
    }

    if statement.else_block.is_some() {
        return Err(unsupported_binding_diagnostic(
            "IR v0 cannot lower optional `let ... else` bindings",
        ));
    }

    if let Some(ty) = &statement.ty
        && !is_i32_type(ty)
    {
        return Err(unsupported_binding_diagnostic(
            "IR v0 can only lower local bindings annotated as `i32`",
        ));
    }

    let destination = context.next_local_location()?;
    let instructions =
        lower_i32_expression_to_location(&statement.initializer, destination, context)?;
    context.define_local(statement.name.clone());
    Ok(instructions)
}

fn is_i32_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "i32")
}

fn unsupported_binding_diagnostic(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error("E8008", message)]
}

use super::expressions::lower_i32_return_expression;
use crate::ast::{FunctionDecl, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{Function, Instruction, Type};

pub(super) fn lower_function(function: &FunctionDecl) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() || !function.parameters.parameters.is_empty() {
        return Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower non-generic zero-argument functions, got `{}`",
                function.name
            ),
        )]);
    }

    let return_type = lower_function_return_type(&function.return_type, &function.name)?;
    let instructions = lower_function_body(function, &return_type)?;

    Ok(Function {
        name: function.name.clone(),
        return_type,
        instructions,
    })
}

fn lower_function_return_type(ty: &TypeExpr, name: &str) -> Result<Type, Vec<Diagnostic>> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(Type::Void),
        _ => Err(vec![Diagnostic::error(
            "E8007",
            format!("IR v0 can only lower function `{name}` return type `i32` or `void`"),
        )]),
    }
}

fn lower_function_body(
    function: &FunctionDecl,
    return_type: &Type,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match function.body.statements.as_slice() {
        [Stmt::Return(statement)] => match (return_type, &statement.expression) {
            (Type::I32, Some(expression)) => lower_i32_return_expression(expression),
            (Type::Void, None) => Ok(vec![Instruction::Return]),
            (Type::Void, Some(_)) => Err(vec![Diagnostic::error(
                "E8007",
                format!(
                    "IR v0 cannot lower value returns from void function `{}`",
                    function.name
                ),
            )]),
            (Type::I32, None) => Err(vec![Diagnostic::error(
                "E8007",
                format!(
                    "IR v0 cannot lower bare returns from i32 function `{}`",
                    function.name
                ),
            )]),
            (Type::Fallible(_), _) => unreachable!("fallible function type is not lowered in v0"),
        },
        [] if *return_type == Type::Void => Ok(vec![Instruction::Return]),
        _ => Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower function `{}` bodies containing a single return",
                function.name
            ),
        )]),
    }
}

use super::expressions::{I32ExpressionContext, lower_i32_return_expression};
use crate::ast::{FunctionDecl, Parameter, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{Function, Instruction, Type};

pub(super) fn lower_function(function: &FunctionDecl) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() {
        return Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower non-generic functions, got `{}`",
                function.name
            ),
        )]);
    }

    let parameters = lower_i32_parameters(function)?;
    let return_type = lower_function_return_type(&function.return_type, &function.name)?;
    let context = I32ExpressionContext::new(parameters);
    let instructions = lower_function_body(function, &return_type, &context)?;

    Ok(Function {
        name: function.name.clone(),
        return_type,
        instructions,
    })
}

fn lower_i32_parameters(function: &FunctionDecl) -> Result<Vec<String>, Vec<Diagnostic>> {
    if function.parameters.parameters.len() > MAX_I32_PARAMETERS {
        return Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower up to {MAX_I32_PARAMETERS} i32 parameters for function `{}`",
                function.name
            ),
        )]);
    }

    function
        .parameters
        .parameters
        .iter()
        .map(|parameter| lower_i32_parameter(parameter, &function.name))
        .collect()
}

fn lower_i32_parameter(
    parameter: &Parameter,
    function_name: &str,
) -> Result<String, Vec<Diagnostic>> {
    match &parameter.ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(parameter.name.clone()),
        _ => Err(vec![Diagnostic::error(
            "E8007",
            format!("IR v0 can only lower i32 parameters for function `{function_name}`"),
        )]),
    }
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
    context: &I32ExpressionContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match function.body.statements.as_slice() {
        [Stmt::Return(statement)] => match (return_type, &statement.expression) {
            (Type::I32, Some(expression)) => lower_i32_return_expression(expression, context),
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

const MAX_I32_PARAMETERS: usize = 8;

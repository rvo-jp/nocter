use super::bindings::lower_let_binding;
use super::context::{FunctionSignatures, LoweringContext};
use super::control_flow::{lower_terminal_bool_if_statement, lower_terminal_i32_if_statement};
use super::expressions::{lower_bool_return_expression, lower_i32_return_expression};
use crate::ast::{FunctionDecl, Parameter, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{CallTarget, Function, Instruction, Type};
use crate::resolve::ResolveOutput;
use crate::source::SourceId;

pub(super) fn lower_function(
    function: &FunctionDecl,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<Function, Vec<Diagnostic>> {
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
    let mut context = LoweringContext::new(
        function.name.clone(),
        return_type.clone(),
        function_signatures,
        parameters,
    )
    .with_call_resolution(root_source, resolved);
    let instructions = lower_function_body(function, &return_type, &mut context)?;

    Ok(Function {
        name: function.name.clone(),
        target,
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
        TypeExpr::Reference(reference) if reference.name == "bool" => Ok(Type::Bool),
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(Type::Void),
        _ => Err(vec![Diagnostic::error(
            "E8007",
            format!("IR v0 can only lower function `{name}` return type `i32`, `bool`, or `void`"),
        )]),
    }
}

fn lower_function_body(
    function: &FunctionDecl,
    return_type: &Type,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let statements = function.body.statements.as_slice();

    if statements.is_empty() && *return_type == Type::Void {
        return Ok(vec![Instruction::Return]);
    }

    let Some((last, leading)) = statements.split_last() else {
        return Err(unsupported_function_body_diagnostic(&function.name));
    };

    let mut instructions = lower_leading_bindings(leading, context)?;

    match last {
        Stmt::Return(statement) => {
            let return_instructions = match (return_type, &statement.expression) {
                (Type::I32, Some(expression)) => lower_i32_return_expression(expression, context),
                (Type::Bool, Some(expression)) => {
                    lower_bool_return_expression(expression, context, "E8007")
                }
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
                (Type::Bool, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from bool function `{}`",
                        function.name
                    ),
                )]),
                (Type::Fallible(_), _) => {
                    unreachable!("fallible function type is not lowered in v0")
                }
            }?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        Stmt::If(statement) if return_type == &Type::I32 => {
            instructions.extend(lower_terminal_i32_if_statement(
                statement,
                context,
                "E8007",
                "functions",
            )?);
            Ok(instructions)
        }
        Stmt::If(statement) if return_type == &Type::Bool => {
            instructions.extend(lower_terminal_bool_if_statement(
                statement,
                context,
                "E8007",
                "functions",
            )?);
            Ok(instructions)
        }
        _ => Err(unsupported_function_body_diagnostic(&function.name)),
    }
}

fn lower_leading_bindings(
    statements: &[Stmt],
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        let Stmt::Binding(statement) = statement else {
            return Err(vec![Diagnostic::error(
                "E8007",
                "IR v0 can only lower leading scalar `let` bindings before `return`",
            )]);
        };

        instructions.extend(lower_let_binding(statement, context)?);
    }

    Ok(instructions)
}

fn unsupported_function_body_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower function `{function_name}` bodies containing leading scalar `let` bindings followed by `return`"
        ),
    )]
}

const MAX_I32_PARAMETERS: usize = 8;

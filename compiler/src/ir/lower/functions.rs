use super::bindings::lower_let_binding;
use super::context::{FunctionNames, FunctionSignatures, LoweringContext};
use super::control_flow::{lower_terminal_bool_if_statement, lower_terminal_i32_if_statement};
use super::expressions::{
    lower_bool_return_expression, lower_i32_return_expression, lower_never_return_expression,
    lower_usize_return_expression,
};
use crate::ast::{FunctionDecl, Parameter, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{CallTarget, Function, Instruction, Type};
use crate::resolve::ResolveOutput;
use crate::source::SourceId;

pub(super) fn lower_function(
    function: &FunctionDecl,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
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

    let parameters = lower_scalar_parameters(function)?;
    let return_type = lower_function_return_type(&function.return_type, &function.name)?;
    let mut context = LoweringContext::new(
        function.name.clone(),
        return_type.clone(),
        function_signatures,
        parameters.i32,
        parameters.usize,
        parameters.bool,
    )
    .with_call_resolution(root_source, resolved, function_names);
    let instructions = lower_function_body(function, &return_type, &mut context)?;

    Ok(Function {
        name: function.name.clone(),
        target,
        return_type,
        instructions,
    })
}

struct LoweredScalarParameters {
    i32: Vec<Option<String>>,
    usize: Vec<Option<String>>,
    bool: Vec<Option<String>>,
}

fn lower_scalar_parameters(
    function: &FunctionDecl,
) -> Result<LoweredScalarParameters, Vec<Diagnostic>> {
    if function.parameters.parameters.len() > MAX_SCALAR_PARAMETERS {
        return Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower up to {MAX_SCALAR_PARAMETERS} scalar parameters for function `{}`",
                function.name
            ),
        )]);
    }

    let mut i32_parameters = Vec::with_capacity(function.parameters.parameters.len());
    let mut usize_parameters = Vec::with_capacity(function.parameters.parameters.len());
    let mut bool_parameters = Vec::with_capacity(function.parameters.parameters.len());
    for parameter in &function.parameters.parameters {
        match lower_scalar_parameter_kind(parameter, &function.name)? {
            ScalarParameterKind::I32 => {
                i32_parameters.push(Some(parameter.name.clone()));
                usize_parameters.push(None);
                bool_parameters.push(None);
            }
            ScalarParameterKind::Usize => {
                i32_parameters.push(None);
                usize_parameters.push(Some(parameter.name.clone()));
                bool_parameters.push(None);
            }
            ScalarParameterKind::Bool => {
                i32_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(Some(parameter.name.clone()));
            }
        }
    }

    Ok(LoweredScalarParameters {
        i32: i32_parameters,
        usize: usize_parameters,
        bool: bool_parameters,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarParameterKind {
    I32,
    Usize,
    Bool,
}

fn lower_scalar_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    match &parameter.ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(ScalarParameterKind::I32),
        TypeExpr::Reference(reference) if reference.name == "usize" => {
            Ok(ScalarParameterKind::Usize)
        }
        TypeExpr::Reference(reference) if reference.name == "bool" => Ok(ScalarParameterKind::Bool),
        _ => Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower `i32`, `usize`, and `bool` parameters for function `{function_name}`"
            ),
        )]),
    }
}

fn lower_function_return_type(ty: &TypeExpr, name: &str) -> Result<Type, Vec<Diagnostic>> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "usize" => Ok(Type::Usize),
        TypeExpr::Reference(reference) if reference.name == "bool" => Ok(Type::Bool),
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(Type::Void),
        TypeExpr::Reference(reference) if reference.name == "never" => Ok(Type::Never),
        _ => Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower function `{name}` return type `i32`, `usize`, `bool`, `void`, or `never`"
            ),
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
            if let Some(expression) = &statement.expression
                && let Some(return_instructions) =
                    lower_never_return_expression(expression, context)?
            {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }

            let return_instructions = match (return_type, &statement.expression) {
                (Type::I32, Some(expression)) => lower_i32_return_expression(expression, context),
                (Type::Usize, Some(expression)) => {
                    lower_usize_return_expression(expression, context)
                }
                (Type::Bool, Some(expression)) => {
                    lower_bool_return_expression(expression, context, "E8007")
                }
                (Type::Never, Some(_)) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 can only lower never function `{}` returns from `never` calls",
                        function.name
                    ),
                )]),
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
                (Type::Usize, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from usize function `{}`",
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
                (Type::Never, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from never function `{}`",
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
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_return_expression(&statement.expression, context)?
            else {
                return Err(unsupported_function_body_diagnostic(&function.name));
            };
            instructions.extend(terminating_instructions);
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

const MAX_SCALAR_PARAMETERS: usize = 8;

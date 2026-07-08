use super::bindings::lower_let_binding;
use super::context::{FunctionSignatures, LoweringContext};
use super::control_flow::lower_terminal_i32_if_statement;
use super::errors::{lower_make_error_message, with_trailing_newline};
use super::expressions::lower_i32_return_expression;
use crate::ast::{Expr, FunctionDecl, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{Function, I32Location, I32Value, Instruction, Type};

pub(super) fn lower_entry_function(
    function: &FunctionDecl,
    function_signatures: FunctionSignatures,
) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() || !function.parameters.parameters.is_empty() {
        return Err(vec![Diagnostic::error(
            "E8001",
            "IR v0 can only lower a non-generic zero-parameter entry function",
        )]);
    }

    let return_type = lower_entry_return_type(&function.return_type)?;
    let instructions = lower_entry_body(function, &return_type, function_signatures)?;

    Ok(Function {
        name: function.name.clone(),
        return_type,
        instructions,
    })
}

fn lower_entry_return_type(ty: &TypeExpr) -> Result<Type, Vec<Diagnostic>> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(Type::Void),
        TypeExpr::Fallible(fallible) => lower_entry_return_type(&fallible.success)
            .map(|success| Type::Fallible(Box::new(success))),
        _ => Err(vec![Diagnostic::error(
            "E8001",
            "IR v0 can only lower entry function return type `i32`, `i32!`, or `void`",
        )]),
    }
}

fn lower_entry_body(
    function: &FunctionDecl,
    return_type: &Type,
    function_signatures: FunctionSignatures,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();
    let statements = function.body.statements.as_slice();

    if statements.is_empty() && *return_type == Type::Void {
        return Ok(vec![Instruction::Return]);
    }

    let Some((last, leading)) = statements.split_last() else {
        return Err(unsupported_entry_body_diagnostic());
    };

    let mut context = LoweringContext::empty(
        function.name.clone(),
        success_type.clone(),
        function_signatures,
    );
    let mut instructions = lower_leading_bindings(leading, &mut context)?;

    match last {
        Stmt::Return(statement) => {
            if leading.is_empty()
                && let Some(expression) = &statement.expression
                && return_type_is_lowerable_i32_fallible(return_type)
                && expression_is_make_error_call(expression)
            {
                return lower_i32_fallible_failure(expression);
            }

            let return_instructions = match (success_type, &statement.expression) {
                (Type::I32, Some(expression)) => lower_i32_return_expression(expression, &context),
                (Type::Void, None) => Ok(vec![Instruction::Return]),
                (Type::Void, Some(_)) => Err(vec![Diagnostic::error(
                    "E8002",
                    "IR v0 cannot lower value returns from `void` entry function",
                )]),
                (Type::I32, None) => Err(vec![Diagnostic::error(
                    "E8002",
                    "IR v0 cannot lower bare returns from `i32` entry function",
                )]),
                (Type::Bool, _) => unreachable!("bool entry type is not lowered in v0"),
                (Type::Fallible(_), _) => unreachable!("fallible success type must be unwrapped"),
            }?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::I32 => {
            instructions.extend(lower_terminal_i32_if_statement(
                statement,
                &context,
                "E8002",
                "entry functions",
            )?);
            Ok(instructions)
        }
        _ => Err(unsupported_entry_body_diagnostic()),
    }
}

fn return_type_is_lowerable_i32_fallible(return_type: &Type) -> bool {
    matches!(return_type, Type::Fallible(success) if success.as_ref() == &Type::I32)
}

fn expression_is_make_error_call(expression: &Expr) -> bool {
    let Expr::Call(call) = expression else {
        return false;
    };
    matches!(call.callee.as_ref(), Expr::Identifier(identifier) if identifier.name == "make_error")
}

fn lower_i32_fallible_failure(expression: &Expr) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let message = lower_make_error_message(expression)?;
    Ok(vec![
        Instruction::WriteStaticStderr(with_trailing_newline(message)),
        Instruction::SetI32 {
            destination: I32Location::Return,
            value: I32Value::Const(1),
        },
        Instruction::Return,
    ])
}

fn lower_leading_bindings(
    statements: &[Stmt],
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        let Stmt::Binding(statement) = statement else {
            return Err(unsupported_entry_body_diagnostic());
        };

        instructions.extend(lower_let_binding(statement, context)?);
    }

    Ok(instructions)
}

fn unsupported_entry_body_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8002",
        "IR v0 can only lower entry function bodies containing leading scalar `let` bindings followed by `return`, `return make_error(...)`, or a void return",
    )]
}

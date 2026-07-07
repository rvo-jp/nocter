use super::errors::{lower_make_error_message, with_trailing_newline};
use super::literals::lower_i32_literal;
use crate::ast::{ProgramDecl, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{Function, Instruction, Type};

pub(super) fn lower_program_function(program: &ProgramDecl) -> Result<Function, Vec<Diagnostic>> {
    let return_type = lower_program_return_type(&program.return_type)?;
    let instructions = lower_program_body(program, &return_type)?;

    Ok(Function {
        name: "program".to_string(),
        return_type,
        instructions,
    })
}

fn lower_program_return_type(ty: &TypeExpr) -> Result<Type, Vec<Diagnostic>> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(Type::Void),
        TypeExpr::Fallible(fallible) => lower_program_return_type(&fallible.success)
            .map(|success| Type::Fallible(Box::new(success))),
        _ => Err(vec![Diagnostic::error(
            "E8001",
            "IR v0 can only lower `program` return type `i32`, `i32!`, or `void`",
        )]),
    }
}

fn lower_program_body(
    program: &ProgramDecl,
    return_type: &Type,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();

    match program.body.statements.as_slice() {
        [Stmt::Return(statement)] => match (success_type, &statement.expression) {
            (Type::I32, Some(expression)) => {
                let value = lower_i32_literal(expression)?;
                Ok(vec![Instruction::ReturnI32(value)])
            }
            (Type::Void, None) => Ok(vec![Instruction::ReturnVoid]),
            (Type::Void, Some(_)) => Err(vec![Diagnostic::error(
                "E8002",
                "IR v0 cannot lower value returns from `void` program",
            )]),
            (Type::I32, None) => Err(vec![Diagnostic::error(
                "E8002",
                "IR v0 cannot lower bare returns from `i32` program",
            )]),
            (Type::Fallible(_), _) => unreachable!("fallible success type must be unwrapped"),
        },
        [Stmt::Fail(statement)] => match return_type {
            Type::Fallible(success) if success.as_ref() == &Type::I32 => {
                let message = lower_make_error_message(&statement.expression)?;
                Ok(vec![
                    Instruction::WriteStaticStderr(with_trailing_newline(message)),
                    Instruction::ReturnI32(1),
                ])
            }
            Type::Fallible(_) => Err(vec![Diagnostic::error(
                "E8004",
                "IR v0 can only lower `fail make_error(...)` from `program(): i32!`",
            )]),
            Type::I32 | Type::Void => Err(vec![Diagnostic::error(
                "E8004",
                "IR v0 cannot lower `fail` from non-fallible `program`",
            )]),
        },
        [] if *return_type == Type::Void => Ok(vec![Instruction::ReturnVoid]),
        _ => Err(vec![Diagnostic::error(
            "E8002",
            "IR v0 can only lower `program` bodies containing `return <i32 literal>`, `fail make_error(...)`, or a void return",
        )]),
    }
}

use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{FunctionNames, FunctionSignatures, LoweringContext};
use super::control_flow::lower_terminal_i32_if_statement;
use super::errors::{ErrorPayload, lower_error_payload};
use super::expressions::{
    lower_i32_return_expression, lower_never_return_expression, lower_void_expression_statement,
    mark_fallible_success_returns, success_return_instruction,
};
use super::functions::{
    append_scope_end_drops_before_return, lower_drop_statement, mark_explicit_moves_in_expression,
    mark_lowered_statement_aggregate_uses,
};
use crate::ast::{FunctionDecl, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{CallTarget, Function, Instruction, Type};
use crate::resolve::ResolveOutput;
use crate::source::SourceId;

pub(super) fn lower_entry_function(
    function: &FunctionDecl,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() || !function.parameters.parameters.is_empty() {
        return Err(vec![Diagnostic::error(
            "E8001",
            "IR v0 can only lower a non-generic zero-parameter entry function",
        )]);
    }

    let return_type = lower_entry_return_type(&function.return_type)?;
    let instructions = lower_entry_body(
        function,
        &return_type,
        function_signatures,
        function_names,
        root_source,
        resolved,
    )?;

    Ok(Function {
        name: function.name.clone(),
        target: CallTarget::same_file(function.name.clone()),
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
            "IR v0 can only lower entry function return type `i32`, `i32!`, `void`, or `void!`",
        )]),
    }
}

fn lower_entry_body(
    function: &FunctionDecl,
    return_type: &Type,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();
    let statements = function.body.statements.as_slice();

    if statements.is_empty() && *success_type == Type::Void {
        return Ok(vec![success_return_instruction(return_type)]);
    }

    let Some((last, leading)) = statements.split_last() else {
        return Err(unsupported_entry_body_diagnostic());
    };

    let mut context = LoweringContext::empty(
        function.name.clone(),
        success_type.clone(),
        function_signatures,
    )
    .with_function_return_type(return_type.clone())
    .with_call_resolution(root_source, resolved, function_names);
    let mut instructions = lower_leading_bindings(leading, &mut context)?;

    match last {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression
                && let Some(return_instructions) =
                    lower_never_return_expression(expression, &context)?
            {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && matches!(return_type, Type::Fallible(_))
                && let Some(payload) =
                    lower_error_payload(expression, resolved, root_source, Some(&context))?
            {
                instructions.extend(append_scope_end_drops_before_return(
                    lower_fallible_failure(payload),
                    &mut context,
                )?);
                return Ok(instructions);
            }

            let return_instructions = match (success_type, &statement.expression) {
                (Type::I32, Some(expression)) => lower_i32_return_expression(expression, &context),
                (Type::U8, _) => unreachable!("u8 entry type is not lowered in v0"),
                (Type::Usize, _) => unreachable!("usize entry type is not lowered in v0"),
                (Type::Str, _) => unreachable!("str entry type is not lowered in v0"),
                (Type::Slice { .. }, _) => unreachable!("slice entry type is not lowered in v0"),
                (Type::Aggregate { .. }, _) => {
                    unreachable!("aggregate entry type is not lowered in v0")
                }
                (Type::DirectAggregate { .. }, _) => {
                    unreachable!("direct aggregate entry type is not lowered in v0")
                }
                (Type::Void, None) => Ok(vec![Instruction::Return]),
                (Type::Void, Some(_)) => Err(vec![Diagnostic::error(
                    "E8002",
                    "IR v0 cannot lower value returns from `void` entry function",
                )]),
                (Type::I32, None) => Err(vec![Diagnostic::error(
                    "E8002",
                    "IR v0 cannot lower bare returns from `i32` entry function",
                )]),
                (Type::Borrow { .. }, _) => unreachable!("borrow entry type is not lowered in v0"),
                (Type::Bool, _) => unreachable!("bool entry type is not lowered in v0"),
                (Type::Never, _) => unreachable!("never entry type is not lowered in v0"),
                (Type::Fallible(_), _) => unreachable!("fallible success type must be unwrapped"),
            }?;
            if let Some(expression) = &statement.expression {
                mark_explicit_moves_in_expression(expression, &mut context);
            }
            let return_instructions =
                mark_fallible_success_returns(return_type, return_instructions);
            instructions.extend(append_scope_end_drops_before_return(
                return_instructions,
                &mut context,
            )?);
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::I32 => {
            let branch_instructions =
                lower_terminal_i32_if_statement(statement, &context, "E8002", "entry functions")?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_return_expression(&statement.expression, &context)?
            else {
                if success_type == &Type::Void
                    && let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, &context)?
                {
                    instructions.extend(void_instructions);
                    mark_explicit_moves_in_expression(&statement.expression, &mut context);
                    instructions.extend(append_scope_end_drops_before_return(
                        vec![success_return_instruction(return_type)],
                        &mut context,
                    )?);
                    return Ok(instructions);
                }

                return Err(unsupported_entry_body_diagnostic());
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_entry_body_diagnostic()),
    }
}

fn lower_fallible_failure(payload: ErrorPayload) -> Vec<Instruction> {
    let (code, message) = payload.into_str_values();
    vec![Instruction::ReturnFallibleFailure { code, message }]
}

fn lower_leading_bindings(
    statements: &[Stmt],
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context)?);
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context)?);
            }
            Stmt::Expression(statement) => {
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_entry_body_diagnostic());
                };
                instructions.extend(void_instructions);
            }
            Stmt::Drop(statement) => {
                instructions.extend(lower_drop_statement(statement, context)?);
            }
            _ => return Err(unsupported_entry_body_diagnostic()),
        };
        mark_lowered_statement_aggregate_uses(statement, context);
    }

    Ok(instructions)
}

fn unsupported_entry_body_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8002",
        "IR v0 can only lower entry function bodies containing leading scalar local bindings, scalar assignments, or void call statements followed by `return`, a static error constructor failure return, or a void return",
    )]
}

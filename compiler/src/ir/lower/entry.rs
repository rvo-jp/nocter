use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{FunctionNames, FunctionSignatures, LoweringContext};
use super::control_flow::{
    lower_nonterminal_if_statement, lower_terminal_i32_if_statement,
    lower_terminal_void_if_statement,
};
use super::errors::{ErrorPayload, lower_error_payload};
use super::expressions::{
    lower_i32_return_expression, lower_never_return_expression, lower_void_expression_statement,
    mark_fallible_success_returns, success_return_instruction,
};
use super::functions::{
    append_scope_end_drops_before_exit, lower_drop_statement, lower_value_return_with_scope_drops,
    mark_explicit_moves_in_expression, mark_lowered_statement_aggregate_uses,
};
use crate::ast::{FunctionDecl, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{CallTarget, Function, Instruction, Type};
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceId, SourceMap};

pub(super) fn lower_entry_function(
    function: &FunctionDecl,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() || !function.parameters.parameters.is_empty() {
        let span = function.generics.span.unwrap_or(function.parameters.span);
        return Err(attach_primary_span_if_absent(
            vec![Diagnostic::error(
                "E8001",
                "IR v0 can only lower a non-generic zero-parameter entry function",
            )],
            sources,
            span,
        ));
    }

    let return_type = lower_entry_return_type(&function.return_type).map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, function.return_type.span())
    })?;
    let instructions = lower_entry_body(
        function,
        &return_type,
        sources,
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
    sources: &SourceMap,
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
        return Err(attach_primary_span_if_absent(
            unsupported_entry_body_diagnostic(),
            sources,
            function.body.span,
        ));
    };

    let mut context = LoweringContext::empty(
        function.name.clone(),
        success_type.clone(),
        function_signatures,
    )
    .with_function_return_type(return_type.clone())
    .with_call_resolution(root_source, resolved, function_names);
    let mut instructions =
        lower_leading_bindings(leading, &mut context).map_err(|diagnostics| {
            let span = leading
                .first()
                .map_or(function.body.span, |statement| statement.span());
            attach_primary_span_if_absent(diagnostics, sources, span)
        })?;

    match last {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression
                && let Some(return_instructions) =
                    lower_never_return_expression(expression, &context).map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, expression.span())
                    })?
            {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && matches!(return_type, Type::Fallible(_))
                && let Some(payload) =
                    lower_error_payload(expression, resolved, root_source, Some(&context)).map_err(
                        |diagnostics| {
                            attach_primary_span_if_absent(diagnostics, sources, expression.span())
                        },
                    )?
            {
                instructions.extend(append_scope_end_drops_before_exit(
                    lower_fallible_failure(payload),
                    &mut context,
                )?);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && let Some(return_instructions) = lower_value_return_with_scope_drops(
                    success_type,
                    expression,
                    return_type,
                    &mut context,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, expression.span())
                })?
            {
                instructions.extend(return_instructions);
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
            }
            .map_err(|diagnostics| {
                let span = statement
                    .expression
                    .as_ref()
                    .map_or(statement.span, |expression| expression.span());
                attach_primary_span_if_absent(diagnostics, sources, span)
            })?;
            let return_instructions =
                mark_fallible_success_returns(return_type, return_instructions);
            instructions.extend(append_scope_end_drops_before_exit(
                return_instructions,
                &mut context,
            )?);
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::I32 => {
            let branch_instructions = lower_terminal_i32_if_statement(
                statement,
                &context,
                return_type,
                "E8002",
                "entry functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::Void => {
            let branch_instructions = lower_terminal_void_if_statement(
                statement,
                &context,
                return_type,
                "E8002",
                "entry functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_return_expression(
                &statement.expression,
                &context,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.expression.span())
            })?
            else {
                if success_type == &Type::Void
                    && let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, &context).map_err(
                            |diagnostics| {
                                attach_primary_span_if_absent(
                                    diagnostics,
                                    sources,
                                    statement.expression.span(),
                                )
                            },
                        )?
                {
                    instructions.extend(void_instructions);
                    mark_explicit_moves_in_expression(&statement.expression, &mut context);
                    instructions.extend(append_scope_end_drops_before_exit(
                        vec![success_return_instruction(return_type)],
                        &mut context,
                    )?);
                    return Ok(instructions);
                }

                return Err(attach_primary_span_if_absent(
                    unsupported_entry_body_diagnostic(),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(attach_primary_span_if_absent(
            unsupported_entry_body_diagnostic(),
            sources,
            last.span(),
        )),
    }
}

fn attach_primary_span_if_absent(
    diagnostics: Vec<Diagnostic>,
    sources: &SourceMap,
    span: ByteSpan,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.with_primary_span_if_absent(sources, span))
        .collect()
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
            Stmt::If(statement) => {
                instructions.extend(lower_nonterminal_if_statement(
                    statement,
                    context,
                    "E8002",
                    "entry functions",
                )?);
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
        "IR v0 can only lower entry function bodies containing leading scalar local bindings, scalar assignments, drop statements, void call statements, or supported non-terminal `if` statements followed by `return`, a static error constructor failure return, or a void return",
    )]
}

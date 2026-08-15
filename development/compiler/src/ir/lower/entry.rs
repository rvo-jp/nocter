use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{
    ErrorPayloads, FunctionNames, FunctionSignatures, LoweringContext, ResolvedSources,
};
use super::control_flow::{
    lower_nonterminal_for_range_statement, lower_nonterminal_if_statement,
    lower_nonterminal_if_statement_with_branch_prologues, lower_nonterminal_loop_statement,
    lower_nonterminal_payloadless_switch_body, lower_nonterminal_payloadless_switch_statement,
    lower_nonterminal_region_statement, lower_nonterminal_while_statement,
    lower_terminal_condition, lower_terminal_i32_if_statement_with_branch_prologues,
    lower_terminal_i32_switch_block, lower_terminal_usize_if_statement_with_branch_prologues,
    lower_terminal_usize_switch_block, lower_terminal_void_if_statement_with_branch_prologues,
    lower_terminal_void_switch_block,
};
use super::expressions::{
    lower_void_expression_statement, mark_outcome_success_returns, success_return_instruction,
};
use super::functions::{
    BranchPrologue, LoweredPayloadlessSwitch, LoweredPayloadlessSwitchBody, LoweredSwitchBlock,
    LoweredSwitchCondition, append_scope_end_drops_before_exit, lower_drop_statement,
    lower_never_expression, lower_return_statement_with_scope_drops,
    mark_explicit_moves_in_expression, mark_lowered_statement_aggregate_uses,
    reachable_body_prefix, tag_only_if_is_as_control_flow, tag_only_switch_as_control_flow,
};
use super::types::{return_type_expr_has_optional_layer, return_type_from_type_expr};
use crate::ast::{Block, Expr, FunctionDecl, IfStmt, ReturnStmt, Stmt, TestDecl, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{CallTarget, Function, Instruction, Type};
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::TypedHir;

#[cfg(test)]
pub(super) fn lower_entry_function(
    function: &FunctionDecl,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    resolved_sources: ResolvedSources<'_>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    lower_entry_function_with_target(
        function,
        CallTarget::same_file(function.name.clone()),
        sources,
        function_signatures,
        function_names,
        root_source,
        resolved,
        typed_hir,
        &crate::mir::BodyCache::default(),
        resolved_sources,
        error_payloads,
    )
}

pub(super) fn lower_entry_function_with_target(
    function: &FunctionDecl,
    target: CallTarget,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    mir_bodies: &crate::mir::BodyCache,
    resolved_sources: ResolvedSources<'_>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() || !function.parameters.parameters.is_empty() {
        let span = function.generics.span.unwrap_or(function.parameters.span);
        return Err(attach_primary_span_if_absent(
            vec![Diagnostic::error(
                "E8001",
                "native lowering can only lower a non-generic zero-parameter entry function",
            )],
            sources,
            span,
        ));
    }

    lower_entry_parts(
        &function.name,
        function.span,
        &function.return_type,
        function.body.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "E8006",
                "native lowering cannot use a bodyless function as an entry point",
            )]
        })?,
        target,
        sources,
        function_signatures,
        function_names,
        root_source,
        resolved,
        typed_hir,
        mir_bodies,
        resolved_sources,
        error_payloads,
    )
}

pub(super) fn lower_test_entry_function(
    test: &TestDecl,
    target: CallTarget,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    mir_bodies: &crate::mir::BodyCache,
    resolved_sources: ResolvedSources<'_>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    let return_type = test.return_type();
    lower_entry_parts(
        &test.name,
        test.span,
        &return_type,
        &test.body,
        target,
        sources,
        function_signatures,
        function_names,
        root_source,
        resolved,
        typed_hir,
        mir_bodies,
        resolved_sources,
        error_payloads,
    )
}

fn lower_entry_parts(
    name: &str,
    declaration_span: ByteSpan,
    return_type_expr: &TypeExpr,
    body: &Block,
    target: CallTarget,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    mir_bodies: &crate::mir::BodyCache,
    resolved_sources: ResolvedSources<'_>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    let return_type =
        lower_entry_return_type(return_type_expr, resolved).map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, return_type_expr.span())
        })?;
    let parameter_slots = super::context::LoweringParameterSlots::default();
    if let Some(instructions) = super::mir::try_lower_body(
        mir_bodies,
        body,
        &[],
        return_type_expr,
        &return_type,
        resolved,
        &resolved_sources,
        typed_hir,
        &std::collections::HashMap::new(),
        name,
        &function_signatures,
        &function_names,
        &error_payloads,
        &parameter_slots,
        root_source,
        sources,
    ) {
        return Ok(Function {
            name: name.to_string(),
            target,
            return_type,
            instructions: instructions?,
        });
    }
    let instructions = lower_entry_body(
        name,
        declaration_span,
        return_type_expr,
        body,
        &return_type,
        sources,
        function_signatures,
        function_names,
        root_source,
        resolved,
        typed_hir,
        resolved_sources,
        error_payloads,
    )?;

    Ok(Function {
        name: name.to_string(),
        target,
        return_type,
        instructions,
    })
}

fn lower_entry_return_type(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Result<Type, Vec<Diagnostic>> {
    let Some(return_type) = return_type_from_type_expr(ty, resolved) else {
        return Err(unsupported_entry_return_type_diagnostic());
    };
    if entry_return_type_is_supported(&return_type) {
        Ok(return_type)
    } else {
        Err(unsupported_entry_return_type_diagnostic())
    }
}

fn entry_return_type_is_supported(ty: &Type) -> bool {
    match ty {
        Type::I32 | Type::Usize | Type::Void => true,
        Type::Fallible(success) => entry_return_type_is_supported(success),
        _ => false,
    }
}

fn unsupported_entry_return_type_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8001",
        "native lowering can only lower entry function return type `i32`, `usize`, `i32!`, `usize!`, `void`, or `void!`",
    )]
}

fn lower_entry_body(
    name: &str,
    declaration_span: ByteSpan,
    return_type_expr: &TypeExpr,
    body: &Block,
    return_type: &Type,
    sources: &SourceMap,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
    resolved_sources: ResolvedSources<'_>,
    error_payloads: ErrorPayloads,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();
    let original_statements = body.statements.as_slice();

    if original_statements.iter().all(statement_is_import)
        && body.result.is_none()
        && *success_type == Type::Void
    {
        return Ok(vec![success_return_instruction(return_type)]);
    }

    let mut context =
        LoweringContext::empty(name.to_string(), success_type.clone(), function_signatures)
            .with_function_return_type(return_type.clone())
            .with_function_returns_optional(return_type_expr_has_optional_layer(
                return_type_expr,
                resolved,
            ))
            .with_call_resolution(
                root_source,
                resolved,
                typed_hir,
                function_names,
                resolved_sources,
            )
            .with_error_payloads(error_payloads);

    let (statements, body_result) =
        reachable_body_prefix(original_statements, body.result.as_deref(), &context);

    if let Some(result) = body_result {
        let mut instructions = lower_leading_bindings(statements, &mut context, sources)?;
        instructions.extend(lower_entry_body_result(
            result,
            return_type,
            &mut context,
            sources,
        )?);
        return Ok(instructions);
    }

    if success_type == &Type::Void
        && statements
            .iter()
            .rev()
            .find(|statement| !statement_is_import(statement))
            .is_some_and(statement_allows_implicit_void_return)
    {
        let mut instructions = lower_leading_bindings(statements, &mut context, sources)?;
        instructions.extend(append_scope_end_drops_before_exit(
            vec![success_return_instruction(return_type)],
            &mut context,
        )?);
        return Ok(instructions);
    }

    let Some((last, leading)) = statements.split_last() else {
        return Err(attach_primary_span_if_absent(
            unsupported_entry_body_diagnostic(),
            sources,
            declaration_span,
        ));
    };

    let mut instructions = lower_leading_bindings(leading, &mut context, sources)?;

    match last {
        Stmt::Return(statement) => {
            let return_instructions = lower_entry_return_statement_with_scope_drops(
                statement,
                return_type,
                &mut context,
                sources,
            )
            .map_err(|diagnostics| {
                let span = statement
                    .expression
                    .as_ref()
                    .map_or(statement.span, |expression| expression.span());
                attach_primary_span_if_absent(diagnostics, sources, span)
            })?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        Stmt::If(statement) => {
            let Some(branch_instructions) = lower_terminal_entry_if_statement_for_success_type(
                statement,
                &context,
                return_type,
                sources,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_entry_body_diagnostic(),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, &mut context, "E8002").map_err(
                |diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                },
            )?;
            let Some(branch_instructions) =
                lower_terminal_entry_if_statement_for_success_type_with_branch_prologues(
                    &if_is.statement,
                    &context,
                    &if_is.then_prologue,
                    &BranchPrologue::empty(),
                    return_type,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_entry_body_diagnostic(),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(if_is.leading_instructions);
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::Switch(statement) => {
            let switch = tag_only_switch_as_control_flow(statement, &mut context, "E8002")
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?;
            let Some(branch_instructions) =
                lower_terminal_entry_payloadless_switch_for_success_type(
                    switch,
                    &context,
                    return_type,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_entry_body_diagnostic(),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_expression(
                &statement.expression,
                &mut context,
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
        Stmt::Loop(statement) => {
            instructions.extend(
                lower_nonterminal_loop_statement(
                    statement,
                    &mut context,
                    "E8002",
                    "entry functions",
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            );
            Ok(instructions)
        }
        Stmt::Region(statement) => {
            instructions.extend(
                lower_nonterminal_region_statement(
                    statement,
                    &context,
                    None,
                    &[],
                    "E8002",
                    "entry functions",
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            );
            Ok(instructions)
        }
        _ => Err(attach_primary_span_if_absent(
            unsupported_entry_body_diagnostic(),
            sources,
            last.span(),
        )),
    }
}

fn lower_entry_body_result(
    expression: &Expr,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) =
        lower_entry_control_body_result(expression, return_type, context, sources)?
    {
        return Ok(instructions);
    }

    if return_type.success_type() == &Type::Void {
        if let Some(terminating_instructions) = lower_never_expression(expression, context)
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, expression.span())
            })?
        {
            return Ok(terminating_instructions);
        }

        if let Some(mut void_instructions) = lower_void_expression_statement(expression, context)
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, expression.span())
            })?
        {
            mark_explicit_moves_in_expression(expression, context);
            void_instructions.extend(append_scope_end_drops_before_exit(
                vec![success_return_instruction(return_type)],
                context,
            )?);
            return Ok(void_instructions);
        }
    }

    let statement = ReturnStmt {
        span: expression.span(),
        expression: Some(expression.clone()),
    };
    lower_return_statement_with_scope_drops(&statement, context, "E8002").map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, expression.span())
    })
}

fn lower_entry_return_statement_with_scope_drops(
    statement: &ReturnStmt,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(expression) = &statement.expression
        && let Some(instructions) =
            lower_entry_control_body_result(expression, return_type, context, sources)?
    {
        return Ok(instructions);
    }

    lower_return_statement_with_scope_drops(statement, context, "E8002")
}

fn lower_entry_control_body_result(
    expression: &Expr,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_entry_if_body_result(statement, return_type, context, sources),
        Expr::IfIs(statement) => {
            let mut control_context = context.clone();
            let if_is = tag_only_if_is_as_control_flow(statement, &mut control_context, "E8002")?;
            lower_entry_if_body_result_with_branch_prologues(
                &if_is.statement,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                return_type,
                &mut control_context,
                sources,
            )
            .map(|result| {
                result.map(|branch_instructions| {
                    let mut instructions = if_is.leading_instructions;
                    instructions.extend(branch_instructions);
                    instructions
                })
            })
        }
        Expr::Match(statement) => {
            let mut control_context = context.clone();
            let switch = tag_only_switch_as_control_flow(statement, &mut control_context, "E8002")?;
            lower_entry_payloadless_switch_body_result(
                switch,
                return_type,
                &mut control_context,
                sources,
            )
        }
        _ => Ok(None),
    }
}

fn lower_entry_payloadless_switch_body_result(
    switch: LoweredPayloadlessSwitch,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match lower_terminal_entry_payloadless_switch_body_for_success_type(
        switch.body.clone(),
        context,
        return_type,
        sources,
    ) {
        Ok(Some(mut branch_instructions)) => {
            let mut instructions = switch.leading_instructions;
            instructions.append(&mut branch_instructions);
            Ok(Some(mark_outcome_success_returns(
                return_type,
                instructions,
            )))
        }
        Ok(None) => Ok(None),
        Err(_) if return_type.success_type() == &Type::Void => Ok(Some(
            lower_void_nonterminal_entry_payloadless_switch_body_result(
                switch,
                return_type,
                context,
                sources,
            )?,
        )),
        Err(diagnostics) => Err(diagnostics),
    }
}

fn lower_void_nonterminal_entry_payloadless_switch_body_result(
    switch: LoweredPayloadlessSwitch,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = switch.leading_instructions;
    instructions.extend(lower_nonterminal_payloadless_switch_body(
        switch.body,
        context,
        None,
        &[],
        "E8002",
        "entry functions",
        sources,
    )?);
    instructions.extend(append_scope_end_drops_before_exit(
        vec![success_return_instruction(return_type)],
        context,
    )?);
    Ok(instructions)
}

fn lower_entry_if_body_result(
    statement: &IfStmt,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    lower_entry_if_body_result_with_branch_prologues(
        statement,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        return_type,
        context,
        sources,
    )
}

fn lower_entry_if_body_result_with_branch_prologues(
    statement: &IfStmt,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match lower_terminal_entry_if_statement_for_success_type_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        return_type,
        sources,
    ) {
        Ok(instructions) => Ok(instructions),
        Err(_) if return_type.success_type() == &Type::Void => Ok(Some(
            lower_void_nonterminal_entry_if_body_result_with_branch_prologues(
                statement,
                then_prologue,
                else_prologue,
                return_type,
                context,
                sources,
            )?,
        )),
        Err(diagnostics) => Err(diagnostics),
    }
}

fn lower_void_nonterminal_entry_if_body_result_with_branch_prologues(
    statement: &IfStmt,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = lower_nonterminal_if_statement_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        None,
        &[],
        "E8002",
        "entry functions",
        sources,
    )?;
    instructions.extend(append_scope_end_drops_before_exit(
        vec![success_return_instruction(return_type)],
        context,
    )?);
    Ok(instructions)
}

fn lower_terminal_entry_if_statement_for_success_type(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    lower_terminal_entry_if_statement_for_success_type_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        return_type,
        sources,
    )
}

fn lower_terminal_entry_if_statement_for_success_type_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(branch_instructions) =
        lower_terminal_entry_if_statement_body_for_success_type_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            sources,
        )?
    else {
        return Ok(None);
    };

    Ok(Some(mark_outcome_success_returns(
        return_type,
        branch_instructions,
    )))
}

fn lower_terminal_entry_if_statement_body_for_success_type_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let branch_instructions = match return_type.success_type() {
        Type::I32 => lower_terminal_i32_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            "E8002",
            "entry functions",
            sources,
        )?,
        Type::Usize => lower_terminal_usize_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            "E8002",
            "entry functions",
            sources,
        )?,
        Type::Void => lower_terminal_void_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            "E8002",
            "entry functions",
            sources,
        )?,
        _ => return Ok(None),
    };

    Ok(Some(branch_instructions))
}

fn lower_terminal_entry_payloadless_switch_for_success_type(
    switch: LoweredPayloadlessSwitch,
    context: &LoweringContext,
    return_type: &Type,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(branch_instructions) = lower_terminal_entry_payloadless_switch_body_for_success_type(
        switch.body,
        context,
        return_type,
        sources,
    )?
    else {
        return Ok(None);
    };

    let mut instructions = switch.leading_instructions;
    instructions.extend(branch_instructions);
    Ok(Some(mark_outcome_success_returns(
        return_type,
        instructions,
    )))
}

fn lower_terminal_entry_payloadless_switch_body_for_success_type(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    return_type: &Type,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => {
            lower_terminal_entry_switch_block_for_success_type(block, context, return_type, sources)
        }
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_entry_switch_condition_for_success_type(
                condition,
                context,
                return_type,
                sources,
            )
        }
    }
}

fn lower_terminal_entry_switch_condition_for_success_type(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    return_type: &Type,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(then_instructions) = lower_terminal_entry_switch_block_for_success_type(
        condition.then_branch,
        context,
        return_type,
        sources,
    )?
    else {
        return Ok(None);
    };
    let Some(else_instructions) = lower_terminal_entry_payloadless_switch_body_for_success_type(
        *condition.else_body,
        context,
        return_type,
        sources,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        "E8002",
        sources,
    )?))
}

fn lower_terminal_entry_switch_block_for_success_type(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    return_type: &Type,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let branch_instructions = match return_type.success_type() {
        Type::I32 => lower_terminal_i32_switch_block(
            block,
            context,
            return_type,
            "E8002",
            "entry functions",
            sources,
        )?,
        Type::Usize => lower_terminal_usize_switch_block(
            block,
            context,
            return_type,
            "E8002",
            "entry functions",
            sources,
        )?,
        Type::Void => lower_terminal_void_switch_block(
            block,
            context,
            return_type,
            "E8002",
            "entry functions",
            sources,
        )?,
        _ => return Ok(None),
    };
    Ok(Some(branch_instructions))
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn statement_allows_implicit_void_return(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::Drop(_)
            | Stmt::ForRange(_)
            | Stmt::While(_)
            | Stmt::Loop(_)
    )
}

fn statement_is_import(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Import(_) | Stmt::FromImport(_))
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

fn lower_leading_bindings(
    statements: &[Stmt],
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Expression(statement) => {
                let Some(void_instructions) = lower_void_expression_statement(
                    &statement.expression,
                    context,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.expression.span())
                })?
                else {
                    return Err(attach_primary_span_if_absent(
                        unsupported_entry_body_diagnostic(),
                        sources,
                        statement.span,
                    ));
                };
                instructions.extend(void_instructions);
            }
            Stmt::Drop(statement) => {
                instructions.extend(lower_drop_statement(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::If(statement) => {
                instructions.extend(
                    lower_nonterminal_if_statement(
                        statement,
                        context,
                        None,
                        &[],
                        "E8002",
                        "entry functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::IfIs(statement) => {
                let if_is = tag_only_if_is_as_control_flow(statement, context, "E8002").map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                    },
                )?;
                let target_cleanup = if_is.target_cleanup;
                instructions.extend(if_is.leading_instructions);
                instructions.extend(
                    lower_nonterminal_if_statement_with_branch_prologues(
                        &if_is.statement,
                        context,
                        &if_is.then_prologue,
                        &BranchPrologue::empty(),
                        None,
                        &[],
                        "E8002",
                        "entry functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
                if let Some(cleanup) = target_cleanup {
                    cleanup.append_to(&mut instructions, context)?;
                }
            }
            Stmt::Switch(statement) => {
                instructions.extend(
                    lower_nonterminal_payloadless_switch_statement(
                        statement,
                        context,
                        None,
                        &[],
                        "E8002",
                        "entry functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::While(statement) => {
                instructions.extend(
                    lower_nonterminal_while_statement(
                        statement,
                        context,
                        "E8002",
                        "entry functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::ForRange(statement) => {
                instructions.extend(
                    lower_nonterminal_for_range_statement(
                        statement,
                        context,
                        "E8002",
                        "entry functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::CollectionFor(statement) => {
                instructions.extend(
                    super::collection_for::lower_collection_for_statement(
                        statement,
                        context,
                        "E8002",
                        "entry functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::Loop(statement) => {
                instructions.extend(
                    lower_nonterminal_loop_statement(
                        statement,
                        context,
                        "E8002",
                        "entry functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::Region(statement) => {
                instructions.extend(
                    lower_nonterminal_region_statement(
                        statement,
                        context,
                        None,
                        &[],
                        "E8002",
                        "entry functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            _ => {
                return Err(attach_primary_span_if_absent(
                    unsupported_entry_body_diagnostic(),
                    sources,
                    statement.span(),
                ));
            }
        };
        mark_lowered_statement_aggregate_uses(statement, context);
    }

    Ok(instructions)
}

fn unsupported_entry_body_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8002",
        "native lowering can only lower entry function bodies containing leading scalar local bindings, scalar assignments, drop statements, effect-only call statements, or supported non-terminal `if`/`for`/`while`/`loop` statements followed by `return`, a static error constructor failure return, or a void return",
    )]
}

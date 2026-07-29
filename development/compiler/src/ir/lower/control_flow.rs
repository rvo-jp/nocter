use super::bindings::{
    LoopControlContext, assignment_targets_direct_slice_index,
    assignment_targets_readwrite_aggregate_field, lower_assignment, lower_local_binding,
    lower_local_binding_with_loop_control,
};
use super::context::LoweringContext;
use super::expressions::{
    lower_bool_expression_to_value, lower_i32_expression_to_location,
    lower_slice_return_expression, lower_str_return_expression, lower_u8_return_expression,
    lower_usize_expression_to_location, lower_usize_return_expression,
    lower_void_expression_statement, primitive_trap_call,
    short_circuit_bool_expression_needs_branch, success_return_instruction,
};
use super::functions::{
    append_scope_end_drops_before_exit, expression_contains_explicit_aggregate_move,
    expression_contains_explicit_aggregate_move_outside, lower_drop_statement,
    lower_never_expression_with_scope_drops, lower_return_statement_with_scope_drops,
    lower_scope_end_drops_for_locals_since, lower_terminal_return_statement_with_scope_drops,
    mark_explicit_moves_in_expression, mark_lowered_statement_aggregate_uses,
    payloadless_if_is_as_if_statement, payloadless_switch_as_if_statement,
    payloadless_switch_is_exhaustive,
};
use crate::ast::{
    AssignmentOperator, BinaryExpr, BinaryOperator, Block, Expr, ForRangeStmt, IfStmt, LoopStmt,
    ReturnStmt, Stmt, UnaryOperator, WhileStmt,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgumentSource, BoolLocation, BoolValue, BorrowSource, FallibleFailureMode,
    I32ComparisonOperator, I32Location, I32Value, Instruction, ScalarArgument, SliceLocation,
    StrLocation, Type, U8Location, UsizeLocation, UsizeValue,
};
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::TypecheckScalarViewKind;
use std::collections::HashSet;

type ReturnLowerer = fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>;

struct LoweredNonterminalBlock {
    instructions: Vec<Instruction>,
    ends_execution: bool,
}

pub(super) enum TerminalBranch<'a> {
    Statement(&'a Stmt),
    Result(&'a Expr),
}

pub(super) fn lower_terminal_i32_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "i32",
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_i32_return_block(
            &statement.then_block,
            context,
            &statement.condition,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        lower_i32_return_block(
            else_block,
            context,
            &statement.condition,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_bool_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "bool",
        ));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_bool_return_block(
            &statement.then_block,
            context,
            &statement.condition,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        lower_bool_return_block(
            else_block,
            context,
            &statement.condition,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_u8_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "u8",
        lower_u8_return_expression,
        sources,
    )
}

pub(super) fn lower_terminal_usize_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "usize",
        lower_usize_return_expression,
        sources,
    )
}

pub(super) fn lower_terminal_str_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        "&str",
        lower_str_return_expression,
        sources,
    )
}

pub(super) fn lower_terminal_slice_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let return_label = match return_type.success_type() {
        Type::Slice { is_readwrite: true } => "&+[T]",
        _ => "&[T]",
    };

    lower_terminal_scalar_if_statement(
        statement,
        context,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_slice_return_expression,
        sources,
    )
}

fn lower_terminal_scalar_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            return_label,
        ));
    };
    let then_instructions = lower_scalar_return_block(
        &statement.then_block,
        context,
        &statement.condition,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )?;
    let else_instructions = lower_scalar_return_block(
        else_block,
        context,
        &statement.condition,
        return_type,
        diagnostic_code,
        subject,
        return_label,
        lower_return_expression,
        sources,
    )?;

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_void_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "void",
        ));
    };

    let then_instructions = lower_void_return_block(
        &statement.then_block,
        context,
        &statement.condition,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = lower_void_return_block(
        else_block,
        context,
        &statement.condition,
        return_type,
        diagnostic_code,
        subject,
        sources,
    )?;

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_terminal_condition(
    condition: &Expr,
    mut then_instructions: Vec<Instruction>,
    mut else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(condition, context)
        && !condition_explicit_moves_are_single_evaluation_call(condition)
    {
        return Err(attach_primary_span_if_absent(
            unsupported_control_flow_condition_move_diagnostic(diagnostic_code),
            sources,
            condition.span(),
        ));
    }

    if let Some(binary) = short_circuit_condition_needs_branch(condition, context) {
        return lower_short_circuit_terminal_condition(
            binary,
            then_instructions,
            else_instructions,
            context,
            diagnostic_code,
            sources,
        );
    }

    let condition = lower_bool_expression_to_value(condition, context, diagnostic_code).map_err(
        |diagnostics| attach_primary_span_if_absent(diagnostics, sources, condition.span()),
    )?;
    let mut instructions = condition.instructions;
    let moved_slots = aggregate_argument_slots_in_instructions(&instructions);
    remove_condition_moved_aggregate_drops(&mut then_instructions, &moved_slots);
    remove_condition_moved_aggregate_drops(&mut else_instructions, &moved_slots);
    instructions.push(Instruction::If {
        condition: condition.value,
        then_instructions,
        else_instructions,
    });
    Ok(instructions)
}

fn condition_explicit_moves_are_single_evaluation_call(condition: &Expr) -> bool {
    match condition {
        Expr::Call(_) => true,
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            condition_explicit_moves_are_single_evaluation_call(&unary.operand)
        }
        Expr::Propagate(propagation) => {
            condition_explicit_moves_are_single_evaluation_call(&propagation.expression)
        }
        Expr::Force(force) => {
            condition_explicit_moves_are_single_evaluation_call(&force.expression)
        }
        Expr::Catch(catch) => {
            condition_explicit_moves_are_single_evaluation_call(&catch.expression)
        }
        Expr::Group(group) => {
            condition_explicit_moves_are_single_evaluation_call(&group.expression)
        }
        _ => false,
    }
}

fn aggregate_argument_slots_in_instructions(instructions: &[Instruction]) -> HashSet<usize> {
    let mut slots = HashSet::new();
    for instruction in instructions {
        match instruction {
            Instruction::CallI32 { arguments, .. }
            | Instruction::CallFallibleI32 { arguments, .. }
            | Instruction::CallU8 { arguments, .. }
            | Instruction::CallFallibleU8 { arguments, .. }
            | Instruction::CallUsize { arguments, .. }
            | Instruction::CallFallibleUsize { arguments, .. }
            | Instruction::CallBool { arguments, .. }
            | Instruction::CallFallibleBool { arguments, .. }
            | Instruction::CallStr { arguments, .. }
            | Instruction::CallFallibleStr { arguments, .. }
            | Instruction::CallSlice { arguments, .. }
            | Instruction::CallFallibleSlice { arguments, .. }
            | Instruction::CallVoid { arguments, .. }
            | Instruction::CallAggregate { arguments, .. }
            | Instruction::CallFallibleAggregate { arguments, .. }
            | Instruction::CallDirectAggregate { arguments, .. }
            | Instruction::CallFallibleDirectAggregate { arguments, .. }
            | Instruction::TailCall { arguments, .. } => {
                collect_aggregate_argument_slots(arguments, &mut slots);
            }
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                slots.extend(aggregate_argument_slots_in_instructions(then_instructions));
                slots.extend(aggregate_argument_slots_in_instructions(else_instructions));
            }
            Instruction::While {
                condition_instructions,
                body_instructions,
                ..
            } => {
                slots.extend(aggregate_argument_slots_in_instructions(
                    condition_instructions,
                ));
                slots.extend(aggregate_argument_slots_in_instructions(body_instructions));
            }
            _ => {}
        }
    }
    slots
}

fn collect_aggregate_argument_slots(arguments: &[ScalarArgument], slots: &mut HashSet<usize>) {
    for argument in arguments {
        let source = match argument {
            ScalarArgument::AggregateIndirect(argument) => &argument.source,
            ScalarArgument::AggregateDirect(argument) => &argument.source,
            _ => continue,
        };
        let AggregateArgumentSource::Slot(slot_index) = source;
        slots.insert(*slot_index);
    }
}

fn remove_condition_moved_aggregate_drops(
    instructions: &mut Vec<Instruction>,
    moved_slots: &HashSet<usize>,
) {
    if moved_slots.is_empty() {
        return;
    }
    for instruction in instructions.iter_mut() {
        match instruction {
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                remove_condition_moved_aggregate_drops(then_instructions, moved_slots);
                remove_condition_moved_aggregate_drops(else_instructions, moved_slots);
            }
            Instruction::While {
                condition_instructions,
                body_instructions,
                ..
            } => {
                remove_condition_moved_aggregate_drops(condition_instructions, moved_slots);
                remove_condition_moved_aggregate_drops(body_instructions, moved_slots);
            }
            Instruction::CallFallibleI32 { failure_mode, .. }
            | Instruction::CallFallibleU8 { failure_mode, .. }
            | Instruction::CallFallibleUsize { failure_mode, .. }
            | Instruction::CallFallibleBool { failure_mode, .. }
            | Instruction::CallFallibleStr { failure_mode, .. }
            | Instruction::CallFallibleSlice { failure_mode, .. }
            | Instruction::CallFallibleAggregate { failure_mode, .. }
            | Instruction::CallFallibleDirectAggregate { failure_mode, .. } => {
                remove_condition_moved_aggregate_drops_from_failure_mode(failure_mode, moved_slots);
            }
            _ => {}
        }
    }
    instructions.retain(|instruction| !is_condition_moved_aggregate_drop(instruction, moved_slots));
}

fn remove_condition_moved_aggregate_drops_from_failure_mode(
    failure_mode: &mut FallibleFailureMode,
    moved_slots: &HashSet<usize>,
) {
    match failure_mode {
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            remove_condition_moved_aggregate_drops(instructions, moved_slots);
        }
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
    }
}

fn is_condition_moved_aggregate_drop(
    instruction: &Instruction,
    moved_slots: &HashSet<usize>,
) -> bool {
    let Instruction::CallVoid { arguments, .. } = instruction else {
        return false;
    };
    let [ScalarArgument::Borrow(argument)] = arguments.as_slice() else {
        return false;
    };
    let BorrowSource::AggregateSlot(slot_index) = argument.source else {
        return false;
    };
    moved_slots.contains(&slot_index)
}

pub(super) fn lower_nonterminal_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    loop_scope_mark: Option<usize>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(&statement.condition, context) {
        return Err(attach_primary_span_if_absent(
            unsupported_control_flow_condition_move_diagnostic(diagnostic_code),
            sources,
            statement.condition.span(),
        ));
    }

    let then_instructions = lower_nonterminal_if_block(
        &statement.then_block,
        context,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;
    let else_instructions = if let Some(else_block) = &statement.else_block {
        lower_nonterminal_if_block(
            else_block,
            context,
            loop_scope_mark,
            continue_instructions,
            diagnostic_code,
            subject,
            sources,
        )?
    } else {
        Vec::new()
    };

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )
}

pub(super) fn lower_nonterminal_while_statement(
    statement: &WhileStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move(&statement.condition, context) {
        return Err(attach_primary_span_if_absent(
            unsupported_control_flow_condition_move_diagnostic(diagnostic_code),
            sources,
            statement.condition.span(),
        ));
    }

    let condition = lower_bool_expression_to_value(&statement.condition, context, diagnostic_code)
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.condition.span())
        })?;
    let body_instructions =
        lower_nonterminal_while_block(&statement.body, context, diagnostic_code, subject, sources)?;

    Ok(vec![Instruction::While {
        condition_instructions: condition.instructions,
        condition: condition.value,
        body_instructions,
    }])
}

pub(super) fn lower_nonterminal_loop_statement(
    statement: &LoopStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let body_instructions =
        lower_nonterminal_while_block(&statement.body, context, diagnostic_code, subject, sources)?;

    Ok(vec![Instruction::While {
        condition_instructions: Vec::new(),
        condition: BoolValue::Const(true),
        body_instructions,
    }])
}

pub(super) fn lower_nonterminal_for_range_statement(
    statement: &ForRangeStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match context.binding_scalar_view_kind(statement.name_span) {
        Some(TypecheckScalarViewKind::I32) => lower_nonterminal_i32_for_range_statement(
            statement,
            context,
            diagnostic_code,
            subject,
            sources,
        ),
        Some(TypecheckScalarViewKind::Usize) => lower_nonterminal_usize_for_range_statement(
            statement,
            context,
            diagnostic_code,
            subject,
            sources,
        ),
        _ => Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        )),
    }
}

fn lower_nonterminal_i32_for_range_statement(
    statement: &ForRangeStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let value_hidden = hidden_for_range_local_name(statement, "value");
    let end_hidden = hidden_for_range_local_name(statement, "end");
    let value = context.next_i32_local_location()?;
    context.define_i32_local(value_hidden.clone());
    let end = context.next_i32_local_location()?;
    context.define_i32_local(end_hidden);

    let mut instructions = lower_i32_expression_to_location(&statement.start, value, context)
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.start.span())
        })?;
    instructions.extend(
        lower_i32_expression_to_location(&statement.end, end, context).map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.end.span())
        })?,
    );
    if !context.rename_local(&value_hidden, statement.name.clone()) {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let increment = vec![Instruction::AddI32 {
        destination: value,
        left: I32Value::Location(value),
        right: I32Value::Const(1),
    }];
    let body_instructions = lower_nonterminal_for_range_block(
        &statement.body,
        context,
        &increment,
        diagnostic_code,
        subject,
        sources,
    )?;
    if !context.rename_local(&statement.name, value_hidden) {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    }
    instructions.push(Instruction::While {
        condition_instructions: Vec::new(),
        condition: BoolValue::I32Comparison {
            operator: I32ComparisonOperator::Less,
            left: I32Value::Location(value),
            right: I32Value::Location(end),
        },
        body_instructions,
    });
    Ok(instructions)
}

fn lower_nonterminal_usize_for_range_statement(
    statement: &ForRangeStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let value_hidden = hidden_for_range_local_name(statement, "value");
    let end_hidden = hidden_for_range_local_name(statement, "end");
    let value = context.next_usize_local_location()?;
    context.define_usize_local(value_hidden.clone());
    let end = context.next_usize_local_location()?;
    context.define_usize_local(end_hidden);

    let mut instructions = lower_usize_expression_to_location(&statement.start, value, context)
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, statement.start.span())
        })?;
    instructions.extend(
        lower_usize_expression_to_location(&statement.end, end, context).map_err(
            |diagnostics| attach_primary_span_if_absent(diagnostics, sources, statement.end.span()),
        )?,
    );
    if !context.rename_local(&value_hidden, statement.name.clone()) {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    }

    let increment = vec![Instruction::AddUsize {
        destination: value,
        left: UsizeValue::Location(value),
        right: UsizeValue::Const(1),
    }];
    let body_instructions = lower_nonterminal_for_range_block(
        &statement.body,
        context,
        &increment,
        diagnostic_code,
        subject,
        sources,
    )?;
    if !context.rename_local(&statement.name, value_hidden) {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    }
    instructions.push(Instruction::While {
        condition_instructions: Vec::new(),
        condition: BoolValue::UsizeComparison {
            operator: I32ComparisonOperator::Less,
            left: UsizeValue::Location(value),
            right: UsizeValue::Location(end),
        },
        body_instructions,
    });
    Ok(instructions)
}

fn lower_nonterminal_for_range_block(
    block: &Block,
    context: &LoweringContext,
    increment_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut body_context = context.clone();
    let local_mark = body_context.local_mark();
    let lowered = lower_nonterminal_loop_block_statements(
        &block.statements,
        block.result.as_deref(),
        &mut body_context,
        local_mark,
        Some(local_mark),
        increment_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;
    let mut instructions = lowered.instructions;
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut body_context,
            local_mark,
        )?);
        instructions.extend(increment_instructions.iter().cloned());
    }
    Ok(instructions)
}

fn hidden_for_range_local_name(statement: &ForRangeStmt, role: &str) -> String {
    format!(
        "<for-range:{}:{}:{role}>",
        statement.name_span.start, statement.name_span.end
    )
}

fn lower_nonterminal_while_block(
    block: &Block,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut body_context = context.clone();
    let local_mark = body_context.local_mark();
    let lowered = lower_nonterminal_loop_block_statements(
        &block.statements,
        block.result.as_deref(),
        &mut body_context,
        local_mark,
        Some(local_mark),
        &[],
        diagnostic_code,
        subject,
        sources,
    )?;
    let mut instructions = lowered.instructions;
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut body_context,
            local_mark,
        )?);
    }
    Ok(instructions)
}

fn lower_nonterminal_if_block(
    block: &Block,
    context: &LoweringContext,
    loop_scope_mark: Option<usize>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let local_mark = branch_context.local_mark();
    let lowered = lower_nonterminal_loop_block_statements(
        &block.statements,
        block.result.as_deref(),
        &mut branch_context,
        local_mark,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;
    let mut instructions = lowered.instructions;
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut branch_context,
            local_mark,
        )?);
    }
    Ok(instructions)
}

fn lower_nonterminal_loop_block_statements(
    statements: &[Stmt],
    result: Option<&Expr>,
    context: &mut LoweringContext,
    local_mark: usize,
    loop_scope_mark: Option<usize>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<LoweredNonterminalBlock, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    let mut ends_execution = false;
    for (index, statement) in statements.iter().enumerate() {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.initializer,
                    context,
                    local_mark,
                ) && !outer_aggregate_move_binding_before_function_exit_allowed(
                    statement, context, local_mark, statements, index, result,
                ) {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.initializer.span(),
                    ));
                }
                let loop_control = loop_scope_mark.map(|loop_scope_mark| LoopControlContext {
                    loop_scope_mark,
                    continue_instructions,
                });
                instructions.extend(
                    lower_local_binding_with_loop_control(statement, context, loop_control)
                        .map_err(|diagnostics| {
                            attach_primary_span_if_absent(diagnostics, sources, statement.span)
                        })?,
                )
            }
            Stmt::Assignment(statement) => {
                let target_allowed =
                    nonterminal_assignment_target_allowed(statement, context, local_mark)
                        || outer_aggregate_assignment_before_function_exit_allowed(
                            statement, context, local_mark, statements, index, result,
                        );
                let explicit_outer_aggregate_move_allowed =
                    aggregate_move_assignment_before_function_exit_allowed(
                        statement, context, local_mark, statements, index, result,
                    );
                if !target_allowed
                    || (expression_contains_explicit_aggregate_move_outside(
                        &statement.value,
                        context,
                        local_mark,
                    ) && !explicit_outer_aggregate_move_allowed)
                {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.span,
                    ));
                }
                instructions.extend(lower_assignment(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Expression(statement) => {
                if expression_contains_explicit_aggregate_move_outside(
                    &statement.expression,
                    context,
                    local_mark,
                ) {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.expression.span(),
                    ));
                }
                if let Some(terminating_instructions) =
                    lower_never_expression_with_scope_drops(&statement.expression, context)
                        .map_err(|diagnostics| {
                            attach_primary_span_if_absent(
                                diagnostics,
                                sources,
                                statement.expression.span(),
                            )
                        })?
                {
                    instructions.extend(terminating_instructions);
                    ends_execution = true;
                } else {
                    let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, context).map_err(
                            |diagnostics| {
                                attach_primary_span_if_absent(
                                    diagnostics,
                                    sources,
                                    statement.expression.span(),
                                )
                            },
                        )?
                    else {
                        return Err(attach_primary_span_if_absent(
                            unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                            sources,
                            statement.span,
                        ));
                    };
                    instructions.extend(void_instructions);
                }
            }
            Stmt::Drop(statement) => {
                if !context.aggregate_local_defined_since(&statement.name, local_mark)
                    && !statement_suffix_exits_function(statements, index, result, context)
                {
                    return Err(attach_primary_span_if_absent(
                        unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
                        sources,
                        statement.span,
                    ));
                }
                instructions.extend(lower_drop_statement(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Return(statement) => {
                instructions.extend(
                    lower_terminal_return_statement_with_scope_drops(
                        statement,
                        context,
                        diagnostic_code,
                        subject,
                        sources,
                    )
                    .map_err(|diagnostics| {
                        let span = statement
                            .expression
                            .as_ref()
                            .map_or(statement.span, |expression| expression.span());
                        attach_primary_span_if_absent(diagnostics, sources, span)
                    })?,
                );
                ends_execution = true;
                break;
            }
            Stmt::If(statement) => {
                let lowered = lower_nonterminal_if_statement(
                    statement,
                    context,
                    loop_scope_mark,
                    continue_instructions,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?;
                ends_execution = instruction_list_ends_execution(&lowered);
                instructions.extend(lowered);
            }
            Stmt::IfIs(statement) => {
                let if_statement =
                    payloadless_if_is_as_if_statement(statement, context, diagnostic_code)
                        .map_err(|diagnostics| {
                            attach_primary_span_if_absent(
                                diagnostics,
                                sources,
                                statement.pattern_span,
                            )
                        })?;
                let lowered = lower_nonterminal_if_statement(
                    &if_statement,
                    context,
                    loop_scope_mark,
                    continue_instructions,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?;
                ends_execution = instruction_list_ends_execution(&lowered);
                instructions.extend(lowered);
            }
            Stmt::Switch(statement) => {
                let switch =
                    payloadless_switch_as_if_statement(statement, context, diagnostic_code)
                        .map_err(|diagnostics| {
                            attach_primary_span_if_absent(diagnostics, sources, statement.span)
                        })?;
                instructions.extend(switch.leading_instructions);
                let lowered = lower_nonterminal_if_statement(
                    &switch.if_statement,
                    context,
                    loop_scope_mark,
                    continue_instructions,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?;
                ends_execution = instruction_list_ends_execution(&lowered);
                instructions.extend(lowered);
            }
            Stmt::While(statement) => instructions.extend(
                lower_nonterminal_while_statement(
                    statement,
                    context,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::ForRange(statement) => instructions.extend(
                lower_nonterminal_for_range_statement(
                    statement,
                    context,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Loop(statement) => instructions.extend(
                lower_nonterminal_loop_statement(
                    statement,
                    context,
                    diagnostic_code,
                    subject,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Break(_) => {
                instructions.extend(
                    lower_nonterminal_loop_control_statement(
                        Instruction::Break,
                        context,
                        loop_scope_mark,
                        &[],
                        diagnostic_code,
                        subject,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span())
                    })?,
                );
                ends_execution = true;
                break;
            }
            Stmt::Continue(_) => {
                instructions.extend(
                    lower_nonterminal_loop_control_statement(
                        Instruction::Continue,
                        context,
                        loop_scope_mark,
                        continue_instructions,
                        diagnostic_code,
                        subject,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span())
                    })?,
                );
                ends_execution = true;
                break;
            }
        }
        mark_lowered_statement_aggregate_uses(statement, context);
        if ends_execution {
            break;
        }
    }
    if !ends_execution && let Some(result) = result {
        instructions.extend(lower_nonterminal_block_result(
            result,
            context,
            local_mark,
            diagnostic_code,
            subject,
            sources,
        )?);
        ends_execution = expression_exits_function(result, context);
    }
    Ok(LoweredNonterminalBlock {
        instructions,
        ends_execution,
    })
}

fn lower_nonterminal_block_result(
    expression: &Expr,
    context: &mut LoweringContext,
    local_mark: usize,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if expression_contains_explicit_aggregate_move_outside(expression, context, local_mark) {
        return Err(attach_primary_span_if_absent(
            unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
            sources,
            expression.span(),
        ));
    }

    if let Some(terminating_instructions) =
        lower_never_expression_with_scope_drops(expression, context).map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, expression.span())
        })?
    {
        mark_explicit_moves_in_expression(expression, context);
        return Ok(terminating_instructions);
    }

    let Some(void_instructions) =
        lower_void_expression_statement(expression, context).map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, expression.span())
        })?
    else {
        return Err(attach_primary_span_if_absent(
            unsupported_nonterminal_if_diagnostic(diagnostic_code, subject),
            sources,
            expression.span(),
        ));
    };
    mark_explicit_moves_in_expression(expression, context);
    Ok(void_instructions)
}

fn statement_suffix_exits_function(
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
    context: &LoweringContext,
) -> bool {
    statement_sequence_or_result_exits_function(
        statements.get(index + 1..).unwrap_or(&[]),
        result,
        context,
    )
}

fn statement_sequence_or_result_exits_function(
    statements: &[Stmt],
    result: Option<&Expr>,
    context: &LoweringContext,
) -> bool {
    for statement in statements {
        if statement_may_exit_current_loop(statement) {
            return false;
        }
        if statement_exits_function(statement, context) {
            return true;
        }
    }
    result.is_some_and(|expression| expression_exits_function(expression, context))
}

pub(super) fn statement_exits_function(statement: &Stmt, context: &LoweringContext) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(_) => true,
        Stmt::Expression(statement) => expression_exits_function(&statement.expression, context),
        Stmt::If(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        Stmt::IfIs(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        Stmt::Switch(statement) => {
            if statement.else_arm.is_none() && !payloadless_switch_is_exhaustive(statement, context)
            {
                return false;
            }

            statement
                .arms
                .iter()
                .all(|arm| block_exits_function(&arm.body, context))
                && statement
                    .else_arm
                    .as_ref()
                    .is_none_or(|else_arm| block_exits_function(&else_arm.body, context))
        }
        _ => false,
    }
}

fn block_exits_function(block: &Block, context: &LoweringContext) -> bool {
    statement_sequence_or_result_exits_function(&block.statements, block.result.as_deref(), context)
}

fn expression_exits_function(expression: &Expr, context: &LoweringContext) -> bool {
    match unwrap_group(expression) {
        Expr::Call(call) => {
            if primitive_trap_call(call, context) {
                return true;
            }
            let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
                return false;
            };
            context.call_return_type(&target) == Some(&Type::Never)
        }
        Expr::If(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        Expr::IfIs(statement) => {
            let Some(else_block) = &statement.else_block else {
                return false;
            };
            block_exits_function(&statement.then_block, context)
                && block_exits_function(else_block, context)
        }
        Expr::Match(statement) => {
            if statement.else_arm.is_none() && !payloadless_switch_is_exhaustive(statement, context)
            {
                return false;
            }

            statement
                .arms
                .iter()
                .all(|arm| block_exits_function(&arm.body, context))
                && statement
                    .else_arm
                    .as_ref()
                    .is_none_or(|else_arm| block_exits_function(&else_arm.body, context))
        }
        _ => false,
    }
}

fn statement_may_exit_current_loop(statement: &Stmt) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Stmt::IfIs(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Stmt::Switch(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop(&arm.body))
                || statement
                    .else_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop(&arm.body))
        }
        Stmt::While(_) | Stmt::Loop(_) => false,
        _ => false,
    }
}

fn block_may_exit_current_loop(block: &Block) -> bool {
    block.statements.iter().any(statement_may_exit_current_loop)
        || block
            .result
            .as_deref()
            .is_some_and(expression_may_exit_current_loop)
}

fn expression_may_exit_current_loop(expression: &Expr) -> bool {
    match unwrap_group(expression) {
        Expr::If(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Expr::IfIs(statement) => {
            block_may_exit_current_loop(&statement.then_block)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(block_may_exit_current_loop)
        }
        Expr::Match(statement) => {
            statement
                .arms
                .iter()
                .any(|arm| block_may_exit_current_loop(&arm.body))
                || statement
                    .else_arm
                    .as_ref()
                    .is_some_and(|arm| block_may_exit_current_loop(&arm.body))
        }
        _ => false,
    }
}

fn outer_aggregate_move_binding_before_function_exit_allowed(
    statement: &crate::ast::BindingStmt,
    context: &LoweringContext,
    local_mark: usize,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
) -> bool {
    statement_suffix_exits_function(statements, index, result, context)
        && direct_outer_aggregate_move(&statement.initializer, context, local_mark)
}

fn direct_outer_aggregate_move(
    expression: &Expr,
    context: &LoweringContext,
    local_mark: usize,
) -> bool {
    let Expr::Unary(unary) = unwrap_group(expression) else {
        return false;
    };
    if unary.operator != crate::ast::UnaryOperator::Move {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&unary.operand) else {
        return false;
    };
    context.aggregate_local(&identifier.name).is_some()
        && !context.aggregate_local_defined_since(&identifier.name, local_mark)
}

fn lower_nonterminal_loop_control_statement(
    instruction: Instruction,
    context: &mut LoweringContext,
    loop_scope_mark: Option<usize>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(loop_scope_mark) = loop_scope_mark else {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    };

    let mut instructions = lower_scope_end_drops_for_locals_since(context, loop_scope_mark)?;
    if matches!(instruction, Instruction::Continue) {
        instructions.extend(continue_instructions.iter().cloned());
    }
    instructions.push(instruction);
    Ok(instructions)
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

fn lower_short_circuit_terminal_condition(
    binary: &BinaryExpr,
    then_instructions: Vec<Instruction>,
    else_instructions: Vec<Instruction>,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match binary.operator {
        BinaryOperator::LogicalAnd => lower_terminal_condition(
            &binary.left,
            lower_terminal_condition(
                &binary.right,
                then_instructions,
                else_instructions.clone(),
                context,
                diagnostic_code,
                sources,
            )?,
            else_instructions,
            context,
            diagnostic_code,
            sources,
        ),
        BinaryOperator::LogicalOr => lower_terminal_condition(
            &binary.left,
            then_instructions.clone(),
            lower_terminal_condition(
                &binary.right,
                then_instructions,
                else_instructions,
                context,
                diagnostic_code,
                sources,
            )?,
            context,
            diagnostic_code,
            sources,
        ),
        _ => unreachable!("short-circuit condition must be && or ||"),
    }
}

fn short_circuit_condition_needs_branch<'a>(
    condition: &'a Expr,
    context: &LoweringContext,
) -> Option<&'a BinaryExpr> {
    let condition = unwrap_group(condition);
    let Expr::Binary(binary) = condition else {
        return None;
    };

    if short_circuit_bool_expression_needs_branch(binary, context) {
        Some(binary)
    } else {
        None
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn assignment_target_root_name(expression: &Expr) -> Option<&str> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Some(&identifier.name),
        Expr::Member(member) => assignment_target_root_name(&member.object),
        _ => None,
    }
}

fn nonterminal_assignment_target_allowed(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
    local_mark: usize,
) -> bool {
    assignment_target_root_name(&statement.target)
        .is_some_and(|target_name| context.local_defined_since(target_name, local_mark))
        || assignment_targets_whole_scalar_or_view_local(statement, context)
        || compound_assignment_targets_scalar_integer_local(statement, context)
        || assignment_targets_whole_aggregate_local(statement, context)
        || assignment_targets_readwrite_aggregate_field(statement, context)
        || assignment_targets_direct_slice_index(statement, context)
}

fn assignment_targets_whole_scalar_or_view_local(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&statement.target) else {
        return false;
    };
    matches!(
        context.i32_location(&identifier.name),
        Some(I32Location::Local(_))
    ) || matches!(
        context.u8_location(&identifier.name),
        Some(U8Location::Local(_))
    ) || matches!(
        context.usize_location(&identifier.name),
        Some(UsizeLocation::Local(_))
    ) || matches!(
        context.bool_location(&identifier.name),
        Some(BoolLocation::Local(_))
    ) || matches!(
        context.str_location(&identifier.name),
        Some(StrLocation::Local(_))
    ) || matches!(
        context.slice_location(&identifier.name),
        Some(SliceLocation::Local(_))
    )
}

fn compound_assignment_targets_scalar_integer_local(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    if statement.operator == AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&statement.target) else {
        return false;
    };
    matches!(
        context.i32_location(&identifier.name),
        Some(I32Location::Local(_))
    ) || matches!(
        context.usize_location(&identifier.name),
        Some(UsizeLocation::Local(_))
    )
}

fn assignment_targets_whole_aggregate_local(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
) -> bool {
    if statement.operator != AssignmentOperator::Assign {
        return false;
    }
    let Expr::Identifier(identifier) = unwrap_group(&statement.target) else {
        return false;
    };
    context.aggregate_local(&identifier.name).is_some()
}

fn outer_aggregate_assignment_before_function_exit_allowed(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
    local_mark: usize,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
) -> bool {
    if !statement_suffix_exits_function(statements, index, result, context) {
        return false;
    }
    let Some(target_name) = assignment_target_root_name(&statement.target) else {
        return false;
    };
    context.aggregate_local(target_name).is_some()
        && !context.aggregate_local_defined_since(target_name, local_mark)
}

fn aggregate_move_assignment_before_function_exit_allowed(
    statement: &crate::ast::AssignmentStmt,
    context: &LoweringContext,
    local_mark: usize,
    statements: &[Stmt],
    index: usize,
    result: Option<&Expr>,
) -> bool {
    if !statement_suffix_exits_function(statements, index, result, context) {
        return false;
    }
    let Some(target_name) = assignment_target_root_name(&statement.target) else {
        return false;
    };
    context.aggregate_local(target_name).is_some()
        && direct_outer_aggregate_move(&statement.value, context, local_mark)
}

pub(super) fn instruction_list_ends_execution(instructions: &[Instruction]) -> bool {
    match instructions.last() {
        Some(
            Instruction::Return
            | Instruction::ReturnFallibleSuccess
            | Instruction::ReturnOptionalNone
            | Instruction::ReturnFallibleFailure { .. }
            | Instruction::TailCall { .. }
            | Instruction::Trap
            | Instruction::Break
            | Instruction::Continue,
        ) => true,
        Some(Instruction::If {
            then_instructions,
            else_instructions,
            ..
        }) => {
            !else_instructions.is_empty()
                && instruction_list_ends_execution(then_instructions)
                && instruction_list_ends_execution(else_instructions)
        }
        _ => false,
    }
}

fn lower_i32_return_block(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "i32")?;
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "i32",
        sources,
    )?;

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_i32_result_expression(
                expression,
                &mut branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                &mut branch_context,
                diagnostic_code,
                subject,
                sources,
            )?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
            instructions.extend(lower_terminal_i32_if_statement(
                statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, &branch_context, diagnostic_code)?;
            instructions.extend(lower_terminal_i32_if_statement(
                &if_statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch = payloadless_switch_as_if_statement(
                statement,
                &mut branch_context,
                diagnostic_code,
            )?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_i32_if_statement(
                &switch.if_statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "i32",
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "i32",
        )),
    }
}

fn lower_bool_return_block(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "bool")?;
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "bool",
        sources,
    )?;

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_bool_result_expression(
                expression,
                &mut branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                &mut branch_context,
                diagnostic_code,
                subject,
                sources,
            )?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
            instructions.extend(lower_terminal_bool_if_statement(
                statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, &branch_context, diagnostic_code)?;
            instructions.extend(lower_terminal_bool_if_statement(
                &if_statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch = payloadless_switch_as_if_statement(
                statement,
                &mut branch_context,
                diagnostic_code,
            )?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_bool_if_statement(
                &switch.if_statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "bool",
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "bool",
        )),
    }
}

fn lower_i32_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_i32_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, context, diagnostic_code)?;
            lower_terminal_i32_if_statement(
                &if_statement,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_if_statement(statement, context, diagnostic_code)?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_i32_if_statement(
                &switch.if_statement,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        _ => lower_implicit_return_result_expression(expression, context, diagnostic_code),
    }
}

fn lower_bool_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_bool_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, context, diagnostic_code)?;
            lower_terminal_bool_if_statement(
                &if_statement,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_if_statement(statement, context, diagnostic_code)?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_bool_if_statement(
                &switch.if_statement,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        _ => lower_implicit_return_result_expression(expression, context, diagnostic_code),
    }
}

fn lower_scalar_return_block(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) =
        split_terminal_branch_block(block, diagnostic_code, subject, return_label)?;
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        return_label,
        sources,
    )?;

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_scalar_result_expression(
                expression,
                &mut branch_context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                &mut branch_context,
                diagnostic_code,
                subject,
                sources,
            )?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
            instructions.extend(lower_terminal_scalar_if_statement(
                statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, &branch_context, diagnostic_code)?;
            instructions.extend(lower_terminal_scalar_if_statement(
                &if_statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch = payloadless_switch_as_if_statement(
                statement,
                &mut branch_context,
                diagnostic_code,
            )?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_scalar_if_statement(
                &switch.if_statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    return_label,
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            return_label,
        )),
    }
}

fn lower_scalar_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    lower_return_expression: ReturnLowerer,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_scalar_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            return_label,
            lower_return_expression,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, context, diagnostic_code)?;
            lower_terminal_scalar_if_statement(
                &if_statement,
                context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_if_statement(statement, context, diagnostic_code)?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_scalar_if_statement(
                &switch.if_statement,
                context,
                return_type,
                diagnostic_code,
                subject,
                return_label,
                lower_return_expression,
                sources,
            )?);
            Ok(instructions)
        }
        _ => lower_implicit_return_result_expression(expression, context, diagnostic_code),
    }
}

fn lower_void_return_block(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) = split_terminal_branch_block(block, diagnostic_code, subject, "void")?;
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        diagnostic_code,
        subject,
        "void",
        sources,
    )?;

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_void_result_expression(
                expression,
                &mut branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                &mut branch_context,
                diagnostic_code,
                subject,
                sources,
            )?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
            instructions.extend(lower_terminal_void_if_statement(
                statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, &branch_context, diagnostic_code)?;
            instructions.extend(lower_terminal_void_if_statement(
                &if_statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch = payloadless_switch_as_if_statement(
                statement,
                &mut branch_context,
                diagnostic_code,
            )?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_void_if_statement(
                &switch.if_statement,
                &branch_context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Expression(statement)) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                &mut branch_context,
            )?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "void",
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            "void",
        )),
    }
}

fn lower_void_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_void_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_statement =
                payloadless_if_is_as_if_statement(statement, context, diagnostic_code)?;
            lower_terminal_void_if_statement(
                &if_statement,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_if_statement(statement, context, diagnostic_code)?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_void_if_statement(
                &switch.if_statement,
                context,
                return_type,
                diagnostic_code,
                subject,
                sources,
            )?);
            Ok(instructions)
        }
        _ => {
            if let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(expression, context)?
            {
                mark_explicit_moves_in_expression(expression, context);
                return Ok(terminating_instructions);
            }

            let Some(mut void_instructions) = lower_void_expression_statement(expression, context)?
            else {
                return Err(unsupported_terminal_if_diagnostic(
                    diagnostic_code,
                    subject,
                    "void",
                ));
            };
            mark_explicit_moves_in_expression(expression, context);
            void_instructions.extend(append_scope_end_drops_before_exit(
                vec![success_return_instruction(return_type)],
                context,
            )?);
            Ok(void_instructions)
        }
    }
}

fn lower_implicit_return_result_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let statement = ReturnStmt {
        span: expression.span(),
        expression: Some(expression.clone()),
    };
    lower_return_statement_with_scope_drops(&statement, context, diagnostic_code)
}

pub(super) fn split_terminal_branch_block<'a>(
    block: &'a Block,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
) -> Result<(TerminalBranch<'a>, &'a [Stmt]), Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        return Ok((TerminalBranch::Result(result), block.statements.as_slice()));
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            return_label,
        ));
    };
    Ok((TerminalBranch::Statement(terminal), leading))
}

pub(super) fn lower_terminal_branch_leading_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => instructions.extend(
                lower_local_binding(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Assignment(statement) => instructions.extend(
                lower_assignment(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Drop(statement) => instructions.extend(
                lower_drop_statement(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
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
                        unsupported_terminal_if_diagnostic(diagnostic_code, subject, return_label),
                        sources,
                        statement.span,
                    ));
                };
                instructions.extend(void_instructions);
            }
            _ => {
                return Err(attach_primary_span_if_absent(
                    unsupported_terminal_if_diagnostic(diagnostic_code, subject, return_label),
                    sources,
                    statement.span(),
                ));
            }
        }
        mark_lowered_statement_aggregate_uses(statement, context);
    }
    Ok(instructions)
}

fn unsupported_terminal_if_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
    return_type: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower terminal `if` statements for {subject} when both branches contain only supported binding, assignment, explicit `drop`, or effect-only call statements followed by returns or nested terminal `if` branches returning `{return_type}`"
        ),
    )]
}

fn unsupported_nonterminal_if_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower non-terminal `if`/`while`/`loop` statements for {subject} when branches/bodies contain supported local bindings, branch/body-local assignments, outer scalar/view/aggregate local assignments, explicit aggregate drops, effect-only call statements, returns, or nested non-terminal `if`/`while`/`loop` statements"
        ),
    )]
}

fn unsupported_control_flow_condition_move_diagnostic(
    diagnostic_code: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 cannot lower control-flow conditions that explicitly move aggregate values",
    )]
}

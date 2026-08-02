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
    BranchPrologue, LoweredPayloadlessSwitchBody, LoweredSwitchBlock, LoweredSwitchCondition,
    append_scope_end_drops_before_exit, expression_contains_explicit_aggregate_move,
    expression_contains_explicit_aggregate_move_outside, lower_drop_statement,
    lower_never_expression_with_scope_drops, lower_return_statement_with_scope_drops,
    lower_scope_end_drops_for_locals_since, lower_terminal_return_statement_with_scope_drops,
    lowerable_switch_is_exhaustive, mark_explicit_moves_in_expression,
    mark_lowered_statement_aggregate_uses, payloadless_switch_as_control_flow,
    payloadless_switch_is_exhaustive, tag_only_if_is_as_control_flow,
    tag_only_switch_as_control_flow,
};
use super::regions::CleanupScopeMark;
pub(super) use super::regions::lower_nonterminal_region_statement;
use crate::ast::{
    AssignmentOperator, BinaryExpr, BinaryOperator, Block, Expr, ForRangeStmt, IfStmt, LoopStmt,
    ReturnStmt, Stmt, SwitchStmt, UnaryOperator, WhileStmt,
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

mod assignments;
mod condition_drops;
mod diagnostics;
mod exit_analysis;
mod loop_control;
mod model;
pub(in crate::ir::lower) mod nonterminal;
mod return_blocks;
mod terminal;
mod terminal_branches;
mod terminal_conditions;
mod utility;

use assignments::{
    aggregate_move_assignment_before_function_exit_allowed, nonterminal_assignment_target_allowed,
    outer_aggregate_assignment_before_function_exit_allowed,
    outer_aggregate_move_binding_before_function_exit_allowed,
};
use condition_drops::{
    aggregate_argument_slots_in_instructions, condition_explicit_moves_are_single_evaluation_call,
    remove_condition_moved_aggregate_drops,
};
use diagnostics::{
    attach_primary_span_if_absent, unsupported_control_flow_condition_move_diagnostic,
    unsupported_nonterminal_if_diagnostic, unsupported_terminal_if_diagnostic,
};
pub(super) use exit_analysis::statement_exits_function;
use exit_analysis::{expression_exits_function, statement_suffix_exits_function};
use loop_control::lower_nonterminal_loop_control_statement;
pub(super) use model::TerminalBranch;
use model::{LoweredNonterminalBlock, ReturnLowerer};
pub(super) use nonterminal::{
    lower_nonterminal_for_range_statement, lower_nonterminal_if_statement,
    lower_nonterminal_if_statement_with_branch_prologues, lower_nonterminal_loop_statement,
    lower_nonterminal_payloadless_switch_body, lower_nonterminal_payloadless_switch_statement,
    lower_nonterminal_while_statement,
};
pub(super) use return_blocks::instruction_list_ends_execution;
use return_blocks::{
    lower_bool_return_block_with_context_and_prefix, lower_bool_return_block_with_prologue,
    lower_i32_return_block_with_context_and_prefix, lower_i32_return_block_with_prologue,
    lower_scalar_return_block, lower_scalar_return_block_with_context_and_prefix,
    lower_void_return_block_with_context_and_prefix, lower_void_return_block_with_prologue,
};
use terminal::{
    lower_terminal_bool_if_statement, lower_terminal_bool_payloadless_switch_body,
    lower_terminal_i32_if_statement, lower_terminal_i32_payloadless_switch_body,
    lower_terminal_scalar_if_statement, lower_terminal_scalar_if_statement_with_branch_prologues,
    lower_terminal_scalar_payloadless_switch_body, lower_terminal_void_if_statement,
    lower_terminal_void_payloadless_switch_body,
};
pub(super) use terminal::{
    lower_terminal_bool_if_statement_with_branch_prologues, lower_terminal_bool_switch_block,
    lower_terminal_condition, lower_terminal_i32_if_statement_with_branch_prologues,
    lower_terminal_i32_switch_block, lower_terminal_slice_if_statement_with_branch_prologues,
    lower_terminal_slice_switch_block, lower_terminal_str_if_statement_with_branch_prologues,
    lower_terminal_str_switch_block, lower_terminal_u8_if_statement_with_branch_prologues,
    lower_terminal_u8_switch_block, lower_terminal_usize_if_statement_with_branch_prologues,
    lower_terminal_usize_switch_block, lower_terminal_void_if_statement_with_branch_prologues,
    lower_terminal_void_switch_block,
};
pub(super) use terminal_branches::{
    lower_terminal_branch_leading_statements, split_terminal_branch_block,
};
use terminal_conditions::{
    lower_short_circuit_terminal_condition, short_circuit_condition_needs_branch,
};
use utility::unwrap_group;

use super::aggregates::{
    ArrayInitializationProgress, PayloadInitializationProgress, StructInitializationProgress,
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr_with_resolver,
    aggregate_type_layout, array_literal_requires_runtime_progress,
    lower_aggregate_array_literal_to_location,
    lower_aggregate_array_literal_to_location_with_progress,
    lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries,
    lower_aggregate_struct_literal_to_location_with_temporaries,
    lower_payload_enum_constructor_to_location,
    lower_payload_enum_constructor_to_location_with_progress, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
    type_expr_is_copy_aggregate_value_with_resolver,
};
use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{
    AggregateBorrowParameter, AggregateDrop, AggregateField, AggregateParameterSource, ArrayDrop,
    ArrayElementDropState, BorrowParameter, DropObligation, ErrorPayloads, FunctionNames,
    FunctionSignatures, LiteralPackLowering, LiteralPackLoweringSegment,
    LoweringAggregateParameter, LoweringContext, LoweringOutcomeParameter, LoweringParameterSlots,
    OutcomeDrop, PayloadEnumDrop, PayloadEnumDropField, PayloadEnumDropVariant,
    PayloadFieldDropState, PendingAggregateDrop, ResolvedSources, SliceTypeInfo, StructDrop,
    StructDropField, StructFieldDropState, aggregate_drop_for_type_expr_with_resolver,
    outcome_drop_for_type_expr_with_resolver,
};
use super::control_flow::{
    TerminalBranch, lower_nonterminal_for_range_statement, lower_nonterminal_if_statement,
    lower_nonterminal_if_statement_with_branch_prologues, lower_nonterminal_loop_statement,
    lower_nonterminal_payloadless_switch_body, lower_nonterminal_payloadless_switch_statement,
    lower_nonterminal_region_statement, lower_nonterminal_while_statement,
    lower_terminal_bool_if_statement_with_branch_prologues, lower_terminal_bool_switch_block,
    lower_terminal_branch_leading_statements, lower_terminal_condition,
    lower_terminal_i32_if_statement_with_branch_prologues, lower_terminal_i32_switch_block,
    lower_terminal_slice_if_statement_with_branch_prologues, lower_terminal_slice_switch_block,
    lower_terminal_str_if_statement_with_branch_prologues, lower_terminal_str_switch_block,
    lower_terminal_u8_if_statement_with_branch_prologues, lower_terminal_u8_switch_block,
    lower_terminal_usize_if_statement_with_branch_prologues, lower_terminal_usize_switch_block,
    lower_terminal_void_if_statement_with_branch_prologues, lower_terminal_void_switch_block,
    split_terminal_branch_block, statement_exits_function,
};
use super::errors::{ErrorPayload, lower_error_payload};
use super::expressions::{
    PointerTakeDestination, TemporaryAllocator, lower_aggregate_member_field_access,
    lower_bool_expression_to_location, lower_bool_return_expression,
    lower_borrow_expression_to_location, lower_call_arguments_to_scalar_arguments,
    lower_catch_failure_mode, lower_fallible_bool_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
    lower_i32_expression_to_location, lower_i32_return_expression,
    lower_macos_syscall_primitive_call_to_location, lower_never_return_expression,
    lower_slice_expression_to_location, lower_slice_return_expression,
    lower_str_expression_to_location, lower_str_return_expression,
    lower_take_value_at_ptr_primitive_call, lower_u8_expression_to_location,
    lower_u8_return_expression, lower_usize_expression_to_location, lower_usize_return_expression,
    lower_void_expression_statement, mark_outcome_success_returns,
    primitive_take_value_at_ptr_call, success_return_instruction,
};
use super::types::{
    borrow_inner_type_with_resolver, borrow_type_from_type_expr,
    parameter_type_from_type_expr_with_resolver, return_type_expr_has_optional_layer_with_resolver,
    return_type_expr_is_top_level_optional_with_resolver, return_type_from_type_expr_with_resolver,
    type_expr_with_self_type, view_element_type_from_type_expr_with_resolver,
};
use crate::abi::{
    AbiType, AbiValue, ValueClassification, ValueLayout, abi_value_from_type_expr_with_resolver,
    function_parameter_abi_word_count_from_signature_with_resolver, layout_of,
};
use crate::analysis::literal_specializations::{
    LiteralSpecialization, literal_element_parameter_name,
};
use crate::ast::{
    ArrayLiteralExpr, BinaryExpr, BinaryOperator, Block, CallExpr, DestructDecl, DropStmt, Expr,
    FunctionDecl, IdentifierExpr, IfIsStmt, IfStmt, LiteralDecl, LiteralExpr, LiteralShape,
    MemberExpr, MethodDecl, Parameter, PayloadEnumPatternTargetShape, ReturnStmt, Stmt,
    StructLiteralExpr, SwitchArm, SwitchPayloadPattern, SwitchStmt, TypeExpr, TypeReference,
    UnaryOperator, substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, BoolValue, BorrowArgument, BorrowSource, CallTarget, Function,
    I32ComparisonOperator, I32Location, I32Value, Instruction, OutcomeFailureMode, ScalarArgument,
    SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation,
    UsizeValue,
};
use crate::outcomes::outcome_shape_with_resolver;
use crate::resolve::{
    FunctionSignature as ResolvedFunctionSignature, ParameterSignature, ResolveOutput,
};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{TypecheckFacts, TypecheckPayloadBindingMode, TypecheckSliceElementKind};
use std::collections::HashMap;

use super::outcome_propagation::propagating_outcome_mode;

mod aggregate_returns;
mod callable_body;
mod diagnostics;
mod entrypoints;
mod otherwise_returns;
mod outcome_returns;
pub(super) mod parameters;
mod payload_patterns;
mod return_scopes;
mod scope_drops;
mod switches;
mod value_returns;

use aggregate_returns::*;
use callable_body::*;
use diagnostics::*;
use otherwise_returns::*;
use outcome_returns::*;
use parameters::*;
use switches::*;
use value_returns::*;

pub(in crate::ir::lower) use aggregate_returns::lower_aggregate_drop_instructions_at_location;
pub(in crate::ir::lower) use aggregate_returns::lower_aggregate_return_expression_to_location;
pub(in crate::ir::lower) use callable_body::reachable_body_prefix;
pub(in crate::ir::lower) use entrypoints::*;
pub(in crate::ir::lower) use payload_patterns::*;
pub(in crate::ir::lower) use return_scopes::*;
pub(in crate::ir::lower) use scope_drops::*;

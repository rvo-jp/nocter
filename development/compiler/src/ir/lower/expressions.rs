use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    aggregate_type_layout, lower_aggregate_struct_literal_to_location_with_temporaries,
    push_aggregate_call_instruction, push_fallible_aggregate_call_instruction,
    supported_aggregate_copy_layout,
};
use super::bindings::{
    lower_aggregate_optional_otherwise_to_location, lower_assignment,
    lower_bool_optional_otherwise_to_location, lower_i32_optional_otherwise_to_location,
    lower_local_binding, lower_slice_optional_otherwise_to_location,
    lower_str_optional_otherwise_to_location, lower_u8_optional_otherwise_to_location,
    lower_usize_optional_otherwise_to_location,
};
use super::context::{AggregateDrop, AggregateFieldKind, LoweringContext};
use super::errors::{ErrorPayload, lower_error_payload};
use super::functions::{
    BranchPrologue, LoweredPayloadlessSwitchBody, LoweredSwitchBlock, LoweredSwitchCondition,
    append_scope_end_drops_before_exit, expression_contains_explicit_aggregate_move,
    expression_contains_explicit_aggregate_move_outside, lower_aggregate_drop_instructions,
    lower_aggregate_return_expression, lower_direct_aggregate_return_with_scope_drops,
    lower_never_expression_with_scope_drops, lower_scope_end_drops_for_locals_since,
    lower_value_return_with_scope_drops, mark_explicit_moves_in_expression,
    mark_lowered_statement_aggregate_uses, propagating_failure_mode,
    tag_only_if_is_as_control_flow, tag_only_switch_as_control_flow,
};
use super::literals::{
    lower_i32_literal, lower_str_literal, lower_u8_literal, lower_usize_literal,
};
use super::types::{
    return_type_expr_is_top_level_optional_with_resolver, scalar_or_view_type_from_type_expr,
    top_level_optional_success_abi_value_with_resolver,
};
mod aggregate_fields;
mod aggregate_members;
mod bool_values;
mod byte_collections;
mod byte_view_values;
mod call_arguments;
mod calls;
mod control_flow_values;
mod diagnostics;
mod fallible;
mod fixed_array_accesses;
mod fixed_arrays;
mod integer_values;
mod predicates;
mod returns;
mod scalar_values;
mod statement_effects;
mod temporaries;
mod utility;
mod void_effects;

pub(super) use aggregate_fields::*;
use aggregate_members::*;
pub(super) use bool_values::*;
use byte_collections::*;
pub(super) use byte_view_values::*;
pub(super) use call_arguments::*;
use control_flow_values::*;
use diagnostics::*;
use fallible::*;
pub(super) use fixed_array_accesses::*;
use fixed_arrays::*;
pub(super) use integer_values::*;
pub(super) use returns::*;
use scalar_values::*;
use statement_effects::*;
use utility::*;
pub(super) use void_effects::*;

use crate::abi::{
    AbiType, ValueLayout, abi_value_from_type_expr, abi_value_from_type_expr_with_resolver,
    array_element_stride,
};
use crate::ast::{
    BinaryExpr, BinaryOperator, Block, CallExpr, CatchExpr, Expr, IfIsStmt, IfStmt, IndexExpr,
    Stmt, SwitchStmt, TypeConversionExpr, UnaryExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue,
    FallibleFailureMode, I32ComparisonOperator, I32Location, I32Value, Instruction, ScalarArgument,
    SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation,
    UsizeValue,
};
use crate::literals::decode_integer_literal_value;
pub(super) use calls::lower_macos_syscall_primitive_call_to_location;
pub(super) use calls::lower_pointer_address_expression_to_word;
pub(super) use calls::primitive_trap_call;
use calls::{
    call_arguments_require_stack, is_tail_call_stack_pointer_argument,
    lower_addr_primitive_call_to_location, lower_addr_primitive_call_to_word,
    lower_arg_count_raw_primitive_call_to_word, lower_arg_raw_primitive_call_to_value,
    lower_bool_normal_call, lower_call_arguments, lower_close_fd_raw_primitive_call,
    lower_copy_ptr_to_ptr_primitive_call, lower_copy_str_to_ptr_primitive_call,
    lower_direct_tail_call, lower_exit_raw_primitive_call, lower_fallible_void_normal_call,
    lower_from_ref_primitive_call_to_location, lower_from_ref_primitive_call_to_word,
    lower_i32_normal_call, lower_pointee_size_primitive_call_to_word,
    lower_slice_from_raw_parts_primitive_call_to_location, lower_slice_normal_call,
    lower_store_u8_to_ptr_primitive_call, lower_store_value_to_ptr_primitive_call,
    lower_str_bytes_primitive_call_to_location, lower_str_bytes_primitive_call_to_value,
    lower_str_from_raw_parts_primitive_call_to_location, lower_str_normal_call,
    lower_u8_normal_call, lower_usize_normal_call, lower_void_normal_call, primitive_addr_call,
    primitive_arg_count_raw_call, primitive_arg_raw_call, primitive_bytes_from_str_call,
    primitive_close_fd_raw_call, primitive_copy_ptr_to_ptr_call, primitive_copy_str_to_ptr_call,
    primitive_exit_raw_call, primitive_from_ref_call, primitive_pointee_size_call,
    primitive_slice_from_raw_parts_call, primitive_store_u8_to_ptr_call,
    primitive_store_value_to_ptr_call, primitive_str_from_raw_parts_call,
    primitive_write_bytes_raw_call, primitive_write_text_raw_call,
};
pub(super) use calls::{
    lower_fallible_bool_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
};
use predicates::{
    bool_comparison_contains_call, bool_comparison_needs_temporaries,
    expressions_are_lowerable_bool_comparison_operands, expressions_are_lowerable_bool_values,
    expressions_are_lowerable_usize_values, i32_comparison_needs_temporaries,
    is_i32_binary_operator, is_u8_binary_operator, is_usize_binary_operator,
    str_comparison_is_lowerable, u8_comparison_is_lowerable, usize_comparison_needs_temporaries,
};
pub(super) use predicates::{
    expression_contains_interpolated_string, expression_is_lowerable_bool_binding,
    short_circuit_bool_expression_needs_branch,
};
pub(super) use temporaries::TemporaryAllocator;
use temporaries::{
    LoweredI32Value, LoweredSliceValue, LoweredStrValue, LoweredU8Value, LoweredUsizeValue,
};

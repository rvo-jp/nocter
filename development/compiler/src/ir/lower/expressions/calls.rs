use super::super::aggregates::{
    ArrayInitializationProgress, PayloadInitializationProgress, StructInitializationProgress,
    aggregate_call_instruction, aggregate_type_layout, array_literal_requires_runtime_progress,
    lower_aggregate_array_literal_to_location_with_progress,
    lower_aggregate_array_literal_to_location_with_temporaries,
    lower_aggregate_struct_literal_to_location_at_offset_with_temporaries,
    lower_aggregate_struct_literal_to_location_with_temporaries,
    lower_payload_enum_constructor_to_location,
    lower_payload_enum_constructor_to_location_with_progress,
    payload_enum_constructor_member_and_arguments, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
};
use super::super::bindings::lower_aggregate_optional_otherwise_to_location;
use super::super::context::{
    AggregateDrop, AggregateFieldKind, ArrayElementDropState, LoweringContext,
    PayloadFieldDropState, StructFieldDropState,
};
use super::super::errors::lower_error_payload;
use super::super::functions::lower_aggregate_return_expression_to_location;
use super::super::outcome_propagation::propagating_outcome_mode;
use super::super::types::{
    borrow_inner_type_with_resolver, scalar_or_view_type_from_type_expr_with_resolver,
    view_element_type_from_type_expr,
};
use super::temporaries::TemporaryAllocator;
use super::{
    aggregate_member_field_kind_from_member, lower_aggregate_member_field_access,
    lower_bool_expression_to_value_with_temporaries, lower_catch_failure_mode,
    lower_i32_expression_to_value, lower_slice_expression_to_value, lower_str_expression_to_value,
    lower_u8_expression_to_value, lower_usize_expression_to_value,
    unavailable_call_target_diagnostic, usize_destination_reserved_abi_words,
};
use crate::abi::{
    ARGUMENT_REGISTER_COUNT, AbiType, ValueLayout, abi_value_from_type_expr_with_resolver,
};
use crate::ast::{BorrowExpr, CallExpr, Expr, IndexExpr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateArgument, AggregateArgumentSource, AggregateLocation, BoolLocation, BorrowArgument,
    BorrowSource, CallTarget, DirectAggregateArgument, I32Location, I32Value, Instruction,
    OutcomeFailureMode, ScalarArgument, SliceElementAddressKind, SliceElementIndex, SliceLocation,
    SliceValue, StrLocation, StrValue, Type, U8Location, UsizeLocation, UsizeValue,
};
use crate::typecheck::TypecheckSliceElementKind;

mod aggregate_arguments;
mod arguments;
mod borrow_arguments;
mod composed_outcomes;
mod evaluation;
mod normal_calls;
mod outcome_arguments;
mod pointer_drops;
mod pointer_takes;
mod primitives;
mod return_validation;
mod tail_calls;
mod utility;

use aggregate_arguments::{
    lower_aggregate_argument_source, lower_tracked_array_argument_source,
    lower_tracked_closure_argument_source, lower_tracked_interpolation_argument_source,
    lower_tracked_payload_argument_source, lower_tracked_spread_argument_source,
    lower_tracked_struct_argument_source,
};
pub(in crate::ir::lower) use arguments::lower_call_arguments_with_explicit_types;
pub(super) use arguments::{call_arguments_require_stack, lower_call_arguments};
pub(in crate::ir::lower) use borrow_arguments::lower_borrow_source_from_expression;
pub(in crate::ir::lower) use borrow_arguments::lower_borrow_source_from_expression_without_coercion;
use borrow_arguments::{
    lower_borrow_argument, lower_implicit_receiver_borrow_argument, materialize_slice_borrow_index,
};
pub(in crate::ir::lower) use composed_outcomes::lower_composed_outcome_call;
use evaluation::CallEvaluationContext;
pub(in crate::ir::lower) use normal_calls::lower_fallible_borrow_normal_call;
pub(super) use normal_calls::{
    lower_bool_normal_call, lower_borrow_normal_call, lower_fallible_void_normal_call,
    lower_i32_normal_call, lower_slice_normal_call, lower_str_normal_call, lower_u8_normal_call,
    lower_usize_normal_call, lower_void_normal_call,
};
pub(in crate::ir::lower) use normal_calls::{
    lower_fallible_bool_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
};
use outcome_arguments::lower_stored_outcome_argument;
pub(super) use pointer_drops::{
    lower_drop_value_at_ptr_primitive_call, primitive_drop_value_at_ptr_call,
};
pub(in crate::ir::lower) use pointer_takes::{
    PointerTakeDestination, lower_take_value_at_ptr_primitive_call,
    primitive_take_value_at_ptr_call,
};
pub(super) use primitives::{
    lower_addr_primitive_call_to_location, lower_addr_primitive_call_to_word,
    lower_arg_count_raw_primitive_call_to_word, lower_arg_raw_primitive_call_to_value,
    lower_close_fd_raw_primitive_call, lower_copy_ptr_to_ptr_primitive_call,
    lower_copy_str_to_ptr_primitive_call, lower_env_count_raw_primitive_call_to_word,
    lower_env_entry_raw_primitive_call_to_value, lower_exit_raw_primitive_call,
    lower_from_ref_primitive_call_to_location, lower_from_ref_primitive_call_to_word,
    lower_pointee_layout_primitive_call_to_word,
    lower_slice_from_raw_parts_primitive_call_to_location, lower_store_u8_to_ptr_primitive_call,
    lower_store_value_to_ptr_primitive_call, lower_str_bytes_primitive_call_to_location,
    lower_str_bytes_primitive_call_to_value, lower_str_from_raw_parts_primitive_call_to_location,
    lower_str_subview_primitive_call_to_location, primitive_addr_call,
    primitive_arg_count_raw_call, primitive_arg_raw_call, primitive_bytes_from_str_call,
    primitive_close_fd_raw_call, primitive_copy_ptr_to_ptr_call, primitive_copy_str_to_ptr_call,
    primitive_current_allocation_kind_call, primitive_current_allocation_state_call,
    primitive_env_count_raw_call, primitive_env_entry_raw_call, primitive_exit_raw_call,
    primitive_from_ref_call, primitive_open_read_raw_call, primitive_pointee_layout_call,
    primitive_read_bytes_raw_call, primitive_slice_from_raw_parts_call,
    primitive_store_u8_to_ptr_call, primitive_store_value_to_ptr_call,
    primitive_str_from_raw_parts_call, primitive_str_subview_call, primitive_write_bytes_raw_call,
    primitive_write_text_raw_call,
};
pub(in crate::ir::lower) use primitives::{
    lower_macos_syscall_primitive_call_to_location, lower_pointer_address_expression_to_word,
    primitive_trap_call,
};
use primitives::{
    lower_open_read_raw_primitive_call, lower_read_bytes_raw_primitive_call,
    lower_write_bytes_raw_primitive_call, lower_write_text_raw_primitive_call,
};
use return_validation::{
    describe_type, validate_bool_normal_call_return_type, validate_borrow_normal_call_return_type,
    validate_normal_call_return_type, validate_outcome_bool_normal_call_return_type,
    validate_outcome_borrow_normal_call_return_type, validate_outcome_i32_normal_call_return_type,
    validate_outcome_slice_normal_call_return_type, validate_outcome_str_normal_call_return_type,
    validate_outcome_u8_normal_call_return_type, validate_outcome_usize_normal_call_return_type,
    validate_outcome_void_normal_call_return_type, validate_slice_normal_call_return_type,
    validate_str_normal_call_return_type, validate_tail_call_return_type,
    validate_u8_normal_call_return_type, validate_usize_normal_call_return_type,
    validate_void_normal_call_return_type,
};
pub(super) use tail_calls::{is_tail_call_stack_pointer_argument, lower_direct_tail_call};
use utility::unwrap_group;

use super::bindings::lower_aggregate_optional_otherwise_to_location;
use super::context::{
    AggregateField, AggregateFieldKind, LoweringContext, SliceTypeInfo,
    aggregate_drop_for_type_expr_with_resolver_ref,
};
use super::expressions::{
    TemporaryAllocator, lower_aggregate_member_field_access, lower_bool_expression_to_value,
    lower_call_arguments_to_scalar_arguments_with_temporaries, lower_catch_failure_mode,
    lower_i32_expression_to_word, lower_macos_syscall_primitive_call_to_location,
    lower_slice_expression_to_value, lower_str_expression_to_value, lower_u8_expression_to_word,
    lower_usize_expression_to_word, push_store_slice_view_to_aggregate_field,
    push_store_str_view_to_aggregate_field,
};
use super::functions::propagating_failure_mode;
use super::literals::{
    lower_i8_literal, lower_i16_literal, lower_i64_literal, lower_u16_literal, lower_u32_literal,
    lower_u64_literal,
};
use super::types::view_element_type_from_type_expr_with_resolver;
use crate::abi::{
    AbiType, AbiValue, ValueLayout, abi_value_from_type_expr,
    abi_value_from_type_expr_with_resolver, array_element_stride, layout_of, layout_struct,
};
use crate::ast::{
    ArrayLiteralExpr, CallExpr, Expr, MemberExpr, StructLiteralExpr, TypeExpr, UnaryOperator,
    substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, CallTarget, FallibleFailureMode, Instruction, ScalarArgument, Type, U8Value,
    UsizeValue,
};
use crate::resolve::{ResolveOutput, StructFieldSignature, TypeSymbol, TypeSymbolKind};
use crate::source::SourceId;
use crate::typecheck::TypecheckSliceElementKind;
use std::collections::{HashMap, HashSet};

mod call_instructions;
mod copyability;
mod field_layouts;
mod field_values;
mod initialization;
mod literals;

pub(super) use call_instructions::*;
pub(super) use copyability::*;
pub(super) use field_layouts::*;
pub(super) use initialization::*;
pub(super) use literals::*;

use field_values::lower_aggregate_field_to_location;

pub(super) fn unsupported_aggregate_struct_literal_diagnostic(
    diagnostic_code: &'static str,
    subject: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!(
            "IR v0 can only lower aggregate {subject} from struct literals whose fields are supported scalar/view values, storage-only integer literals, nested struct literals, copy aggregate values, aggregate calls, or aggregate member values"
        ),
    )]
}

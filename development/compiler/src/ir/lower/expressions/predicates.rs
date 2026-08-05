use super::super::context::{AggregateFieldKind, LoweringContext};
use super::super::literals::{
    lower_i32_literal, lower_str_literal, lower_u8_literal, lower_usize_literal,
};
use super::super::types::{scalar_or_view_type_from_type_expr, view_element_type_from_type_expr};
use super::{primitive_current_allocation_kind_call, primitive_current_allocation_state_call};
use crate::ast::{
    BinaryExpr, BinaryOperator, CallExpr, Expr, InterpolatedStringPart, TypeConversionExpr,
    UnaryOperator,
};
use crate::ir::Type;
use crate::typecheck::TypecheckSliceElementKind;

mod effects;
mod lowerability;

pub(in crate::ir::lower) use effects::{
    expression_contains_call, short_circuit_bool_expression_needs_branch,
};
pub(in crate::ir::lower) use lowerability::expression_is_lowerable_bool_binding;
pub(super) use lowerability::{
    bool_comparison_contains_call, bool_comparison_needs_temporaries,
    expressions_are_lowerable_bool_comparison_operands, expressions_are_lowerable_bool_values,
    expressions_are_lowerable_usize_values, i32_comparison_needs_temporaries,
    is_i32_binary_operator, is_u8_binary_operator, is_usize_binary_operator,
    str_comparison_is_lowerable, str_comparison_needs_temporaries, u8_comparison_is_lowerable,
    usize_comparison_needs_temporaries,
};

use lowerability::*;

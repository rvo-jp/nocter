//! Type facts produced from the same environment and expression typing logic as
//! the checker.

use super::bindings::continuing_binding_type;
use super::calls::{
    infer_generic_substitutions, method_member_for_call,
    method_self_type_for_receiver_in_environment, resolved_call_signature, resolved_method_call,
};
use super::environments::{
    environment_for_catch, environment_for_collection_for_binding,
    environment_for_for_range_binding, environment_for_function, environment_for_if_is_binding,
    environment_for_interface_method, environment_for_literal,
    environment_for_literal_pack_binding, environment_for_method,
    environment_for_parameters_in_method_owner, environment_for_switch_arm,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::places::field_member_is_writable_place;
use super::structs::{
    resolved_struct_field_for_literal_field, resolved_struct_field_for_member,
    struct_literal_field_type, struct_member_type,
};
use super::type_expr::{
    canonical_type_expr, infer_type_expr_substitutions, type_expr_to_type_in_environment,
    type_expr_to_type_with_self_type, type_expr_to_type_with_substitutions,
};
use super::variants::{
    enum_variant_call_substitutions, resolved_enum_variant_for_call,
    resolved_enum_variant_for_member,
};
use crate::ast::{
    ArrayLength, ArrayType, AstFile, BindingStmt, Block, BorrowType, CallExpr, ConformanceDecl,
    ConformanceMember, Expr, FallibleType, GenericParamList, GenericType, IfIsStmt, InstanceDecl,
    InstanceMember, InterpolatedStringPart, Item, MemberExpr, MethodDecl, MethodOwnerDecl,
    MethodReceiverMode, OptionalType, Parameter, PointerType, Stmt, StructLiteralExpr,
    StructLiteralField, SwitchArm, SwitchPayloadBinding, TypeExpr, TypeReference, ViewType,
    substitute_type_expr_parameters,
};
use crate::integer::IntegerType;
use crate::resolve::{
    FunctionSignature, MethodSignature, ResolveOutput, SymbolKind, TypeSymbol, TypeSymbolKind,
};
use crate::source::ByteSpan;
use std::collections::{HashMap, HashSet};

mod callables;
mod collector;
mod hover_labels;
mod model;
mod specializations;
mod type_exprs;
mod utility;

#[cfg(test)]
mod tests;

pub(crate) use callables::{CallableCallFact, CallableCallSpecialization};
pub(crate) use collector::collect_typecheck_facts;
pub(crate) use model::{
    DropTypeSpecialization, FunctionCallSpecialization, GenericParameterFact,
    MethodCallSpecialization, TypeOccurrenceFact, TypeOccurrenceTarget, TypecheckClosurePlan,
    TypecheckCoercionPlan, TypecheckCollectionForPlan, TypecheckCollectionForSourceMode,
    TypecheckConversionKind, TypecheckConversionPlan, TypecheckFacts, TypecheckInterpolationPart,
    TypecheckInterpolationPlan, TypecheckIterationMethod, TypecheckMethodReceiverKind,
    TypecheckPayloadBindingMode, TypecheckScalarViewKind, TypecheckSequenceSpreadMode,
    TypecheckSequenceSpreadPlan, TypecheckSliceElementKind,
};
pub(super) use type_exprs::type_to_type_expr_allowing_parameters;

use callables::*;
use hover_labels::*;
pub(crate) use specializations::drop_type_specialization_from_self_ty;
use specializations::*;
use type_exprs::*;
use utility::*;

pub(crate) fn type_expr_presentation_label(ty: &TypeExpr, resolved: &ResolveOutput) -> String {
    type_label(ty, resolved, None)
}

pub(crate) fn type_symbol_presentation_label(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
) -> String {
    type_owner_hover_label(symbol, resolved).to_string()
}

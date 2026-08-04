//! Type facts produced from the same environment and expression typing logic as
//! the checker.

use super::bindings::continuing_binding_type;
use super::calls::{
    infer_generic_substitutions, method_member_for_call,
    method_self_type_for_receiver_in_environment, resolved_call_signature,
    resolved_method_for_call,
};
use super::environments::{
    environment_for_catch, environment_for_collection_for_binding,
    environment_for_for_range_binding, environment_for_function, environment_for_if_is_binding,
    environment_for_interface_method, environment_for_literal,
    environment_for_literal_pack_binding, environment_for_method,
    environment_for_parameters_in_impl, environment_for_switch_arm, function_self_type,
    impl_self_type,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::places::field_member_is_writable_place;
use super::structs::{
    resolved_struct_field_for_literal_field, resolved_struct_field_for_member,
    struct_literal_field_type, struct_member_type,
};
use super::type_expr::{
    infer_type_expr_substitutions, simple_type_from_display_name, type_expr_display_lossy,
    type_expr_to_type_in_environment, type_expr_to_type_with_self_type,
    type_expr_to_type_with_substitutions,
};
use super::variants::resolved_enum_variant_for_member;
use crate::ast::{
    ArrayLength, ArrayType, AstFile, BindingStmt, Block, BorrowType, CallExpr, EnumDecl,
    EnumVariant, Expr, FallibleType, GenericParamList, GenericType, IfIsStmt, ImplDecl, ImplMember,
    InterpolatedStringPart, Item, MemberExpr, MethodDecl, MethodReceiverMode, OptionalType,
    Parameter, PointerType, Stmt, StructDecl, StructField, StructLiteralExpr, StructLiteralField,
    SwitchArm, SwitchPayloadBinding, TypeAliasDecl, TypeExpr, TypeReference, ViewType,
    substitute_type_expr_parameters,
};
use crate::resolve::{
    AssociatedFunctionSignature, FunctionSignature, MethodSignature, ParameterSignature,
    ResolveOutput, SymbolKind, TypeSymbol, TypeSymbolKind,
};
use crate::source::ByteSpan;
use std::collections::{HashMap, HashSet};

mod collector;
mod hover_labels;
mod model;
mod specializations;
mod type_exprs;
mod utility;

#[cfg(test)]
mod tests;

pub(crate) use collector::collect_typecheck_facts;
pub(crate) use model::{
    DropTypeSpecialization, FunctionCallSpecialization, MethodCallSpecialization,
    TypeReferenceFact, TypecheckClosurePlan, TypecheckCollectionForPlan,
    TypecheckCollectionForSourceMode, TypecheckFacts, TypecheckInterpolationPart,
    TypecheckInterpolationPlan, TypecheckIterationMethod, TypecheckMethodReceiverKind,
    TypecheckPayloadBindingMode, TypecheckScalarViewKind, TypecheckSequenceSpreadMode,
    TypecheckSequenceSpreadPlan, TypecheckSliceElementKind,
};
pub(super) use type_exprs::type_to_type_expr_allowing_parameters;

use hover_labels::*;
use specializations::*;
use type_exprs::*;
use utility::*;

use super::member_presentation::{
    enum_variant_member_label, field_member_label, generic_type_owner_name,
};

pub(crate) fn type_expr_presentation_label(ty: &TypeExpr, resolved: &ResolveOutput) -> String {
    type_label(ty, resolved, None)
}

pub(crate) fn type_symbol_presentation_label(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
) -> String {
    type_owner_hover_label(symbol, resolved).to_string()
}

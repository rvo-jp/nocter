//! Callable ABI projection and MIR-backed entry points.

use super::context::{ErrorPayloads, FunctionNames, FunctionSignatures, ResolvedSources};
use super::parameter_slots::{
    AggregateParameterSource, LoweringAggregateParameter, LoweringOutcomeParameter,
    LoweringParameterSlots, ParameterStorage, SliceTypeInfo,
};
use super::types::{
    borrow_inner_type_with_resolver, borrow_type_from_type_expr,
    parameter_type_from_type_expr_with_resolver, return_type_from_type_expr_with_resolver,
    type_expr_with_self_type, view_element_type_from_type_expr_with_resolver,
};
use crate::abi::{
    AbiType, AbiValue, ValueClassification, abi_value_from_type_expr_with_resolver,
    function_parameter_abi_word_count_from_signature_with_resolver,
};
use crate::analysis::literal_specializations::{
    LiteralSpecialization, literal_element_parameter_name,
};
use crate::ast::{
    DestructDecl, FunctionDecl, LiteralDecl, LiteralShape, Parameter, TypeExpr, TypeReference,
    substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, CallTarget, Function, Instruction, Type};
use crate::outcomes::outcome_shape_with_resolver;
use crate::resolve::{
    FunctionSignature as ResolvedFunctionSignature, ParameterSignature, ResolveOutput,
};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{TypecheckSliceElementKind, TypedHir};
use std::collections::HashMap;

mod entrypoints;
pub(super) mod parameters;

pub(in crate::ir::lower) use entrypoints::*;
use parameters::*;

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

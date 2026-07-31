use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr_with_resolver,
    aggregate_type_layout, lower_aggregate_array_literal_to_location,
    lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_with_temporaries,
    lower_payload_enum_constructor_to_location, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
    type_expr_is_copy_aggregate_value_with_resolver,
};
use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{
    AggregateBorrowParameter, AggregateDrop, AggregateField, AggregateParameterSource,
    BorrowParameter, ErrorPayloads, FunctionNames, FunctionSignatures, LoweringAggregateParameter,
    LoweringContext, LoweringParameterSlots, PayloadEnumDrop, PayloadEnumDropField,
    PayloadEnumDropVariant, PendingAggregateDrop, ResolvedSources, SliceTypeInfo,
    aggregate_drop_for_type_expr_with_resolver,
};
use super::control_flow::{
    TerminalBranch, lower_nonterminal_for_range_statement, lower_nonterminal_if_statement,
    lower_nonterminal_if_statement_with_branch_prologues, lower_nonterminal_loop_statement,
    lower_nonterminal_payloadless_switch_body, lower_nonterminal_payloadless_switch_statement,
    lower_nonterminal_while_statement, lower_terminal_bool_if_statement_with_branch_prologues,
    lower_terminal_bool_switch_block, lower_terminal_branch_leading_statements,
    lower_terminal_condition, lower_terminal_i32_if_statement_with_branch_prologues,
    lower_terminal_i32_switch_block, lower_terminal_slice_if_statement_with_branch_prologues,
    lower_terminal_slice_switch_block, lower_terminal_str_if_statement_with_branch_prologues,
    lower_terminal_str_switch_block, lower_terminal_u8_if_statement_with_branch_prologues,
    lower_terminal_u8_switch_block, lower_terminal_usize_if_statement_with_branch_prologues,
    lower_terminal_usize_switch_block, lower_terminal_void_if_statement_with_branch_prologues,
    lower_terminal_void_switch_block, split_terminal_branch_block, statement_exits_function,
};
use super::errors::{ErrorPayload, lower_error_payload};
use super::expressions::{
    TemporaryAllocator, lower_aggregate_member_field_access, lower_bool_expression_to_location,
    lower_bool_return_expression, lower_call_arguments_to_scalar_arguments,
    lower_catch_failure_mode, lower_fallible_bool_normal_call, lower_fallible_i32_normal_call,
    lower_fallible_slice_normal_call, lower_fallible_str_normal_call,
    lower_fallible_u8_normal_call, lower_fallible_usize_normal_call,
    lower_i32_expression_to_location, lower_i32_return_expression,
    lower_macos_syscall_primitive_call_to_location, lower_never_return_expression,
    lower_slice_expression_to_location, lower_slice_return_expression,
    lower_str_expression_to_location, lower_str_return_expression, lower_u8_expression_to_location,
    lower_u8_return_expression, lower_usize_expression_to_location, lower_usize_return_expression,
    lower_void_expression_statement, mark_fallible_success_returns, success_return_instruction,
};
use super::types::{
    borrow_inner_type_with_resolver, borrow_type_from_type_expr,
    parameter_type_from_type_expr_with_resolver,
    return_type_expr_is_top_level_optional_with_resolver, return_type_from_type_expr_with_resolver,
    type_expr_with_self_type, view_element_type_from_type_expr_with_resolver,
};
use crate::abi::{
    AbiType, AbiValue, ValueClassification, ValueLayout, abi_value_from_type_expr_with_resolver,
    function_parameter_abi_word_count_from_signature_with_resolver, layout_of,
};
use crate::ast::{
    ArrayLiteralExpr, BinaryExpr, BinaryOperator, Block, CallExpr, DropDecl, DropStmt, Expr,
    FunctionDecl, IdentifierExpr, IfIsStmt, IfStmt, LiteralExpr, MemberExpr, MethodDecl, Parameter,
    ReturnStmt, Stmt, StructLiteralExpr, SwitchArm, SwitchPayloadPattern, SwitchStmt, TypeExpr,
    TypeReference, UnaryOperator, substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, BoolValue, BorrowArgument, BorrowSource, CallTarget,
    FallibleFailureMode, Function, I32ComparisonOperator, I32Location, I32Value, Instruction,
    ScalarArgument, SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value,
    UsizeLocation, UsizeValue,
};
use crate::resolve::{
    FunctionSignature as ResolvedFunctionSignature, ParameterSignature, ResolveOutput,
};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{TypecheckFacts, TypecheckSliceElementKind};
use std::collections::HashMap;

pub(super) fn lower_function<'a>(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    if !function
        .generics
        .parameters
        .iter()
        .all(|parameter| substitutions.contains_key(&parameter.name))
    {
        return Err(attach_primary_span_if_absent(
            vec![Diagnostic::error(
                "E8007",
                format!(
                    "IR v0 can only lower function `{}` with concrete generic arguments, got `{}`",
                    name, function.name
                ),
            )],
            sources,
            function.generics.span.unwrap_or(function.span),
        ));
    }

    let parameters = function_parameters(function, substitutions);
    let return_type_expr = substitute_type_expr_parameters(&function.return_type, substitutions);
    let parameter_slots = lower_scalar_parameters(
        &name,
        &parameters,
        root_source,
        resolved,
        &resolved_sources,
        sources,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, function.parameters.span)
    })?;
    validate_parameter_slots_match_function_abi(
        &name,
        &parameters,
        &return_type_expr,
        resolved,
        &resolved_sources,
        &parameter_slots,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, function.parameters.span)
    })?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameter_slots);
    let return_type =
        match lower_function_return_type(&return_type_expr, &name, resolved, &resolved_sources) {
            Ok(return_type) => return_type,
            Err(diagnostics) => {
                return Err(attach_primary_span_if_absent(
                    diagnostics,
                    sources,
                    function.return_type.span(),
                ));
            }
        };
    let success_type = return_type.success_type().clone();
    let mut context = LoweringContext::new(
        name.clone(),
        success_type,
        function_signatures,
        parameter_slots,
    )
    .with_function_return_type(return_type.clone())
    .with_function_return_type_expr(return_type_expr.clone())
    .with_function_returns_optional(return_type_expr_is_top_level_optional_with_resolver(
        &return_type_expr,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    ))
    .with_call_resolution(
        root_source,
        resolved,
        typecheck_facts,
        function_names,
        resolved_sources,
    )
    .with_generic_substitutions(substitutions.clone())
    .with_error_payloads(error_payloads);
    let mut instructions = parameter_setup;
    instructions.extend(lower_callable_body(
        &function.name,
        &function.body,
        &return_type,
        root_source,
        resolved,
        sources,
        &mut context,
    )?);

    Ok(Function {
        name,
        target,
        return_type,
        instructions,
    })
}

fn function_parameters(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
) -> Vec<Parameter> {
    function
        .parameters
        .parameters
        .iter()
        .map(|parameter| Parameter {
            span: parameter.span,
            name: parameter.name.clone(),
            name_span: parameter.name_span,
            ty: substitute_type_expr_parameters(&parameter.ty, substitutions),
        })
        .collect()
}

pub(super) fn lower_drop_function<'a>(
    drop_: &DropDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    let binding = Parameter {
        span: drop_.binding.span,
        name: drop_.binding.name.clone(),
        name_span: drop_.binding.name_span,
        ty: substitute_type_expr_parameters(
            &type_expr_with_self_type(&drop_.binding.ty, self_ty),
            substitutions,
        ),
    };
    let parameters = lower_scalar_parameters(
        &name,
        std::slice::from_ref(&binding),
        root_source,
        resolved,
        &resolved_sources,
        sources,
    )
    .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, binding.span))?;
    validate_parameter_slots_match_function_abi(
        &name,
        std::slice::from_ref(&binding),
        &void_type_expr(drop_.span),
        resolved,
        &resolved_sources,
        &parameters,
    )
    .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, binding.span))?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameters);
    let return_type = Type::Void;
    let mut context = LoweringContext::new(
        name.clone(),
        return_type.clone(),
        function_signatures,
        parameters,
    )
    .with_function_return_type(return_type.clone())
    .with_function_returns_optional(false)
    .with_call_resolution(
        root_source,
        resolved,
        typecheck_facts,
        function_names,
        resolved_sources,
    )
    .with_generic_substitutions(substitutions.clone())
    .with_error_payloads(error_payloads);
    let mut instructions = parameter_setup;
    instructions.extend(lower_callable_body(
        &name,
        &drop_.body,
        &return_type,
        root_source,
        resolved,
        sources,
        &mut context,
    )?);

    Ok(Function {
        name,
        target,
        return_type,
        instructions,
    })
}

pub(super) fn lower_method_function<'a>(
    method: &MethodDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    let Some(body) = &method.body else {
        return Err(attach_primary_span_if_absent(
            vec![Diagnostic::error(
                "E8007",
                format!("IR v0 can only lower method `{name}` with a body"),
            )],
            sources,
            method.span,
        ));
    };

    let parameters = method_parameters(method, self_ty, substitutions);
    let return_type_expr = type_expr_with_impl_substitutions(
        &type_expr_with_self_type(&method.return_type, self_ty),
        substitutions,
    );
    let parameter_slots = lower_scalar_parameters(
        &name,
        &parameters,
        root_source,
        resolved,
        &resolved_sources,
        sources,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, method.parameters.span)
    })?;
    validate_parameter_slots_match_function_abi(
        &name,
        &parameters,
        &return_type_expr,
        resolved,
        &resolved_sources,
        &parameter_slots,
    )
    .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, method.span))?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameter_slots);
    let return_type =
        match lower_function_return_type(&return_type_expr, &name, resolved, &resolved_sources) {
            Ok(return_type) => return_type,
            Err(diagnostics) => {
                return Err(attach_primary_span_if_absent(
                    diagnostics,
                    sources,
                    method.return_type.span(),
                ));
            }
        };
    let success_type = return_type.success_type().clone();
    let mut context = LoweringContext::new(
        name.clone(),
        success_type,
        function_signatures,
        parameter_slots,
    )
    .with_function_return_type(return_type.clone())
    .with_function_return_type_expr(return_type_expr.clone())
    .with_function_returns_optional(return_type_expr_is_top_level_optional_with_resolver(
        &return_type_expr,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    ))
    .with_call_resolution(
        root_source,
        resolved,
        typecheck_facts,
        function_names,
        resolved_sources,
    )
    .with_generic_substitutions(substitutions.clone())
    .with_error_payloads(error_payloads);
    let mut instructions = parameter_setup;
    instructions.extend(lower_callable_body(
        &name,
        body,
        &return_type,
        root_source,
        resolved,
        sources,
        &mut context,
    )?);

    Ok(Function {
        name,
        target,
        return_type,
        instructions,
    })
}

fn method_parameters(
    method: &MethodDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> Vec<Parameter> {
    let mut parameters = Vec::with_capacity(method.parameters.parameters.len() + 1);
    parameters.push(Parameter {
        span: method.receiver.span,
        name: method.receiver.name.clone(),
        name_span: method.receiver.name_span,
        ty: type_expr_with_impl_substitutions(
            &type_expr_with_self_type(&method.receiver.ty, self_ty),
            substitutions,
        ),
    });
    parameters.extend(
        method
            .parameters
            .parameters
            .iter()
            .map(|parameter| Parameter {
                span: parameter.span,
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                ty: type_expr_with_impl_substitutions(&parameter.ty, substitutions),
            }),
    );
    parameters
}

fn type_expr_with_impl_substitutions(
    ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> TypeExpr {
    substitute_type_expr_parameters(ty, substitutions)
}

fn lower_scalar_parameters(
    function_name: &str,
    parameters: &[Parameter],
    root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    sources: &SourceMap,
) -> Result<LoweringParameterSlots, Vec<Diagnostic>> {
    let mut slots = LoweringParameterSlots::default();
    for parameter in parameters {
        match lower_scalar_parameter_kind(
            parameter,
            function_name,
            root_source,
            resolved,
            resolved_sources,
        )
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, parameter.span)
        })? {
            ScalarParameterKind::I32 => {
                slots.push_i32_parameter(parameter.name.clone());
            }
            ScalarParameterKind::U8 => {
                slots.push_u8_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Usize => {
                slots.push_usize_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Bool => {
                slots.push_bool_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Str => {
                slots.push_str_parameter(parameter.name.clone());
                slots.push_empty_abi_word();
            }
            ScalarParameterKind::Slice(info) => {
                slots.push_slice_parameter(
                    parameter.name.clone(),
                    info.element_kind,
                    info.element_type,
                );
                slots.push_empty_abi_word();
            }
            ScalarParameterKind::Error => {
                slots.push_error_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Borrow {
                inner,
                is_readwrite,
            } => {
                let parameter_index = slots.reserve_empty_abi_words(1);
                slots.borrow_parameters.push(BorrowParameter {
                    name: parameter.name.clone(),
                    inner,
                    parameter_index,
                    is_readwrite,
                });
            }
            ScalarParameterKind::BorrowAggregate {
                layout,
                is_readwrite,
                fields,
            } => {
                let parameter_index = slots.reserve_empty_abi_words(1);
                slots.aggregate_borrows.push(AggregateBorrowParameter {
                    name: parameter.name.clone(),
                    layout,
                    parameter_index,
                    is_readwrite,
                    fields,
                });
            }
            ScalarParameterKind::AggregateIndirect { layout, fields } => {
                let parameter_index = slots.reserve_empty_abi_words(1);
                let slot_index = slots.aggregates.len();
                slots.aggregates.push(LoweringAggregateParameter {
                    name: parameter.name.clone(),
                    layout,
                    slot_index,
                    source: AggregateParameterSource::Indirect { parameter_index },
                    is_copy: type_expr_is_copy_aggregate_value_with_resolver(
                        &parameter.ty,
                        resolved,
                        |source| resolved_sources.get(&source).copied(),
                    ),
                    drop_kind: aggregate_drop_for_type_expr_with_resolver(
                        &parameter.ty,
                        root_source,
                        resolved,
                        |source| resolved_sources.get(&source).copied(),
                    ),
                    fields,
                });
            }
            ScalarParameterKind::AggregateDirect {
                layout,
                words,
                fields,
            } => {
                let start_index = slots.reserve_empty_abi_words(words);
                let slot_index = slots.aggregates.len();
                slots.aggregates.push(LoweringAggregateParameter {
                    name: parameter.name.clone(),
                    layout,
                    slot_index,
                    source: AggregateParameterSource::Direct { start_index, words },
                    is_copy: type_expr_is_copy_aggregate_value_with_resolver(
                        &parameter.ty,
                        resolved,
                        |source| resolved_sources.get(&source).copied(),
                    ),
                    drop_kind: aggregate_drop_for_type_expr_with_resolver(
                        &parameter.ty,
                        root_source,
                        resolved,
                        |source| resolved_sources.get(&source).copied(),
                    ),
                    fields,
                });
            }
        }
    }

    Ok(slots)
}

fn validate_parameter_slots_match_function_abi(
    function_name: &str,
    parameters: &[Parameter],
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    slots: &LoweringParameterSlots,
) -> Result<(), Vec<Diagnostic>> {
    let signature = resolved_function_signature(parameters, return_type.clone());
    let expected = function_parameter_abi_word_count_from_signature_with_resolver(
        &signature,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    )
    .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    let actual = slots.parameter_abi_word_count();
    if actual == expected {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 lowered parameters for function `{function_name}` into {actual} ABI words, but the resolved ABI expects {expected}"
        ),
    )])
}

fn resolved_function_signature(
    parameters: &[Parameter],
    return_type: TypeExpr,
) -> ResolvedFunctionSignature {
    ResolvedFunctionSignature {
        generic_parameters: Vec::new(),
        parameters: parameters
            .iter()
            .map(|parameter| ParameterSignature {
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                ty: parameter.ty.clone(),
            })
            .collect(),
        return_type,
    }
}

fn void_type_expr(span: ByteSpan) -> TypeExpr {
    TypeExpr::Reference(TypeReference {
        span,
        name: "void".to_string(),
    })
}

fn lower_aggregate_parameter_setup(parameters: &LoweringParameterSlots) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    for parameter in &parameters.aggregates {
        instructions.push(Instruction::ReserveAggregateSlot {
            slot_index: parameter.slot_index,
            layout: parameter.layout,
        });
        let source = match parameter.source {
            AggregateParameterSource::Indirect { parameter_index } => {
                AggregateLocation::Parameter(parameter_index)
            }
            AggregateParameterSource::Direct { start_index, .. } => {
                AggregateLocation::DirectParameter { start_index }
            }
        };
        instructions.push(Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(parameter.slot_index),
            source,
            layout: parameter.layout,
        });
    }
    instructions
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScalarParameterKind {
    I32,
    U8,
    Usize,
    Bool,
    Str,
    Slice(SliceTypeInfo),
    Error,
    Borrow {
        inner: Type,
        is_readwrite: bool,
    },
    BorrowAggregate {
        layout: crate::abi::ValueLayout,
        is_readwrite: bool,
        fields: Vec<super::context::AggregateField>,
    },
    AggregateIndirect {
        layout: crate::abi::ValueLayout,
        fields: Vec<super::context::AggregateField>,
    },
    AggregateDirect {
        layout: crate::abi::ValueLayout,
        words: usize,
        fields: Vec<super::context::AggregateField>,
    },
}

fn lower_scalar_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    match parameter_type_from_type_expr_with_resolver(&parameter.ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    }) {
        Some(Type::I32) => return Ok(ScalarParameterKind::I32),
        Some(Type::U8) => return Ok(ScalarParameterKind::U8),
        Some(Type::Usize) => return Ok(ScalarParameterKind::Usize),
        Some(Type::Bool) => return Ok(ScalarParameterKind::Bool),
        Some(Type::Str) => return Ok(ScalarParameterKind::Str),
        Some(Type::Slice { .. }) => {
            return Ok(ScalarParameterKind::Slice(slice_type_info_from_type_expr(
                &parameter.ty,
                resolved,
                resolved_sources,
            )));
        }
        Some(Type::Error) => return Ok(ScalarParameterKind::Error),
        _ => {}
    }

    let value = abi_value_from_type_expr_with_resolver(&parameter.ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    match &value.ty {
        AbiType::Borrow => lower_borrow_parameter_kind(
            parameter,
            function_name,
            root_source,
            resolved,
            resolved_sources,
        ),
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_) => {
            lower_aggregate_parameter_kind(
                parameter,
                function_name,
                root_source,
                resolved,
                resolved_sources,
                &value,
            )
        }
        _ => Err(unsupported_parameter_type_diagnostic(function_name)),
    }
}

fn slice_element_kind_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> TypecheckSliceElementKind
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match view_element_type_from_type_expr_with_resolver(ty, resolved, resolver) {
        Some(Type::I32) => TypecheckSliceElementKind::I32,
        Some(Type::U8) => TypecheckSliceElementKind::U8,
        Some(Type::Usize) => TypecheckSliceElementKind::Usize,
        Some(Type::Bool) => TypecheckSliceElementKind::Bool,
        Some(Type::Str) => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}

fn slice_type_info_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> SliceTypeInfo {
    slice_type_info_from_type_expr_with_resolver(ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
}

fn slice_type_info_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: F,
) -> SliceTypeInfo
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    SliceTypeInfo {
        element_kind: slice_element_kind_from_type_expr_with_resolver(ty, resolved, &resolver),
        element_type: view_element_type_expr_from_type_expr_with_resolver(ty, resolved, &resolver),
    }
}

fn view_element_type_expr_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow) => {
            let TypeExpr::View(view) = borrow.inner.as_ref() else {
                return None;
            };
            Some(*view.element.clone())
        }
        TypeExpr::Reference(reference) => {
            let source_resolved = resolver(ty.span().source).unwrap_or(resolved);
            let symbol = source_resolved
                .type_symbol_by_reference_name(&reference.name)
                .or_else(|| resolved.type_symbol_by_reference_name(&reference.name))?;
            let target = symbol.alias_target.as_ref()?;
            let target_resolved = resolver(target.span().source).unwrap_or(source_resolved);
            view_element_type_expr_from_type_expr_with_resolver(target, target_resolved, resolver)
        }
        _ => None,
    }
}

fn lower_borrow_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let Some(borrow) = borrow_type_from_type_expr(&parameter.ty, resolved) else {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    };
    match borrow_inner_type_with_resolver(&borrow.inner, resolved, |source| {
        resolved_sources.get(&source).copied()
    }) {
        Some(Type::Aggregate { .. } | Type::DirectAggregate { .. }) => {
            lower_aggregate_borrow_parameter_kind(
                parameter,
                function_name,
                root_source,
                resolved,
                resolved_sources,
            )
        }
        Some(inner) => Ok(ScalarParameterKind::Borrow {
            inner,
            is_readwrite: borrow.is_readwrite,
        }),
        None => Err(unsupported_parameter_type_diagnostic(function_name)),
    }
}

fn lower_aggregate_borrow_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let Some(borrow) = borrow_type_from_type_expr(&parameter.ty, resolved) else {
        unreachable!("aggregate borrow parameter lowering requires a borrow type");
    };
    let value = abi_value_from_type_expr_with_resolver(&borrow.inner, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    if !matches!(value.ty, AbiType::Struct(_)) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    let fields = aggregate_fields_from_type_expr_with_resolver(
        &borrow.inner,
        root_source,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    )
    .ok_or_else(|| unsupported_parameter_type_diagnostic(function_name))?;
    Ok(ScalarParameterKind::BorrowAggregate {
        layout: value.layout,
        is_readwrite: borrow.is_readwrite,
        fields,
    })
}

fn lower_aggregate_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    value: &AbiValue,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    if !matches!(
        value.ty,
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_)
    ) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    let fields = aggregate_fields_from_type_expr_with_resolver(
        &parameter.ty,
        root_source,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    )
    .ok_or_else(|| unsupported_parameter_type_diagnostic(function_name))?;
    match value.classification {
        ValueClassification::Indirect => Ok(ScalarParameterKind::AggregateIndirect {
            layout: value.layout,
            fields,
        }),
        ValueClassification::Direct { words } => Ok(ScalarParameterKind::AggregateDirect {
            layout: value.layout,
            words,
            fields,
        }),
    }
}

fn unsupported_parameter_type_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower `i32`, `u8`, `usize`, `bool`, `&str`, `&[T]`, `&+[T]`, scalar borrow parameters, aggregate borrow parameters, and supported aggregate value parameters for function `{function_name}`"
        ),
    )]
}

fn lower_function_return_type(
    ty: &TypeExpr,
    name: &str,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Result<Type, Vec<Diagnostic>> {
    return_type_from_type_expr_with_resolver(ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .ok_or_else(|| unsupported_function_return_type_diagnostic(name))
}

fn unsupported_function_return_type_diagnostic(name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower function `{name}` return type `i32`, `u8`, `usize`, `bool`, `&str`, `&[T]`, `&+[T]`, `void`, `never`, aggregates, or a fallible form of those types"
        ),
    )]
}

fn lower_callable_body(
    function_name: &str,
    body: &Block,
    return_type: &Type,
    _root_source: SourceId,
    resolved: &ResolveOutput,
    sources: &SourceMap,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();
    let original_statements = body.statements.as_slice();

    if original_statements.iter().all(statement_is_import)
        && body.result.is_none()
        && *success_type == Type::Void
    {
        return Ok(vec![success_return_instruction(return_type)]);
    }

    let (statements, body_result) =
        reachable_body_prefix(original_statements, body.result.as_deref(), context);

    if let Some(result) = body_result {
        let mut instructions = lower_leading_bindings(statements, context, sources)?;
        instructions.extend(lower_callable_body_result(
            function_name,
            result,
            return_type,
            context,
            sources,
        )?);
        return Ok(instructions);
    }

    if success_type == &Type::Void
        && statements
            .iter()
            .rev()
            .find(|statement| !statement_is_import(statement))
            .is_some_and(statement_allows_implicit_void_return)
    {
        let mut instructions = lower_leading_bindings(statements, context, sources)?;
        instructions.extend(append_scope_end_drops_before_exit(
            vec![success_return_instruction(return_type)],
            context,
        )?);
        return Ok(instructions);
    }

    let Some((last, leading)) = statements.split_last() else {
        return Err(attach_primary_span_if_absent(
            unsupported_function_body_diagnostic(function_name),
            sources,
            body.span,
        ));
    };
    let mut instructions = lower_leading_bindings(leading, context, sources)?;

    match last {
        Stmt::Return(statement) => {
            let return_instructions = lower_terminal_return_statement_with_scope_drops(
                statement,
                context,
                "E8007",
                "functions",
                sources,
            )
            .map_err(|diagnostics| {
                let span = statement
                    .expression
                    .as_ref()
                    .map_or(statement.span, |expression| expression.span());
                attach_primary_span_if_absent(diagnostics, sources, span)
            })?;
            instructions.extend(return_instructions);
            Ok(instructions)
        }
        Stmt::If(statement) => {
            let Some(branch_instructions) = lower_terminal_if_statement_for_success_type(
                statement,
                context,
                function_name,
                return_type,
                "E8007",
                "functions",
                resolved,
                sources,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, "E8007").map_err(
                |diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                },
            )?;
            let Some(branch_instructions) =
                lower_terminal_if_statement_for_success_type_with_branch_prologues(
                    &if_is.statement,
                    context,
                    &if_is.then_prologue,
                    &BranchPrologue::empty(),
                    function_name,
                    return_type,
                    "E8007",
                    "functions",
                    resolved,
                    sources,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(if_is.leading_instructions);
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::Switch(statement) => {
            let switch = tag_only_switch_as_control_flow(statement, context, "E8007").map_err(
                |diagnostics| attach_primary_span_if_absent(diagnostics, sources, statement.span),
            )?;
            let Some(branch_instructions) = lower_terminal_payloadless_switch_for_success_type(
                switch,
                context,
                function_name,
                return_type,
                "E8007",
                "functions",
                resolved,
                sources,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?
            else {
                return Err(attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(branch_instructions);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_expression_with_scope_drops(
                &statement.expression,
                context,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.expression.span())
            })?
            else {
                if success_type == &Type::Void
                    && let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, context).map_err(
                            |diagnostics| {
                                attach_primary_span_if_absent(
                                    diagnostics,
                                    sources,
                                    statement.expression.span(),
                                )
                            },
                        )?
                {
                    instructions.extend(void_instructions);
                    mark_explicit_moves_in_expression(&statement.expression, context);
                    instructions.extend(append_scope_end_drops_before_exit(
                        vec![success_return_instruction(return_type)],
                        context,
                    )?);
                    return Ok(instructions);
                }

                return Err(attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    statement.span,
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        Stmt::Loop(statement) => {
            instructions.extend(
                lower_nonterminal_loop_statement(statement, context, "E8007", "functions", sources)
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
            );
            Ok(instructions)
        }
        _ => Err(attach_primary_span_if_absent(
            unsupported_function_body_diagnostic(function_name),
            sources,
            last.span(),
        )),
    }
}

fn lower_callable_body_result(
    function_name: &str,
    expression: &Expr,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(instructions) = lower_callable_control_body_result(
        function_name,
        expression,
        return_type,
        context,
        sources,
    )? {
        return Ok(instructions);
    }

    if return_type.success_type() == &Type::Void {
        if let Some(terminating_instructions) =
            lower_never_expression_with_scope_drops(expression, context).map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, expression.span())
            })?
        {
            return Ok(terminating_instructions);
        }

        if let Some(mut void_instructions) = lower_void_expression_statement(expression, context)
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, expression.span())
            })?
        {
            mark_explicit_moves_in_expression(expression, context);
            void_instructions.extend(append_scope_end_drops_before_exit(
                vec![success_return_instruction(return_type)],
                context,
            )?);
            return Ok(void_instructions);
        }
    }

    let statement = ReturnStmt {
        span: expression.span(),
        expression: Some(expression.clone()),
    };
    lower_return_statement_with_scope_drops(&statement, context, "E8007")
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, expression.span())
        })
        .map_err(|diagnostics| {
            if diagnostics.is_empty() {
                attach_primary_span_if_absent(
                    unsupported_function_body_diagnostic(function_name),
                    sources,
                    expression.span(),
                )
            } else {
                diagnostics
            }
        })
}

fn lower_callable_control_body_result(
    function_name: &str,
    expression: &Expr,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => {
            lower_callable_if_body_result(statement, function_name, return_type, context, sources)
        }
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, "E8007")?;
            lower_callable_if_body_result_with_branch_prologues(
                &if_is.statement,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                function_name,
                return_type,
                context,
                sources,
            )
            .map(|result| {
                result.map(|branch_instructions| {
                    let mut instructions = if_is.leading_instructions;
                    instructions.extend(branch_instructions);
                    instructions
                })
            })
        }
        Expr::Match(statement) => {
            let switch = tag_only_switch_as_control_flow(statement, context, "E8007")?;
            lower_callable_payloadless_switch_body_result(
                switch,
                function_name,
                return_type,
                context,
                sources,
            )
        }
        _ => Ok(None),
    }
}

fn lower_callable_payloadless_switch_body_result(
    switch: LoweredPayloadlessSwitch,
    function_name: &str,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let resolved = context
        .resolved_calls()
        .map(|(_, resolved)| resolved)
        .ok_or_else(|| unsupported_function_body_diagnostic(function_name))?;
    match lower_terminal_payloadless_switch_body_for_success_type(
        switch.body.clone(),
        context,
        function_name,
        return_type,
        "E8007",
        "functions",
        resolved,
        sources,
    ) {
        Ok(Some(mut branch_instructions)) => {
            let mut instructions = switch.leading_instructions;
            instructions.append(&mut branch_instructions);
            Ok(Some(mark_fallible_success_returns(
                return_type,
                instructions,
            )))
        }
        Ok(None) => Ok(None),
        Err(_) if return_type.success_type() == &Type::Void => Ok(Some(
            lower_void_nonterminal_callable_payloadless_switch_body_result(
                switch,
                return_type,
                context,
                sources,
            )?,
        )),
        Err(diagnostics) => Err(diagnostics),
    }
}

fn lower_void_nonterminal_callable_payloadless_switch_body_result(
    switch: LoweredPayloadlessSwitch,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = switch.leading_instructions;
    instructions.extend(lower_nonterminal_payloadless_switch_body(
        switch.body,
        context,
        None,
        &[],
        "E8007",
        "functions",
        sources,
    )?);
    instructions.extend(append_scope_end_drops_before_exit(
        vec![success_return_instruction(return_type)],
        context,
    )?);
    Ok(instructions)
}

pub(super) fn lower_terminal_return_statement_with_scope_drops(
    statement: &ReturnStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(expression) = &statement.expression
        && let Some(instructions) = lower_terminal_control_return_expression(
            expression,
            context,
            diagnostic_code,
            subject,
            sources,
        )?
    {
        return Ok(instructions);
    }

    lower_return_statement_with_scope_drops(statement, context, diagnostic_code)
}

fn lower_terminal_control_return_expression(
    expression: &Expr,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let return_type = context.function_return_type().clone();
    let function_name = context.function_name().to_string();
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_if_statement_for_success_type(
            statement,
            context,
            &function_name,
            &return_type,
            diagnostic_code,
            subject,
            context
                .resolved_calls()
                .map(|(_, resolved)| resolved)
                .ok_or_else(|| unsupported_function_body_diagnostic(&function_name))?,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, diagnostic_code)?;
            lower_terminal_if_statement_for_success_type_with_branch_prologues(
                &if_is.statement,
                context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                &function_name,
                &return_type,
                diagnostic_code,
                subject,
                context
                    .resolved_calls()
                    .map(|(_, resolved)| resolved)
                    .ok_or_else(|| unsupported_function_body_diagnostic(&function_name))?,
                sources,
            )
            .map(|result| {
                result.map(|branch_instructions| {
                    let mut instructions = if_is.leading_instructions;
                    instructions.extend(branch_instructions);
                    instructions
                })
            })
        }
        Expr::Match(statement) => {
            let switch = tag_only_switch_as_control_flow(statement, context, diagnostic_code)?;
            lower_terminal_payloadless_switch_for_success_type(
                switch,
                context,
                &function_name,
                &return_type,
                diagnostic_code,
                subject,
                context
                    .resolved_calls()
                    .map(|(_, resolved)| resolved)
                    .ok_or_else(|| unsupported_function_body_diagnostic(&function_name))?,
                sources,
            )
        }
        _ => Ok(None),
    }
}

fn lower_callable_if_body_result(
    statement: &IfStmt,
    function_name: &str,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    lower_callable_if_body_result_with_branch_prologues(
        statement,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        function_name,
        return_type,
        context,
        sources,
    )
}

fn lower_callable_if_body_result_with_branch_prologues(
    statement: &IfStmt,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    function_name: &str,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let resolved = context
        .resolved_calls()
        .map(|(_, resolved)| resolved)
        .ok_or_else(|| unsupported_function_body_diagnostic(function_name))?;
    match lower_terminal_if_statement_for_success_type_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        function_name,
        return_type,
        "E8007",
        "functions",
        resolved,
        sources,
    ) {
        Ok(instructions) => Ok(instructions),
        Err(_) if return_type.success_type() == &Type::Void => Ok(Some(
            lower_void_nonterminal_callable_if_body_result_with_branch_prologues(
                statement,
                then_prologue,
                else_prologue,
                return_type,
                context,
                sources,
            )?,
        )),
        Err(diagnostics) => Err(diagnostics),
    }
}

fn lower_void_nonterminal_callable_if_body_result_with_branch_prologues(
    statement: &IfStmt,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    return_type: &Type,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = lower_nonterminal_if_statement_with_branch_prologues(
        statement,
        context,
        then_prologue,
        else_prologue,
        None,
        &[],
        "E8007",
        "functions",
        sources,
    )?;
    instructions.extend(append_scope_end_drops_before_exit(
        vec![success_return_instruction(return_type)],
        context,
    )?);
    Ok(instructions)
}

fn lower_terminal_if_statement_for_success_type(
    statement: &IfStmt,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    lower_terminal_if_statement_for_success_type_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        function_name,
        return_type,
        diagnostic_code,
        subject,
        resolved,
        sources,
    )
}

fn lower_terminal_if_statement_for_success_type_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(branch_instructions) =
        lower_terminal_if_statement_body_for_success_type_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            function_name,
            return_type,
            diagnostic_code,
            subject,
            resolved,
            sources,
        )?
    else {
        return Ok(None);
    };

    Ok(Some(mark_fallible_success_returns(
        return_type,
        branch_instructions,
    )))
}

fn lower_terminal_if_statement_body_for_success_type_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();
    let branch_instructions = match success_type {
        Type::I32 => lower_terminal_i32_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Bool => lower_terminal_bool_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::U8 => lower_terminal_u8_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Usize => lower_terminal_usize_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Str => lower_terminal_str_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Slice { .. } => lower_terminal_slice_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Void => lower_terminal_void_if_statement_with_branch_prologues(
            statement,
            context,
            then_prologue,
            else_prologue,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
            lower_terminal_aggregate_if_statement_with_branch_prologues(
                statement,
                context,
                then_prologue,
                else_prologue,
                success_type,
                function_name,
                resolved,
                sources,
            )?
        }
        Type::Never | Type::Fallible(_) | Type::Borrow { .. } | Type::Error => return Ok(None),
    };

    Ok(Some(branch_instructions))
}

fn lower_terminal_payloadless_switch_for_success_type(
    switch: LoweredPayloadlessSwitch,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(branch_instructions) = lower_terminal_payloadless_switch_body_for_success_type(
        switch.body,
        context,
        function_name,
        return_type,
        diagnostic_code,
        subject,
        resolved,
        sources,
    )?
    else {
        return Ok(None);
    };

    let mut instructions = switch.leading_instructions;
    instructions.extend(branch_instructions);
    Ok(Some(mark_fallible_success_returns(
        return_type,
        instructions,
    )))
}

fn lower_terminal_payloadless_switch_body_for_success_type(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => {
            lower_terminal_switch_block_for_success_type(
                block,
                context,
                function_name,
                return_type,
                diagnostic_code,
                subject,
                resolved,
                sources,
            )
        }
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_switch_condition_for_success_type(
                condition,
                context,
                function_name,
                return_type,
                diagnostic_code,
                subject,
                resolved,
                sources,
            )
        }
    }
}

fn lower_terminal_switch_condition_for_success_type(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(then_instructions) = lower_terminal_switch_block_for_success_type(
        condition.then_branch,
        context,
        function_name,
        return_type,
        diagnostic_code,
        subject,
        resolved,
        sources,
    )?
    else {
        return Ok(None);
    };
    let Some(else_instructions) = lower_terminal_payloadless_switch_body_for_success_type(
        *condition.else_body,
        context,
        function_name,
        return_type,
        diagnostic_code,
        subject,
        resolved,
        sources,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        diagnostic_code,
        sources,
    )?))
}

fn lower_terminal_switch_block_for_success_type(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    function_name: &str,
    return_type: &Type,
    diagnostic_code: &'static str,
    subject: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let branch_instructions = match return_type.success_type() {
        Type::I32 => lower_terminal_i32_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Bool => lower_terminal_bool_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::U8 => lower_terminal_u8_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Usize => lower_terminal_usize_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Str => lower_terminal_str_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Slice { .. } => lower_terminal_slice_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Void => lower_terminal_void_switch_block(
            block,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
            lower_terminal_aggregate_switch_block(
                block,
                context,
                return_type.success_type(),
                function_name,
                resolved,
                sources,
            )?
        }
        _ => return Ok(None),
    };
    Ok(Some(branch_instructions))
}

pub(super) fn payloadless_if_is_as_if_statement(
    statement: &IfIsStmt,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<IfStmt, Vec<Diagnostic>> {
    if statement.payload.is_some() {
        return Err(unsupported_if_is_diagnostic(diagnostic_code));
    }

    let variant = payloadless_if_is_variant_expression(statement);
    let Expr::Member(member) = &variant else {
        unreachable!("payloadless if-is variant expression must be a member expression");
    };
    if context.payloadless_enum_variant_tag(member).is_none() {
        return Err(unsupported_if_is_diagnostic(diagnostic_code));
    }

    Ok(IfStmt {
        span: statement.span,
        condition: Expr::Binary(BinaryExpr {
            span: statement.pattern_span,
            left: Box::new(statement.expression.clone()),
            operator: BinaryOperator::Equal,
            operator_span: statement.pattern_span,
            right: Box::new(variant),
        }),
        then_block: statement.then_block.clone(),
        else_block: statement.else_block.clone(),
    })
}

pub(super) struct LoweredTagOnlyIfIs {
    pub(super) leading_instructions: Vec<Instruction>,
    pub(super) statement: IfStmt,
    pub(super) then_prologue: BranchPrologue,
}

#[derive(Clone)]
pub(super) struct BranchPrologue {
    bindings: Vec<BranchPrologueBinding>,
}

impl BranchPrologue {
    pub(super) fn empty() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    fn single_binding(binding: BranchPrologueBinding) -> Self {
        Self {
            bindings: vec![binding],
        }
    }

    pub(super) fn apply(
        &self,
        context: &mut LoweringContext,
    ) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
        let mut instructions = Vec::new();
        for binding in &self.bindings {
            instructions.extend(binding.lower(context)?);
        }
        Ok(instructions)
    }
}

#[derive(Clone)]
struct BranchPrologueBinding {
    name: String,
    source_slot: usize,
    payload_offset: u32,
    kind: BranchPrologueBindingKind,
    diagnostic_code: &'static str,
}

#[derive(Clone)]
enum BranchPrologueBindingKind {
    ScalarOrStrView(AbiType),
    SliceView(SliceTypeInfo),
    CopyAggregate {
        layout: ValueLayout,
        fields: Vec<AggregateField>,
    },
}

impl BranchPrologueBinding {
    fn lower(&self, context: &mut LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
        let source = AggregateLocation::Slot(self.source_slot);
        match &self.kind {
            BranchPrologueBindingKind::CopyAggregate { layout, fields } => {
                let slot_index = context.define_aggregate_local(
                    self.name.clone(),
                    *layout,
                    true,
                    None,
                    fields.clone(),
                );
                Ok(vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index,
                        layout: *layout,
                    },
                    Instruction::CopyAggregateRange {
                        destination: AggregateLocation::Slot(slot_index),
                        destination_offset: 0,
                        source,
                        source_offset: self.payload_offset,
                        layout: *layout,
                    },
                ])
            }
            BranchPrologueBindingKind::SliceView(info) => {
                let destination = context.next_slice_local_location()?;
                let SliceLocation::Local(index) = destination else {
                    unreachable!("local slice binding locations are local pairs");
                };
                let instructions = payload_view_binding_loads(
                    index,
                    source,
                    self.payload_offset,
                    self.diagnostic_code,
                )?;
                context.define_slice_local(
                    self.name.clone(),
                    info.element_kind,
                    info.element_type.clone(),
                );
                Ok(instructions)
            }
            BranchPrologueBindingKind::ScalarOrStrView(payload_type) => match payload_type {
                AbiType::I32 => {
                    let destination = context.next_i32_local_location()?;
                    context.define_i32_local(self.name.clone());
                    Ok(vec![Instruction::LoadAggregateI32 {
                        destination,
                        source,
                        offset: self.payload_offset,
                    }])
                }
                AbiType::U8 => {
                    let destination = context.next_u8_local_location()?;
                    context.define_u8_local(self.name.clone());
                    Ok(vec![Instruction::LoadAggregateU8 {
                        destination,
                        source,
                        offset: self.payload_offset,
                    }])
                }
                AbiType::Usize => {
                    let destination = context.next_usize_local_location()?;
                    context.define_usize_local(self.name.clone());
                    Ok(vec![Instruction::LoadAggregateUsize {
                        destination,
                        source,
                        offset: self.payload_offset,
                    }])
                }
                AbiType::Bool => {
                    let destination = context.next_bool_local_location()?;
                    context.define_bool_local(self.name.clone());
                    Ok(vec![Instruction::LoadAggregateBool {
                        destination,
                        source,
                        offset: self.payload_offset,
                    }])
                }
                AbiType::StrView => {
                    let destination = context.next_str_local_location()?;
                    let StrLocation::Local(index) = destination else {
                        unreachable!("local str binding locations are local pairs");
                    };
                    let instructions = payload_view_binding_loads(
                        index,
                        source,
                        self.payload_offset,
                        self.diagnostic_code,
                    )?;
                    context.define_str_local(self.name.clone());
                    Ok(instructions)
                }
                _ => Err(unsupported_if_is_diagnostic(self.diagnostic_code)),
            },
        }
    }
}

fn payload_view_binding_loads(
    index: usize,
    source: AggregateLocation,
    payload_offset: u32,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let len_index = index.checked_add(1).ok_or_else(|| {
        payload_binding_overflow_diagnostic(
            diagnostic_code,
            "IR v0 cannot lower payload enum bindings with overflowing local indexes",
        )
    })?;
    let len_offset = payload_offset.checked_add(8).ok_or_else(|| {
        payload_binding_overflow_diagnostic(
            diagnostic_code,
            "IR v0 cannot lower payload enum bindings with overflowing payload offsets",
        )
    })?;
    Ok(vec![
        Instruction::LoadAggregateUsize {
            destination: UsizeLocation::Local(index),
            source,
            offset: payload_offset,
        },
        Instruction::LoadAggregateUsize {
            destination: UsizeLocation::Local(len_index),
            source,
            offset: len_offset,
        },
    ])
}

fn payload_binding_overflow_diagnostic(
    diagnostic_code: &'static str,
    message: &'static str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(diagnostic_code, message)]
}

pub(super) fn tag_only_if_is_as_control_flow(
    statement: &IfIsStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredTagOnlyIfIs, Vec<Diagnostic>> {
    let variant = payloadless_if_is_variant_expression(statement);
    let Expr::Member(member) = &variant else {
        unreachable!("if-is variant expression must be a member expression");
    };

    if context.payloadless_enum_variant_tag(member).is_some() {
        return payloadless_if_is_as_if_statement(statement, context, diagnostic_code).map(
            |statement| LoweredTagOnlyIfIs {
                leading_instructions: Vec::new(),
                statement,
                then_prologue: BranchPrologue::empty(),
            },
        );
    }

    let tag = context
        .enum_variant_tag(member)
        .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    let payload_len = context
        .enum_variant_payload_len(member)
        .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    if !tag_only_if_is_payload_pattern_is_supported(statement.payload.as_ref(), payload_len) {
        return Err(unsupported_if_is_diagnostic(diagnostic_code));
    }
    if !expression_is_payload_enum_aggregate_value(&statement.expression, context) {
        return Err(unsupported_if_is_diagnostic(diagnostic_code));
    }
    let source_slot = tag_only_if_is_aggregate_source_slot(&statement.expression, context)
        .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    let then_prologue =
        tag_only_if_is_then_prologue(statement, source_slot, context, diagnostic_code)?;
    let target_name = tag_only_if_is_target_name(statement);
    let target = context.next_u8_local_location()?;
    context.define_u8_local(target_name.clone());

    Ok(LoweredTagOnlyIfIs {
        leading_instructions: vec![Instruction::LoadAggregateU8 {
            destination: target,
            source: AggregateLocation::Slot(source_slot),
            offset: 0,
        }],
        statement: IfStmt {
            span: statement.span,
            condition: Expr::Binary(BinaryExpr {
                span: statement.pattern_span,
                left: Box::new(Expr::Identifier(IdentifierExpr {
                    span: statement.expression.span(),
                    name: target_name,
                })),
                operator: BinaryOperator::Equal,
                operator_span: statement.pattern_span,
                right: Box::new(Expr::IntegerLiteral(LiteralExpr {
                    span: statement.variant_name_span,
                    value: tag.to_string(),
                })),
            }),
            then_block: statement.then_block.clone(),
            else_block: statement.else_block.clone(),
        },
        then_prologue,
    })
}

pub(super) struct LoweredPayloadlessSwitch {
    pub(super) leading_instructions: Vec<Instruction>,
    pub(super) body: LoweredPayloadlessSwitchBody,
}

#[derive(Clone)]
pub(super) enum LoweredPayloadlessSwitchBody {
    Direct(LoweredSwitchBlock),
    Conditional(LoweredSwitchCondition),
}

#[derive(Clone)]
pub(super) struct LoweredSwitchBlock {
    pub(super) block: Block,
    pub(super) prologue: BranchPrologue,
}

#[derive(Clone)]
pub(super) struct LoweredSwitchCondition {
    pub(super) condition: Expr,
    pub(super) then_branch: LoweredSwitchBlock,
    pub(super) else_body: Box<LoweredPayloadlessSwitchBody>,
}

pub(super) fn payloadless_switch_as_control_flow(
    statement: &SwitchStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredPayloadlessSwitch, Vec<Diagnostic>> {
    let Some(variant_names) = payloadless_switch_variant_names(statement, context) else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };

    let target_name = payloadless_switch_target_name(statement);
    let target = context.next_u8_local_location()?;
    context.define_u8_local(target_name.clone());
    let leading_instructions =
        lower_u8_expression_to_location(&statement.expression, target, context)?;
    let target_expression = Expr::Identifier(IdentifierExpr {
        span: statement.expression.span(),
        name: target_name,
    });
    let body = payloadless_switch_body(
        statement,
        target_expression,
        &variant_names,
        diagnostic_code,
    )?;

    Ok(LoweredPayloadlessSwitch {
        leading_instructions,
        body,
    })
}

pub(super) fn tag_only_switch_as_control_flow(
    statement: &SwitchStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredPayloadlessSwitch, Vec<Diagnostic>> {
    if let Ok(switch) = payloadless_switch_as_control_flow(statement, context, diagnostic_code) {
        return Ok(switch);
    }

    let Some(variant_names) = payload_enum_tag_only_switch_variant_names(statement, context) else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };
    if !expression_is_payload_enum_aggregate_value(&statement.expression, context) {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    }
    let source_slot = tag_only_if_is_aggregate_source_slot(&statement.expression, context)
        .ok_or_else(|| unsupported_switch_diagnostic(diagnostic_code))?;

    let target_name = payloadless_switch_target_name(statement);
    let target = context.next_u8_local_location()?;
    context.define_u8_local(target_name.clone());
    let target_expression = Expr::Identifier(IdentifierExpr {
        span: statement.expression.span(),
        name: target_name,
    });
    let body = tag_only_switch_body(
        statement,
        target_expression,
        &variant_names,
        source_slot,
        context,
        diagnostic_code,
    )?;

    Ok(LoweredPayloadlessSwitch {
        leading_instructions: vec![Instruction::LoadAggregateU8 {
            destination: target,
            source: AggregateLocation::Slot(source_slot),
            offset: 0,
        }],
        body,
    })
}

fn payloadless_if_is_variant_expression(statement: &IfIsStmt) -> Expr {
    Expr::Member(MemberExpr {
        span: statement.pattern_span,
        object: Box::new(Expr::Identifier(IdentifierExpr {
            span: statement.enum_name_span,
            name: statement.enum_name.clone(),
        })),
        member: statement.variant_name.clone(),
        member_span: statement.variant_name_span,
    })
}

fn tag_only_if_is_payload_pattern_is_supported(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
) -> bool {
    matches!(
        (payload, payload_len),
        (None, 0)
            | (Some(SwitchPayloadPattern::Discard(_)), 1)
            | (Some(SwitchPayloadPattern::Binding(_)), 1)
    )
}

fn tag_only_switch_payload_pattern_is_supported(
    payload: Option<&SwitchPayloadPattern>,
    payload_len: usize,
) -> bool {
    matches!(
        (payload, payload_len),
        (None, 0)
            | (Some(SwitchPayloadPattern::Discard(_)), 1)
            | (Some(SwitchPayloadPattern::Binding(_)), 1)
    )
}

fn tag_only_if_is_then_prologue(
    statement: &IfIsStmt,
    source_slot: usize,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BranchPrologue, Vec<Diagnostic>> {
    let Some(SwitchPayloadPattern::Binding(binding)) = &statement.payload else {
        return Ok(BranchPrologue::empty());
    };
    let (payload_offset, payload_type) =
        payload_enum_variant_payload_abi(&statement.expression, &statement.variant_name, context)
            .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    let kind = payload_branch_prologue_binding_kind(binding, payload_type, context)
        .ok_or_else(|| unsupported_if_is_diagnostic(diagnostic_code))?;
    Ok(BranchPrologue::single_binding(BranchPrologueBinding {
        name: binding.name.clone(),
        source_slot,
        payload_offset,
        kind,
        diagnostic_code,
    }))
}

fn payload_enum_variant_payload_abi(
    expression: &Expr,
    variant_name: &str,
    context: &LoweringContext,
) -> Option<(u32, AbiType)> {
    let ty = context.expression_type_expr(expression.span())?;
    let (_, resolved) = context.resolved_calls()?;
    let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;
    let AbiType::Enum(enum_) = value.ty else {
        return None;
    };
    let payload_offset = u32::try_from(enum_.payload_offset).ok()?;
    let payload_type = enum_
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)?
        .payload
        .clone()?;
    Some((payload_offset, payload_type))
}

fn payload_binding_abi_type_is_supported(payload_type: &AbiType) -> bool {
    matches!(
        payload_type,
        AbiType::I32 | AbiType::U8 | AbiType::Usize | AbiType::Bool | AbiType::StrView
    )
}

fn payload_branch_prologue_binding_kind(
    binding: &crate::ast::SwitchPayloadBinding,
    payload_type: AbiType,
    context: &LoweringContext,
) -> Option<BranchPrologueBindingKind> {
    if payload_binding_abi_type_is_supported(&payload_type) {
        return Some(BranchPrologueBindingKind::ScalarOrStrView(payload_type));
    }

    let ty = context.binding_type_expr(binding.span)?;
    let (_, resolved) = context.resolved_calls()?;
    let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    })
    .ok()?;

    if matches!(payload_type, AbiType::SliceView) {
        if !matches!(value.ty, AbiType::SliceView)
            || value.layout != layout_of(&payload_type).ok()?
        {
            return None;
        }
        return Some(BranchPrologueBindingKind::SliceView(
            slice_type_info_from_type_expr_with_resolver(&ty, resolved, |source| {
                context.resolved_source(source)
            }),
        ));
    }

    if !matches!(payload_type, AbiType::Struct(_) | AbiType::Array { .. }) {
        return None;
    }
    let payload_layout = layout_of(&payload_type).ok()?;
    if !supported_aggregate_copy_layout(payload_layout) {
        return None;
    }
    if value.layout != payload_layout
        || !matches!(value.ty, AbiType::Struct(_) | AbiType::Array { .. })
        || !type_expr_is_copy_aggregate_value_with_resolver(&ty, resolved, |source| {
            context.resolved_source(source)
        })
    {
        return None;
    }
    let (root_source, _) = context.resolved_calls()?;
    let fields =
        aggregate_fields_from_type_expr_with_resolver(&ty, root_source, resolved, |source| {
            context.resolved_source(source)
        })
        .unwrap_or_default();
    Some(BranchPrologueBindingKind::CopyAggregate {
        layout: payload_layout,
        fields,
    })
}

fn expression_is_payload_enum_aggregate_value(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    let Some(ty) = context.expression_type_expr(expression.span()) else {
        return false;
    };
    let Some((_, resolved)) = context.resolved_calls() else {
        return false;
    };
    abi_value_from_type_expr_with_resolver(&ty, resolved, |source| context.resolved_source(source))
        .is_ok_and(|value| matches!(value.ty, AbiType::Enum(_)))
}

fn tag_only_if_is_aggregate_source_slot(
    expression: &Expr,
    context: &LoweringContext,
) -> Option<usize> {
    let Expr::Identifier(identifier) = unwrap_group(expression) else {
        return None;
    };
    context
        .aggregate_local(&identifier.name)
        .map(|local| local.slot_index)
}

fn tag_only_if_is_target_name(statement: &IfIsStmt) -> String {
    format!(
        "<if-is:{}:{}:{}>",
        statement.span.source.raw(),
        statement.span.start,
        statement.span.end
    )
}

fn payload_enum_tag_only_switch_variant_names(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> Option<Vec<String>> {
    let Some(first_arm) = statement.arms.first() else {
        return statement.wildcard_arm.as_ref().and_then(|_| {
            context.payload_enum_variant_names_for_expression(&statement.expression)
        });
    };
    let (_, resolved) = context.resolved_calls()?;
    let target_symbol = resolved.type_symbol_by_name(&first_arm.enum_name)?;
    if target_symbol.kind != crate::resolve::TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .all(|variant| variant.payload.is_empty())
    {
        return None;
    }

    let arms_are_supported = statement.arms.iter().all(|arm| {
        let Some(arm_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
            return false;
        };
        if arm_symbol.canonical_name != target_symbol.canonical_name {
            return false;
        }
        let Some(variant) = target_symbol
            .variants
            .iter()
            .find(|variant| variant.name == arm.variant_name)
        else {
            return false;
        };
        tag_only_switch_payload_pattern_is_supported(arm.payload.as_ref(), variant.payload.len())
    });
    arms_are_supported.then(|| {
        target_symbol
            .variants
            .iter()
            .map(|variant| variant.name.clone())
            .collect()
    })
}

fn tag_only_switch_body(
    statement: &SwitchStmt,
    target: Expr,
    variant_names: &[String],
    source_slot: usize,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredPayloadlessSwitchBody, Vec<Diagnostic>> {
    let Some((condition_arms, fallback)) = tag_only_switch_condition_arms_and_fallback(
        statement,
        variant_names,
        source_slot,
        context,
        diagnostic_code,
    )?
    else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };

    if condition_arms.is_empty() {
        return Ok(LoweredPayloadlessSwitchBody::Direct(fallback));
    }

    let mut current = LoweredPayloadlessSwitchBody::Direct(fallback);
    for arm in condition_arms.iter().rev() {
        current = LoweredPayloadlessSwitchBody::Conditional(LoweredSwitchCondition {
            condition: Expr::Binary(BinaryExpr {
                span: arm.span,
                left: Box::new(target.clone()),
                operator: BinaryOperator::Equal,
                operator_span: arm.span,
                right: Box::new(tag_only_switch_variant_tag_expression(
                    arm,
                    context,
                    diagnostic_code,
                )?),
            }),
            then_branch: tag_only_switch_arm_block(
                arm,
                &statement.expression,
                source_slot,
                context,
                diagnostic_code,
            )?,
            else_body: Box::new(current),
        });
    }

    Ok(current)
}

fn tag_only_switch_condition_arms_and_fallback<'a>(
    statement: &'a SwitchStmt,
    variant_names: &[String],
    source_slot: usize,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Option<(&'a [SwitchArm], LoweredSwitchBlock)>, Vec<Diagnostic>> {
    if let Some(wildcard_arm) = &statement.wildcard_arm {
        return Ok(Some((
            &statement.arms,
            LoweredSwitchBlock {
                block: wildcard_arm.body.clone(),
                prologue: BranchPrologue::empty(),
            },
        )));
    }

    if !payloadless_switch_covers_all_variants(statement, variant_names) {
        return Ok(Some((
            &statement.arms,
            LoweredSwitchBlock {
                block: Block {
                    span: statement.span,
                    statements: Vec::new(),
                    result: None,
                },
                prologue: BranchPrologue::empty(),
            },
        )));
    }

    if statement.arms.len() == 1 {
        return Ok(Some((
            &[],
            tag_only_switch_arm_block(
                &statement.arms[0],
                &statement.expression,
                source_slot,
                context,
                diagnostic_code,
            )?,
        )));
    }

    let Some((last, condition_arms)) = statement.arms.split_last() else {
        return Ok(None);
    };
    Ok(Some((
        condition_arms,
        tag_only_switch_arm_block(
            last,
            &statement.expression,
            source_slot,
            context,
            diagnostic_code,
        )?,
    )))
}

fn tag_only_switch_arm_block(
    arm: &SwitchArm,
    target_expression: &Expr,
    source_slot: usize,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredSwitchBlock, Vec<Diagnostic>> {
    Ok(LoweredSwitchBlock {
        block: arm.body.clone(),
        prologue: tag_only_switch_arm_prologue(
            arm,
            target_expression,
            source_slot,
            context,
            diagnostic_code,
        )?,
    })
}

fn tag_only_switch_arm_prologue(
    arm: &SwitchArm,
    target_expression: &Expr,
    source_slot: usize,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BranchPrologue, Vec<Diagnostic>> {
    let Some(SwitchPayloadPattern::Binding(binding)) = &arm.payload else {
        return Ok(BranchPrologue::empty());
    };
    let (payload_offset, payload_type) =
        payload_enum_variant_payload_abi(target_expression, &arm.variant_name, context)
            .ok_or_else(|| unsupported_switch_diagnostic(diagnostic_code))?;
    let kind = payload_branch_prologue_binding_kind(binding, payload_type, context)
        .ok_or_else(|| unsupported_switch_diagnostic(diagnostic_code))?;
    Ok(BranchPrologue::single_binding(BranchPrologueBinding {
        name: binding.name.clone(),
        source_slot,
        payload_offset,
        kind,
        diagnostic_code,
    }))
}

fn tag_only_switch_variant_tag_expression(
    arm: &SwitchArm,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Expr, Vec<Diagnostic>> {
    let variant = payloadless_switch_variant_expression(arm);
    let Expr::Member(member) = &variant else {
        unreachable!("switch variant expression must be a member expression");
    };
    let tag = context
        .enum_variant_tag(member)
        .ok_or_else(|| unsupported_switch_diagnostic(diagnostic_code))?;
    Ok(Expr::IntegerLiteral(LiteralExpr {
        span: arm.variant_name_span,
        value: tag.to_string(),
    }))
}

fn payloadless_switch_variant_names(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> Option<Vec<String>> {
    let Some(first_arm) = statement.arms.first() else {
        return context.payloadless_enum_variant_names_for_expression(&statement.expression);
    };
    let (_, resolved) = context.resolved_calls()?;
    let target_symbol = resolved.type_symbol_by_name(&first_arm.enum_name)?;
    if target_symbol.kind != crate::resolve::TypeSymbolKind::Enum
        || target_symbol.variants.len() > 256
        || target_symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
    {
        return None;
    }

    let arms_are_supported = statement.arms.iter().all(|arm| {
        if arm.payload.is_some() {
            return false;
        }
        let Some(arm_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
            return false;
        };
        if arm_symbol.canonical_name != target_symbol.canonical_name {
            return false;
        }
        let variant = payloadless_switch_variant_expression(arm);
        let Expr::Member(member) = &variant else {
            unreachable!("payloadless switch variant expression must be a member expression");
        };
        context.payloadless_enum_variant_tag(member).is_some()
    });
    arms_are_supported.then(|| {
        target_symbol
            .variants
            .iter()
            .map(|variant| variant.name.clone())
            .collect()
    })
}

pub(super) fn payloadless_switch_is_exhaustive(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> bool {
    if statement.wildcard_arm.is_some() {
        return payloadless_switch_variant_names(statement, context).is_some();
    }

    payloadless_switch_variant_names(statement, context).is_some_and(|variant_names| {
        payloadless_switch_covers_all_variants(statement, &variant_names)
    })
}

pub(super) fn lowerable_switch_is_exhaustive(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> bool {
    if statement.wildcard_arm.is_some() {
        return payloadless_switch_variant_names(statement, context).is_some()
            || payload_enum_tag_only_switch_variant_names(statement, context).is_some();
    }

    payloadless_switch_variant_names(statement, context).is_some_and(|variant_names| {
        payloadless_switch_covers_all_variants(statement, &variant_names)
    }) || payload_enum_tag_only_switch_variant_names(statement, context).is_some_and(
        |variant_names| payloadless_switch_covers_all_variants(statement, &variant_names),
    )
}

fn payloadless_switch_body(
    statement: &SwitchStmt,
    target: Expr,
    variant_names: &[String],
    diagnostic_code: &'static str,
) -> Result<LoweredPayloadlessSwitchBody, Vec<Diagnostic>> {
    let Some((condition_arms, fallback)) =
        payloadless_switch_condition_arms_and_fallback(statement, variant_names)
    else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };

    let fallback = LoweredSwitchBlock {
        block: fallback,
        prologue: BranchPrologue::empty(),
    };
    if condition_arms.is_empty() {
        return Ok(LoweredPayloadlessSwitchBody::Direct(fallback));
    }

    let mut current = LoweredPayloadlessSwitchBody::Direct(fallback);
    for arm in condition_arms.iter().rev() {
        current = LoweredPayloadlessSwitchBody::Conditional(LoweredSwitchCondition {
            condition: Expr::Binary(BinaryExpr {
                span: arm.span,
                left: Box::new(target.clone()),
                operator: BinaryOperator::Equal,
                operator_span: arm.span,
                right: Box::new(payloadless_switch_variant_expression(arm)),
            }),
            then_branch: LoweredSwitchBlock {
                block: arm.body.clone(),
                prologue: BranchPrologue::empty(),
            },
            else_body: Box::new(current),
        });
    }

    Ok(current)
}

fn payloadless_switch_condition_arms_and_fallback<'a>(
    statement: &'a SwitchStmt,
    variant_names: &[String],
) -> Option<(&'a [SwitchArm], Block)> {
    if let Some(wildcard_arm) = &statement.wildcard_arm {
        return Some((&statement.arms, wildcard_arm.body.clone()));
    }

    if !payloadless_switch_covers_all_variants(statement, variant_names) {
        return Some((
            &statement.arms,
            Block {
                span: statement.span,
                statements: Vec::new(),
                result: None,
            },
        ));
    }

    if statement.arms.len() == 1 {
        return Some((&statement.arms, statement.arms[0].body.clone()));
    }

    let (last, condition_arms) = statement.arms.split_last()?;
    Some((condition_arms, last.body.clone()))
}

fn payloadless_switch_covers_all_variants(
    statement: &SwitchStmt,
    variant_names: &[String],
) -> bool {
    variant_names.iter().all(|variant_name| {
        statement
            .arms
            .iter()
            .any(|arm| arm.variant_name == *variant_name)
    })
}

fn payloadless_switch_variant_expression(arm: &SwitchArm) -> Expr {
    Expr::Member(MemberExpr {
        span: arm.span,
        object: Box::new(Expr::Identifier(IdentifierExpr {
            span: arm.enum_name_span,
            name: arm.enum_name.clone(),
        })),
        member: arm.variant_name.clone(),
        member_span: arm.variant_name_span,
    })
}

fn payloadless_switch_target_name(statement: &SwitchStmt) -> String {
    format!(
        "<match:{}:{}:{}>",
        statement.span.source.raw(),
        statement.span.start,
        statement.span.end
    )
}

fn unsupported_if_is_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower payloadless `if is` branches or tag-only payload enum `if is` branches over existing enum values",
    )]
}

fn unsupported_switch_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower payloadless enum `match` statements or tag-only payload enum `match` statements over existing enum values",
    )]
}

pub(super) fn lower_return_statement_with_scope_drops(
    statement: &ReturnStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let return_type = context.function_return_type().clone();
    let success_type = return_type.success_type().clone();
    let function_name = context.function_name().to_string();

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) =
            lower_never_expression_with_scope_drops(expression, context)?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && matches!(return_type, Type::Fallible(_))
        && let Some((root_source, resolved)) = context.resolved_calls()
        && let Some(payload) =
            lower_error_payload(expression, resolved, root_source, Some(context))?
    {
        return append_scope_end_drops_before_exit(lower_fallible_failure(payload), context);
    }

    if let Some(expression) = &statement.expression
        && context.function_returns_optional()
        && expression_is_none_literal(expression)
    {
        return append_scope_end_drops_before_exit(vec![Instruction::ReturnOptionalNone], context);
    }

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) = lower_otherwise_scalar_return_with_scope_drops(
            expression,
            &success_type,
            &return_type,
            context,
            diagnostic_code,
        )?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) = lower_otherwise_aggregate_return_with_scope_drops(
            expression,
            &success_type,
            &return_type,
            context,
        )?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && let Some(return_instructions) =
            lower_value_return_with_scope_drops(&success_type, expression, &return_type, context)?
    {
        return Ok(return_instructions);
    }

    if let Some(expression) = &statement.expression
        && matches!(success_type, Type::DirectAggregate { .. })
        && !context.pending_aggregate_drops().is_empty()
    {
        let Some((_root_source, resolved)) = context.resolved_calls() else {
            return Err(unsupported_return_diagnostic(
                diagnostic_code,
                &function_name,
                "aggregate",
            ));
        };
        return lower_direct_aggregate_return_with_scope_drops(
            expression,
            &success_type,
            &return_type,
            &function_name,
            resolved,
            context,
        );
    }

    let return_instructions = match (&success_type, &statement.expression) {
        (Type::I32, Some(expression)) => lower_i32_return_expression(expression, context),
        (Type::U8, Some(expression)) => lower_u8_return_expression(expression, context),
        (Type::Usize, Some(expression)) => lower_usize_return_expression(expression, context),
        (Type::Bool, Some(expression)) => {
            lower_bool_return_expression(expression, context, diagnostic_code)
        }
        (Type::Str, Some(expression)) => lower_str_return_expression(expression, context),
        (Type::Slice { .. }, Some(expression)) => {
            lower_slice_return_expression(expression, context)
        }
        (Type::Aggregate { .. } | Type::DirectAggregate { .. }, Some(expression)) => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_return_diagnostic(
                    diagnostic_code,
                    &function_name,
                    "aggregate",
                ));
            };
            lower_aggregate_return_expression(
                expression,
                &success_type,
                &function_name,
                resolved,
                context,
            )
        }
        (Type::Never, Some(_)) => Err(vec![Diagnostic::error(
            diagnostic_code,
            format!(
                "IR v0 can only lower never function `{function_name}` returns from `never` calls"
            ),
        )]),
        (Type::Void, None) => Ok(vec![Instruction::Return]),
        (Type::Void, Some(_)) => Err(vec![Diagnostic::error(
            diagnostic_code,
            format!("IR v0 cannot lower value returns from void function `{function_name}`"),
        )]),
        (Type::I32, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "i32",
        )),
        (Type::U8, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "u8",
        )),
        (Type::Usize, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "usize",
        )),
        (Type::Bool, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "bool",
        )),
        (Type::Str, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "&str",
        )),
        (Type::Slice { .. }, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "slice",
        )),
        (Type::Aggregate { .. } | Type::DirectAggregate { .. }, None) => Err(
            unsupported_bare_return_diagnostic(diagnostic_code, &function_name, "aggregate"),
        ),
        (Type::Error, _) => Err(unsupported_return_diagnostic(
            diagnostic_code,
            &function_name,
            "error",
        )),
        (Type::Borrow { .. }, _) => Err(unsupported_return_diagnostic(
            diagnostic_code,
            &function_name,
            "borrow",
        )),
        (Type::Never, None) => Err(unsupported_bare_return_diagnostic(
            diagnostic_code,
            &function_name,
            "never",
        )),
        (Type::Fallible(_), _) => Err(unsupported_return_diagnostic(
            diagnostic_code,
            &function_name,
            "nested fallible",
        )),
    }?;

    let return_instructions = mark_fallible_success_returns(&return_type, return_instructions);
    append_scope_end_drops_before_exit(return_instructions, context)
}

fn unsupported_bare_return_diagnostic(
    diagnostic_code: &'static str,
    function_name: &str,
    return_label: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!("IR v0 cannot lower bare returns from {return_label} function `{function_name}`"),
    )]
}

fn unsupported_return_diagnostic(
    diagnostic_code: &'static str,
    function_name: &str,
    return_label: &str,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        format!("IR v0 cannot lower {return_label} returns from function `{function_name}`"),
    )]
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

fn lower_terminal_aggregate_if_statement(
    statement: &IfStmt,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_terminal_aggregate_if_statement_with_branch_prologues(
        statement,
        context,
        &BranchPrologue::empty(),
        &BranchPrologue::empty(),
        success_type,
        function_name,
        resolved,
        sources,
    )
}

fn lower_terminal_aggregate_if_statement_with_branch_prologues(
    statement: &IfStmt,
    context: &LoweringContext,
    then_prologue: &BranchPrologue,
    else_prologue: &BranchPrologue,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
    };

    let then_instructions = lower_terminal_aggregate_return_block_with_prologue(
        &statement.then_block,
        context,
        &statement.condition,
        then_prologue,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    let else_instructions = lower_terminal_aggregate_return_block_with_prologue(
        else_block,
        context,
        &statement.condition,
        else_prologue,
        success_type,
        function_name,
        resolved,
        sources,
    )?;

    lower_terminal_condition(
        &statement.condition,
        then_instructions,
        else_instructions,
        context,
        "E8007",
        sources,
    )
}

fn lower_terminal_aggregate_payloadless_switch_body(
    body: LoweredPayloadlessSwitchBody,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match body {
        LoweredPayloadlessSwitchBody::Direct(block) => lower_terminal_aggregate_switch_block(
            block,
            context,
            success_type,
            function_name,
            resolved,
            sources,
        ),
        LoweredPayloadlessSwitchBody::Conditional(condition) => {
            lower_terminal_aggregate_switch_condition(
                condition,
                context,
                success_type,
                function_name,
                resolved,
                sources,
            )
        }
    }
}

fn lower_terminal_aggregate_switch_block(
    block: LoweredSwitchBlock,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    let instructions = block.prologue.apply(&mut branch_context)?;
    lower_terminal_aggregate_return_block_with_context_and_prefix(
        &block.block,
        branch_context,
        instructions,
        success_type,
        function_name,
        resolved,
        sources,
    )
}

fn lower_terminal_aggregate_switch_condition(
    condition: LoweredSwitchCondition,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let then_instructions = lower_terminal_aggregate_switch_block(
        condition.then_branch,
        context,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    let else_instructions = lower_terminal_aggregate_payloadless_switch_body(
        *condition.else_body,
        context,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    lower_terminal_condition(
        &condition.condition,
        then_instructions,
        else_instructions,
        context,
        "E8007",
        sources,
    )
}

fn lower_terminal_aggregate_return_block_with_prologue(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    prologue: &BranchPrologue,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let initial_instructions = prologue.apply(&mut branch_context)?;
    lower_terminal_aggregate_return_block_with_context_and_prefix(
        block,
        branch_context,
        initial_instructions,
        success_type,
        function_name,
        resolved,
        sources,
    )
}

fn lower_terminal_aggregate_return_block_with_context_and_prefix(
    block: &Block,
    mut branch_context: LoweringContext,
    mut instructions: Vec<Instruction>,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) =
        split_terminal_branch_block(block, "E8007", "functions", "aggregate")?;
    instructions.extend(lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        "E8007",
        "functions",
        "aggregate",
        sources,
    )?);

    match terminal {
        TerminalBranch::Result(expression) => {
            instructions.extend(lower_terminal_aggregate_result_expression(
                expression,
                success_type,
                function_name,
                resolved,
                &mut branch_context,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Return(statement)) => {
            if statement.expression.as_ref().is_some_and(|expression| {
                matches!(
                    unwrap_group(expression),
                    Expr::If(_) | Expr::IfIs(_) | Expr::Match(_)
                )
            }) {
                instructions.extend(lower_terminal_return_statement_with_scope_drops(
                    statement,
                    &mut branch_context,
                    "E8007",
                    "functions",
                    sources,
                )?);
                return Ok(instructions);
            }
            let Some(expression) = &statement.expression else {
                return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
            };
            mark_explicit_moves_in_expression(expression, &mut branch_context);
            if matches!(success_type, Type::DirectAggregate { .. })
                && !branch_context.pending_aggregate_drops().is_empty()
            {
                instructions.extend(lower_terminal_direct_aggregate_return_with_scope_drops(
                    expression,
                    success_type,
                    function_name,
                    resolved,
                    &mut branch_context,
                )?);
                return Ok(instructions);
            }
            let return_instructions = lower_aggregate_return_expression(
                expression,
                success_type,
                function_name,
                resolved,
                &branch_context,
            )?;
            instructions.extend(append_scope_end_drops_before_exit(
                return_instructions,
                &mut branch_context,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::If(statement)) => {
            instructions.extend(lower_terminal_aggregate_if_statement(
                statement,
                &branch_context,
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::IfIs(statement)) => {
            let if_is = tag_only_if_is_as_control_flow(statement, &mut branch_context, "E8007")?;
            instructions.extend(if_is.leading_instructions);
            instructions.extend(lower_terminal_aggregate_if_statement_with_branch_prologues(
                &if_is.statement,
                &branch_context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch = tag_only_switch_as_control_flow(statement, &mut branch_context, "E8007")?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_aggregate_payloadless_switch_body(
                switch.body,
                &branch_context,
                success_type,
                function_name,
                resolved,
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
                return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_aggregate_if_diagnostic(function_name)),
    }
}

fn lower_terminal_aggregate_result_expression(
    expression: &Expr,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::If(statement) => lower_terminal_aggregate_if_statement(
            statement,
            context,
            success_type,
            function_name,
            resolved,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_is = tag_only_if_is_as_control_flow(statement, context, "E8007")?;
            let mut instructions = if_is.leading_instructions;
            instructions.extend(lower_terminal_aggregate_if_statement_with_branch_prologues(
                &if_is.statement,
                context,
                &if_is.then_prologue,
                &BranchPrologue::empty(),
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        Expr::Match(statement) => {
            let switch = tag_only_switch_as_control_flow(statement, context, "E8007")?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_aggregate_payloadless_switch_body(
                switch.body,
                context,
                success_type,
                function_name,
                resolved,
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

            mark_explicit_moves_in_expression(expression, context);
            if matches!(success_type, Type::DirectAggregate { .. })
                && !context.pending_aggregate_drops().is_empty()
            {
                return lower_terminal_direct_aggregate_return_with_scope_drops(
                    expression,
                    success_type,
                    function_name,
                    resolved,
                    context,
                );
            }

            let return_instructions = lower_aggregate_return_expression(
                expression,
                success_type,
                function_name,
                resolved,
                context,
            )?;
            append_scope_end_drops_before_exit(return_instructions, context)
        }
    }
}

fn lower_terminal_direct_aggregate_return_with_scope_drops(
    expression: &Expr,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, destination) = aggregate_return_layout_and_destination(success_type);
    if !matches!(destination, AggregateLocation::DirectReturn)
        || !supported_aggregate_copy_layout(expected_layout)
    {
        return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_return_expression_to_location(
        expression,
        success_type,
        AggregateLocation::Slot(slot_index),
        function_name,
        resolved,
        context,
    )?);

    let mut tail = append_scope_end_drops_before_exit(vec![Instruction::Return], context)?;
    let Some(return_index) = tail
        .iter()
        .rposition(|instruction| matches!(instruction, Instruction::Return))
    else {
        return Ok(instructions);
    };
    tail.insert(
        return_index,
        Instruction::CopyAggregate {
            destination,
            source: AggregateLocation::Slot(slot_index),
            layout: expected_layout,
        },
    );
    instructions.extend(tail);
    Ok(instructions)
}

pub(super) fn lower_direct_aggregate_return_with_scope_drops(
    expression: &Expr,
    success_type: &Type,
    function_return_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, destination) = aggregate_return_layout_and_destination(success_type);
    if !matches!(destination, AggregateLocation::DirectReturn)
        || !supported_aggregate_copy_layout(expected_layout)
    {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_return_expression_to_location(
        expression,
        success_type,
        AggregateLocation::Slot(slot_index),
        function_name,
        resolved,
        context,
    )?);
    append_scope_drops_then_restore_aggregate_return(
        &mut instructions,
        slot_index,
        expected_layout,
        destination,
        function_return_type,
        context,
    )?;
    Ok(instructions)
}

fn unsupported_terminal_aggregate_if_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower terminal aggregate `if` branches in function `{function_name}` when both branches contain supported leading statements followed by aggregate returns or nested terminal aggregate `if` branches"
        ),
    )]
}

pub(super) fn lower_value_return_with_scope_drops(
    success_type: &Type,
    expression: &Expr,
    return_type: &Type,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    mark_explicit_moves_in_expression(expression, context);
    if context.pending_aggregate_drops().is_empty() {
        return Ok(None);
    }

    let mut instructions = match success_type {
        Type::I32 => {
            let temporary = context.next_i32_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut instructions =
                lower_i32_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(temporary),
                }],
                1,
                return_type,
                context,
            )?;
            instructions
        }
        Type::U8 => {
            let temporary = context.next_u8_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut instructions =
                lower_u8_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::Location(temporary),
                }],
                1,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Usize => {
            let temporary = context.next_usize_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut instructions =
                lower_usize_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::Location(temporary),
                }],
                1,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Bool => {
            let temporary = context.next_bool_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut instructions = lower_bool_expression_to_location(
                expression,
                temporary,
                &expression_context,
                "E8007",
            )?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::Location(temporary),
                }],
                1,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Str => {
            let temporary = context.next_str_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(2);
            let mut instructions =
                lower_str_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::Location(temporary),
                }],
                2,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Slice { .. } => {
            let temporary = context.next_slice_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(2);
            let mut instructions =
                lower_slice_expression_to_location(expression, temporary, &expression_context)?;
            append_scope_drops_then_restore_return(
                &mut instructions,
                vec![Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(temporary),
                }],
                2,
                return_type,
                context,
            )?;
            instructions
        }
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Void
        | Type::Never
        | Type::Borrow { .. }
        | Type::Fallible(_) => return Ok(None),
    };

    Ok(Some(std::mem::take(&mut instructions)))
}

fn append_scope_drops_then_restore_return(
    instructions: &mut Vec<Instruction>,
    restore_return: Vec<Instruction>,
    reserved_local_abi_words: usize,
    return_type: &Type,
    context: &mut LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let cleanup_context = context.with_reserved_local_abi_words(reserved_local_abi_words);
    let mut tail = vec![success_return_instruction(return_type)];
    let Some(return_index) = tail.iter().rposition(is_scope_exit_instruction) else {
        return Ok(());
    };
    let drops = lower_scope_end_drop_instructions(&cleanup_context)?;
    let restore_index = return_index + drops.len();
    tail.splice(return_index..return_index, drops);
    mark_pending_aggregate_drops(context);
    tail.splice(restore_index..restore_index, restore_return);
    instructions.extend(tail);
    Ok(())
}

fn lower_leading_bindings(
    statements: &[Stmt],
    context: &mut LoweringContext,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
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
                        vec![Diagnostic::error(
                            "E8007",
                            "IR v0 can only lower leading scalar local bindings, scalar assignments, drop statements, or effect-only call statements before `return`",
                        )],
                        sources,
                        statement.span,
                    ));
                };
                instructions.extend(void_instructions);
            }
            Stmt::Drop(statement) => {
                instructions.extend(lower_drop_statement(statement, context).map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    },
                )?);
            }
            Stmt::If(statement) => {
                instructions.extend(
                    lower_nonterminal_if_statement(
                        statement,
                        context,
                        None,
                        &[],
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::IfIs(statement) => {
                let if_is = tag_only_if_is_as_control_flow(statement, context, "E8007").map_err(
                    |diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                    },
                )?;
                instructions.extend(if_is.leading_instructions);
                instructions.extend(
                    lower_nonterminal_if_statement_with_branch_prologues(
                        &if_is.statement,
                        context,
                        &if_is.then_prologue,
                        &BranchPrologue::empty(),
                        None,
                        &[],
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::Switch(statement) => {
                instructions.extend(
                    lower_nonterminal_payloadless_switch_statement(
                        statement,
                        context,
                        None,
                        &[],
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::ForRange(statement) => {
                instructions.extend(
                    lower_nonterminal_for_range_statement(
                        statement,
                        context,
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::While(statement) => {
                instructions.extend(
                    lower_nonterminal_while_statement(
                        statement,
                        context,
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            Stmt::Loop(statement) => {
                instructions.extend(
                    lower_nonterminal_loop_statement(
                        statement,
                        context,
                        "E8007",
                        "functions",
                        sources,
                    )
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?,
                );
            }
            _ => {
                return Err(attach_primary_span_if_absent(
                    vec![Diagnostic::error(
                        "E8007",
                        "IR v0 can only lower leading scalar local bindings, scalar assignments, drop statements, effect-only call statements, or supported non-terminal `if`/`for`/`while`/`loop` statements before `return`",
                    )],
                    sources,
                    statement.span(),
                ));
            }
        };
        mark_lowered_statement_aggregate_uses(statement, context);
    }

    Ok(instructions)
}

pub(super) fn lower_drop_statement(
    statement: &DropStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(local) = context.aggregate_local(&statement.name) else {
        return Err(unsupported_drop_statement_diagnostic(&statement.name));
    };
    let Some(drop_kind) = local.drop_kind else {
        context.mark_aggregate_local_dropped(&statement.name);
        return Ok(Vec::new());
    };

    context.mark_aggregate_local_dropped(&statement.name);
    lower_aggregate_drop_instructions(
        &statement.name,
        local.slot_index,
        local.layout,
        &drop_kind,
        context,
    )
}

pub(super) fn lower_never_expression_with_scope_drops(
    expression: &Expr,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Some(instructions) = lower_never_return_expression(expression, context)? else {
        return Ok(None);
    };
    mark_explicit_moves_in_expression(expression, context);
    append_scope_end_drops_before_exit(instructions, context).map(Some)
}

pub(super) fn append_scope_end_drops_before_exit(
    mut instructions: Vec<Instruction>,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(return_index) = instructions.iter().rposition(is_scope_exit_instruction) else {
        return Ok(instructions);
    };
    let drops = lower_scope_end_drop_instructions(context)?;
    instructions.splice(return_index..return_index, drops);
    mark_pending_aggregate_drops(context);
    Ok(instructions)
}

pub(super) fn propagating_failure_mode(
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let instructions = lower_scope_end_drop_instructions(context)?;
    if instructions.is_empty() {
        return Ok(FallibleFailureMode::Propagate);
    }
    let (code, message) = context.next_error_local_locations()?;
    Ok(FallibleFailureMode::PropagateWithCleanup {
        code,
        message,
        instructions,
    })
}

pub(super) fn replacement_drop_for_aggregate_slot(
    slot_index: usize,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(drop_) = context.pending_aggregate_drop_by_slot(slot_index) else {
        return Ok(Vec::new());
    };
    lower_pending_aggregate_drop(&drop_, context)
}

pub(super) fn lower_scope_end_drops_for_locals_since(
    context: &mut LoweringContext,
    local_mark: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let pending = context.pending_aggregate_drops_since(local_mark);
    let mut instructions = Vec::new();
    for drop_ in &pending {
        instructions.extend(lower_pending_aggregate_drop(drop_, context)?);
    }
    for drop_ in &pending {
        context.mark_aggregate_local_dropped(&drop_.name);
    }
    Ok(instructions)
}

fn lower_scope_end_drop_instructions(
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let pending = context.pending_aggregate_drops();
    let mut instructions = Vec::new();
    for drop_ in &pending {
        instructions.extend(lower_pending_aggregate_drop(drop_, context)?);
    }
    Ok(instructions)
}

fn is_scope_exit_instruction(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Return
            | Instruction::ReturnFallibleSuccess
            | Instruction::ReturnOptionalNone
            | Instruction::ReturnFallibleFailure { .. }
            | Instruction::TailCall { .. }
    )
}

fn mark_pending_aggregate_drops(context: &mut LoweringContext) {
    let pending = context.pending_aggregate_drops();
    for drop_ in &pending {
        context.mark_aggregate_local_dropped(&drop_.name);
    }
}

fn lower_pending_aggregate_drop(
    drop_: &PendingAggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_aggregate_drop_instructions(
        &drop_.name,
        drop_.slot_index,
        drop_.layout,
        &drop_.drop_kind,
        context,
    )
}

pub(super) fn lower_aggregate_drop_instructions(
    name: &str,
    slot_index: usize,
    layout: ValueLayout,
    drop_kind: &AggregateDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match drop_kind {
        AggregateDrop::Direct(drop_glue) => {
            lower_direct_aggregate_drop_instruction(name, slot_index, layout, drop_glue, context)
                .map(|instruction| vec![instruction])
        }
        AggregateDrop::PayloadEnum(drop_) => {
            lower_payload_enum_drop_instructions(name, slot_index, drop_, context)
        }
    }
}

fn lower_direct_aggregate_drop_instruction(
    name: &str,
    slot_index: usize,
    layout: ValueLayout,
    drop_glue: &super::context::DropGlue,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(&drop_glue.target) else {
        return Err(unsupported_drop_statement_diagnostic(name));
    };
    if parameter_types.len() != 1 || !drop_parameter_matches_local(&parameter_types[0], layout) {
        return Err(unsupported_drop_statement_diagnostic(name));
    }

    Ok(Instruction::CallVoid {
        target: drop_glue.target.clone(),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(slot_index),
        })],
    })
}

fn lower_payload_enum_drop_instructions(
    name: &str,
    slot_index: usize,
    drop_: &PayloadEnumDrop,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    let tag = temporaries.next_u8()?;
    let mut instructions = vec![Instruction::LoadAggregateU8 {
        destination: tag,
        source: AggregateLocation::Slot(slot_index),
        offset: 0,
    }];
    for variant in drop_.variants.iter().rev() {
        instructions.push(lower_payload_enum_drop_variant_if(
            name, slot_index, tag, variant, context,
        )?);
    }
    Ok(instructions)
}

fn lower_payload_enum_drop_variant_if(
    name: &str,
    slot_index: usize,
    tag: U8Location,
    variant: &PayloadEnumDropVariant,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let mut then_instructions = Vec::new();
    for field in variant.fields.iter().rev() {
        then_instructions.push(lower_payload_enum_drop_field(
            name, slot_index, field, context,
        )?);
    }

    Ok(Instruction::If {
        condition: BoolValue::I32Comparison {
            operator: I32ComparisonOperator::Equal,
            left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(tag))),
            right: I32Value::U8ZeroExtend(Box::new(U8Value::Const(variant.tag))),
        },
        then_instructions,
        else_instructions: Vec::new(),
    })
}

fn lower_payload_enum_drop_field(
    name: &str,
    slot_index: usize,
    field: &PayloadEnumDropField,
    context: &LoweringContext,
) -> Result<Instruction, Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(&field.drop_glue.target) else {
        return Err(unsupported_drop_statement_diagnostic(name));
    };
    if parameter_types.len() != 1
        || !drop_parameter_matches_local(&parameter_types[0], field.payload_layout)
    {
        return Err(unsupported_drop_statement_diagnostic(name));
    }

    Ok(Instruction::CallVoid {
        target: field.drop_glue.target.clone(),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlotField {
                slot_index,
                offset: field.payload_offset,
            },
        })],
    })
}

pub(super) fn mark_lowered_statement_aggregate_uses(
    statement: &Stmt,
    context: &mut LoweringContext,
) {
    match statement {
        Stmt::Binding(statement) => {
            mark_explicit_moves_in_expression(&statement.initializer, context);
        }
        Stmt::Assignment(statement) => {
            if let Expr::Identifier(identifier) = unwrap_group(&statement.target) {
                context.mark_aggregate_local_initialized(&identifier.name);
            }
            mark_explicit_moves_in_expression(&statement.value, context);
        }
        Stmt::Expression(statement) => {
            mark_explicit_moves_in_expression(&statement.expression, context);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                mark_explicit_moves_in_expression(expression, context);
            }
        }
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Drop(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => {}
    }
}

pub(super) fn mark_explicit_moves_in_expression(expression: &Expr, context: &mut LoweringContext) {
    match expression {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = unwrap_group(&unary.operand) {
                context.mark_aggregate_local_moved(&identifier.name);
            } else {
                mark_explicit_moves_in_expression(&unary.operand, context);
            }
        }
        Expr::ArrayLiteral(literal) => {
            for element in &literal.elements {
                mark_explicit_moves_in_expression(element, context);
            }
        }
        Expr::StructLiteral(literal) => {
            for field in &literal.fields {
                mark_explicit_moves_in_expression(&field.value, context);
            }
        }
        Expr::Propagate(propagation) => {
            mark_explicit_moves_in_expression(&propagation.expression, context);
        }
        Expr::Force(force) => {
            mark_explicit_moves_in_expression(&force.expression, context);
        }
        Expr::Catch(catch) => {
            mark_explicit_moves_in_expression(&catch.expression, context);
        }
        Expr::Borrow(borrow) => {
            mark_explicit_moves_in_expression(&borrow.expression, context);
        }
        Expr::Unary(unary) => {
            mark_explicit_moves_in_expression(&unary.operand, context);
        }
        Expr::Binary(binary) => {
            mark_explicit_moves_in_expression(&binary.left, context);
            mark_explicit_moves_in_expression(&binary.right, context);
        }
        Expr::TypeConversion(conversion) => {
            mark_explicit_moves_in_expression(&conversion.expression, context);
        }
        Expr::Call(call) => {
            mark_explicit_moves_in_expression(&call.callee, context);
            for argument in &call.arguments {
                mark_explicit_moves_in_expression(argument, context);
            }
        }
        Expr::Member(member) => {
            mark_explicit_moves_in_expression(&member.object, context);
        }
        Expr::Index(index) => {
            mark_explicit_moves_in_expression(&index.object, context);
            mark_explicit_moves_in_expression(&index.index, context);
        }
        Expr::Group(group) => {
            mark_explicit_moves_in_expression(&group.expression, context);
        }
        Expr::Otherwise(otherwise) => {
            mark_explicit_moves_in_expression(&otherwise.value, context);
            mark_explicit_moves_in_block(&otherwise.fallback, context);
        }
        Expr::If(statement) => {
            mark_explicit_moves_in_expression(&statement.condition, context);
            mark_explicit_moves_in_block(&statement.then_block, context);
            if let Some(block) = &statement.else_block {
                mark_explicit_moves_in_block(block, context);
            }
        }
        Expr::IfIs(statement) => {
            mark_explicit_moves_in_expression(&statement.expression, context);
            mark_explicit_moves_in_block(&statement.then_block, context);
            if let Some(block) = &statement.else_block {
                mark_explicit_moves_in_block(block, context);
            }
        }
        Expr::Match(statement) => {
            mark_explicit_moves_in_expression(&statement.expression, context);
            for arm in &statement.arms {
                mark_explicit_moves_in_block(&arm.body, context);
            }
            if let Some(arm) = &statement.wildcard_arm {
                mark_explicit_moves_in_block(&arm.body, context);
            }
        }
        Expr::InterpolatedString(interpolated) => {
            for part in &interpolated.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    mark_explicit_moves_in_expression(&part.expression, context);
                }
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn mark_explicit_moves_in_block(block: &Block, context: &mut LoweringContext) {
    for statement in &block.statements {
        mark_lowered_statement_aggregate_uses(statement, context);
    }
    if let Some(result) = &block.result {
        mark_explicit_moves_in_expression(result, context);
    }
}

pub(super) fn expression_contains_explicit_aggregate_move(
    expression: &Expr,
    context: &LoweringContext,
) -> bool {
    expression_contains_explicit_aggregate_move_matching(expression, context, &|name, context| {
        context.aggregate_local(name).is_some()
    })
}

pub(super) fn expression_contains_explicit_aggregate_move_outside(
    expression: &Expr,
    context: &LoweringContext,
    local_mark: usize,
) -> bool {
    expression_contains_explicit_aggregate_move_matching(expression, context, &|name, context| {
        context.aggregate_local(name).is_some()
            && !context.aggregate_local_defined_since(name, local_mark)
    })
}

fn expression_contains_explicit_aggregate_move_matching(
    expression: &Expr,
    context: &LoweringContext,
    matches_move: &impl Fn(&str, &LoweringContext) -> bool,
) -> bool {
    match expression {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            if let Expr::Identifier(identifier) = unwrap_group(&unary.operand) {
                matches_move(&identifier.name, context)
            } else {
                expression_contains_explicit_aggregate_move_matching(
                    &unary.operand,
                    context,
                    matches_move,
                )
            }
        }
        Expr::ArrayLiteral(literal) => literal.elements.iter().any(|element| {
            expression_contains_explicit_aggregate_move_matching(element, context, matches_move)
        }),
        Expr::StructLiteral(literal) => literal.fields.iter().any(|field| {
            expression_contains_explicit_aggregate_move_matching(
                &field.value,
                context,
                matches_move,
            )
        }),
        Expr::Propagate(propagation) => expression_contains_explicit_aggregate_move_matching(
            &propagation.expression,
            context,
            matches_move,
        ),
        Expr::Force(force) => expression_contains_explicit_aggregate_move_matching(
            &force.expression,
            context,
            matches_move,
        ),
        Expr::Catch(catch) => expression_contains_explicit_aggregate_move_matching(
            &catch.expression,
            context,
            matches_move,
        ),
        Expr::Borrow(borrow) => expression_contains_explicit_aggregate_move_matching(
            &borrow.expression,
            context,
            matches_move,
        ),
        Expr::Unary(unary) => expression_contains_explicit_aggregate_move_matching(
            &unary.operand,
            context,
            matches_move,
        ),
        Expr::Binary(binary) => {
            expression_contains_explicit_aggregate_move_matching(
                &binary.left,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &binary.right,
                context,
                matches_move,
            )
        }
        Expr::TypeConversion(conversion) => expression_contains_explicit_aggregate_move_matching(
            &conversion.expression,
            context,
            matches_move,
        ),
        Expr::Call(call) => {
            expression_contains_explicit_aggregate_move_matching(
                &call.callee,
                context,
                matches_move,
            ) || call.arguments.iter().any(|argument| {
                expression_contains_explicit_aggregate_move_matching(
                    argument,
                    context,
                    matches_move,
                )
            })
        }
        Expr::Member(member) => expression_contains_explicit_aggregate_move_matching(
            &member.object,
            context,
            matches_move,
        ),
        Expr::Index(index) => {
            expression_contains_explicit_aggregate_move_matching(
                &index.object,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &index.index,
                context,
                matches_move,
            )
        }
        Expr::Group(group) => expression_contains_explicit_aggregate_move_matching(
            &group.expression,
            context,
            matches_move,
        ),
        Expr::Otherwise(otherwise) => {
            expression_contains_explicit_aggregate_move_matching(
                &otherwise.value,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &otherwise.fallback,
                context,
                matches_move,
            )
        }
        Expr::If(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.condition,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.then_block,
                context,
                matches_move,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_contains_explicit_aggregate_move_matching(block, context, matches_move)
            })
        }
        Expr::IfIs(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.then_block,
                context,
                matches_move,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_contains_explicit_aggregate_move_matching(block, context, matches_move)
            })
        }
        Expr::Match(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                matches_move,
            ) || statement.arms.iter().any(|arm| {
                block_contains_explicit_aggregate_move_matching(&arm.body, context, matches_move)
            }) || statement.wildcard_arm.as_ref().is_some_and(|arm| {
                block_contains_explicit_aggregate_move_matching(&arm.body, context, matches_move)
            })
        }
        Expr::InterpolatedString(interpolated) => interpolated.parts.iter().any(|part| {
            if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                expression_contains_explicit_aggregate_move_matching(
                    &part.expression,
                    context,
                    matches_move,
                )
            } else {
                false
            }
        }),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => false,
    }
}

fn block_contains_explicit_aggregate_move_matching(
    block: &Block,
    context: &LoweringContext,
    matches_move: &impl Fn(&str, &LoweringContext) -> bool,
) -> bool {
    block.statements.iter().any(|statement| {
        statement_contains_explicit_aggregate_move_matching(statement, context, matches_move)
    }) || block.result.as_ref().is_some_and(|result| {
        expression_contains_explicit_aggregate_move_matching(result, context, matches_move)
    })
}

fn statement_contains_explicit_aggregate_move_matching(
    statement: &Stmt,
    context: &LoweringContext,
    matches_move: &impl Fn(&str, &LoweringContext) -> bool,
) -> bool {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => false,
        Stmt::Return(statement) => statement.expression.as_ref().is_some_and(|expression| {
            expression_contains_explicit_aggregate_move_matching(expression, context, matches_move)
        }),
        Stmt::Binding(statement) => expression_contains_explicit_aggregate_move_matching(
            &statement.initializer,
            context,
            matches_move,
        ),
        Stmt::Assignment(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.target,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &statement.value,
                context,
                matches_move,
            )
        }
        Stmt::If(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.condition,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.then_block,
                context,
                matches_move,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_contains_explicit_aggregate_move_matching(block, context, matches_move)
            })
        }
        Stmt::IfIs(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.then_block,
                context,
                matches_move,
            ) || statement.else_block.as_ref().is_some_and(|block| {
                block_contains_explicit_aggregate_move_matching(block, context, matches_move)
            })
        }
        Stmt::Switch(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                matches_move,
            ) || statement.arms.iter().any(|arm| {
                block_contains_explicit_aggregate_move_matching(&arm.body, context, matches_move)
            }) || statement.wildcard_arm.as_ref().is_some_and(|arm| {
                block_contains_explicit_aggregate_move_matching(&arm.body, context, matches_move)
            })
        }
        Stmt::ForRange(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.start,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &statement.end,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.body,
                context,
                matches_move,
            )
        }
        Stmt::While(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.condition,
                context,
                matches_move,
            ) || block_contains_explicit_aggregate_move_matching(
                &statement.body,
                context,
                matches_move,
            )
        }
        Stmt::Loop(statement) => {
            block_contains_explicit_aggregate_move_matching(&statement.body, context, matches_move)
        }
        Stmt::Expression(statement) => expression_contains_explicit_aggregate_move_matching(
            &statement.expression,
            context,
            matches_move,
        ),
        Stmt::Drop(_) | Stmt::Break(_) | Stmt::Continue(_) => false,
    }
}

fn drop_parameter_matches_local(parameter_type: &Type, layout: crate::abi::ValueLayout) -> bool {
    let Type::Borrow {
        is_readwrite: true,
        inner,
    } = parameter_type
    else {
        return false;
    };

    match inner.as_ref() {
        Type::Aggregate {
            layout: parameter_layout,
        }
        | Type::DirectAggregate {
            layout: parameter_layout,
            ..
        } => *parameter_layout == layout,
        _ => false,
    }
}

fn unsupported_drop_statement_diagnostic(name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        format!("IR v0 cannot lower drop statement for binding `{name}`"),
    )]
}

pub(super) fn lower_aggregate_return_expression(
    expression: &Expr,
    return_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (_, destination) = aggregate_return_layout_and_destination(return_type);
    let mut instructions = lower_aggregate_return_expression_to_location(
        expression,
        return_type,
        destination,
        function_name,
        resolved,
        context,
    )?;
    instructions.push(Instruction::Return);
    Ok(instructions)
}

fn lower_aggregate_return_expression_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::StructLiteral(literal) => lower_aggregate_struct_literal_return_to_location(
            literal,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::ArrayLiteral(literal) => lower_aggregate_array_literal_return_to_location(
            literal,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::Call(call) => {
            if let Some(instructions) = lower_payload_enum_constructor_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                resolved,
                context,
            )? {
                return Ok(instructions);
            }
            lower_aggregate_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                propagating_failure_mode(context)?,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                FallibleFailureMode::Trap,
            )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return_to_location(
                call,
                return_type,
                destination,
                function_name,
                context,
                lower_catch_failure_mode(catch, context, 0)?,
            )
        }
        Expr::Otherwise(otherwise) => lower_aggregate_otherwise_return_to_location(
            otherwise,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        Expr::Identifier(identifier) => lower_aggregate_local_return_to_location(
            &identifier.name,
            AggregateValueUse::ImplicitCopy,
            return_type,
            destination,
            function_name,
            context,
        ),
        Expr::Member(_) => {
            if let Some(instructions) = lower_payload_enum_constructor_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                resolved,
                context,
            )? {
                return Ok(instructions);
            }
            lower_aggregate_member_return_to_location(
                expression,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unary.operand.as_ref() else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_local_return_to_location(
                &identifier.name,
                AggregateValueUse::ExplicitMove,
                return_type,
                destination,
                function_name,
                context,
            )
        }
        Expr::Group(group) => lower_aggregate_return_expression_to_location(
            &group.expression,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        ),
        _ => Err(unsupported_aggregate_return_diagnostic(function_name)),
    }
}

fn lower_aggregate_otherwise_return_to_location(
    otherwise: &crate::ast::OtherwiseExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !context.pending_aggregate_drops().is_empty() {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if !call_return_type_expr_is_top_level_optional(call, context) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let failure_mode = lower_aggregate_otherwise_return_failure_mode(
        &otherwise.fallback,
        return_type,
        destination,
        function_name,
        resolved,
        context,
    )?;
    lower_aggregate_fallible_call_return_to_location(
        call,
        return_type,
        destination,
        function_name,
        context,
        failure_mode,
    )
}

fn lower_aggregate_otherwise_return_failure_mode(
    fallback: &Block,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let mut fallback_context = context.clone();
    let (mut instructions, exits) = lower_aggregate_otherwise_fallback_to_location(
        fallback,
        return_type,
        destination,
        function_name,
        resolved,
        &mut fallback_context,
    )?;
    if !exits {
        instructions.extend(append_scope_end_drops_before_exit(
            vec![Instruction::Return],
            &mut fallback_context,
        )?);
    }
    Ok(FallibleFailureMode::Handle { instructions })
}

fn lower_aggregate_otherwise_fallback_to_location(
    block: &Block,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
) -> Result<(Vec<Instruction>, bool), Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        let mut instructions = lower_otherwise_return_leading_statements(block, context, "E8007")?;
        if let Some(terminating_instructions) =
            lower_never_expression_with_scope_drops(result, context)?
        {
            instructions.extend(terminating_instructions);
            return Ok((instructions, true));
        }
        instructions.extend(lower_aggregate_return_expression_to_location(
            result,
            return_type,
            destination,
            function_name,
            resolved,
            context,
        )?);
        return Ok((instructions, false));
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let mut instructions = lower_otherwise_return_statement_prefix(leading, context, "E8007")?;
    match terminal {
        Stmt::Return(statement) => {
            instructions.extend(lower_return_statement_with_scope_drops(
                statement, context, "E8007",
            )?);
            Ok((instructions, true))
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(&statement.expression, context)?
            else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            instructions.extend(terminating_instructions);
            Ok((instructions, true))
        }
        _ => Err(unsupported_aggregate_return_diagnostic(function_name)),
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn statement_allows_implicit_void_return(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::Binding(_)
            | Stmt::Assignment(_)
            | Stmt::Drop(_)
            | Stmt::ForRange(_)
            | Stmt::While(_)
            | Stmt::Loop(_)
    )
}

fn statement_is_import(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Import(_) | Stmt::FromImport(_))
}

pub(super) fn reachable_body_prefix<'a>(
    statements: &'a [Stmt],
    result: Option<&'a Expr>,
    context: &LoweringContext,
) -> (&'a [Stmt], Option<&'a Expr>) {
    for (index, statement) in statements.iter().enumerate() {
        if statement_exits_function(statement, context) {
            return (&statements[..=index], None);
        }
    }

    (statements, result)
}

fn lower_aggregate_local_return_to_location(
    name: &str,
    value_use: AggregateValueUse,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(local) = context.aggregate_local(name) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if local.layout != expected_layout
        || (value_use == AggregateValueUse::ImplicitCopy && !local.is_copy)
    {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    Ok(vec![Instruction::CopyAggregate {
        destination,
        source: AggregateLocation::Slot(local.slot_index),
        layout: local.layout,
    }])
}

fn lower_aggregate_member_return_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let access = lower_aggregate_member_field_access(expression, context, &mut temporaries)?
        .ok_or_else(|| unsupported_aggregate_return_diagnostic(function_name))?;
    let source = access.source;
    let source_offset = access.offset;
    let is_copy = access.is_copy;
    let Some(layout) = access.kind.copy_aggregate_layout() else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if layout != expected_layout || !is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let mut instructions = access.instructions;
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset: 0,
        source,
        source_offset,
        layout,
    });
    Ok(instructions)
}

fn lower_aggregate_fallible_call_return_to_location(
    call: &crate::ast::CallExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if success_type.as_ref() != return_type {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }
    validate_aggregate_call_success_return_passing(&target, return_type, function_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    let (layout, _) = aggregate_return_layout_and_destination(return_type);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        return_type,
        destination,
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok(instructions)
}

fn lower_aggregate_call_return_to_location(
    call: &crate::ast::CallExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if macos_syscall_primitive_call(call, context)
        && let Some(layout) = aggregate_call_return_layout_from_resolved(call, context)
    {
        let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
        if layout != expected_layout {
            return Err(unsupported_aggregate_return_diagnostic(function_name));
        }
        let mut temporaries = TemporaryAllocator::new(context)?;
        let Some(instructions) = lower_macos_syscall_primitive_call_to_location(
            call,
            destination,
            expected_layout,
            context,
            &mut temporaries,
        )?
        else {
            return Err(unsupported_aggregate_return_diagnostic(function_name));
        };
        return Ok(instructions);
    }

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let Some(callee_return_type) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if callee_return_type != return_type {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }
    validate_aggregate_call_success_return_passing(&target, return_type, function_name, context)?;

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    let (layout, _) = aggregate_return_layout_and_destination(return_type);
    push_aggregate_call_instruction(
        &mut instructions,
        return_type,
        destination,
        target,
        arguments,
        layout,
    );
    Ok(instructions)
}

fn lower_aggregate_struct_literal_return_to_location(
    literal: &StructLiteralExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);

    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_aggregate_struct_literal_to_location(
        literal,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    Ok(match lowered_direct {
        Ok(instructions) => instructions,
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_aggregate_struct_literal_return_through_slot(
                literal,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    })
}

fn lower_aggregate_array_literal_return_to_location(
    literal: &ArrayLiteralExpr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(value) = fixed_array_return_abi_value(resolved, context) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if !matches!(&value.ty, AbiType::Array { .. }) || value.layout != expected_layout {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_aggregate_array_literal_to_location(
        literal,
        &value.ty,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    Ok(match lowered_direct {
        Ok(instructions) => instructions,
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_aggregate_array_literal_return_through_slot(
                literal,
                &value.ty,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    })
}

fn fixed_array_return_abi_value(
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Option<AbiValue> {
    let mut ty = context.function_return_type_expr()?;
    loop {
        match ty {
            TypeExpr::Fallible(fallible) => ty = &fallible.success,
            TypeExpr::Optional(optional) => ty = &optional.inner,
            _ => {
                return abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok();
            }
        }
    }
}

fn payload_enum_return_abi_value(
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Option<AbiValue> {
    let mut ty = context.function_return_type_expr()?;
    loop {
        match ty {
            TypeExpr::Fallible(fallible) => ty = &fallible.success,
            TypeExpr::Optional(optional) => ty = &optional.inner,
            _ => {
                let value = abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok()?;
                return matches!(value.ty, AbiType::Enum(_)).then_some(value);
            }
        }
    }
}

fn lower_payload_enum_constructor_return_to_location(
    expression: &Expr,
    return_type: &Type,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let (expected_layout, _) = aggregate_return_layout_and_destination(return_type);
    let Some(value) = payload_enum_return_abi_value(resolved, context) else {
        return Ok(None);
    };
    if value.layout != expected_layout {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let subject = format!("returns from function `{function_name}`");
    let aggregate_slot_mark = context.aggregate_slot_mark();
    let lowered_direct = lower_payload_enum_constructor_to_location(
        expression,
        &value.ty,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    let instructions = match lowered_direct {
        Ok(Some(instructions)) => instructions,
        Ok(None) => return Ok(None),
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            context.restore_aggregate_slot_mark(aggregate_slot_mark);
            lower_direct_payload_enum_constructor_return_through_slot(
                expression,
                &value.ty,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    };
    Ok(Some(instructions))
}

fn lower_direct_payload_enum_constructor_return_through_slot(
    expression: &Expr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    let Some(mut constructor_instructions) = lower_payload_enum_constructor_to_location(
        expression,
        expected_type,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
    )?
    else {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    };
    instructions.append(&mut constructor_instructions);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

fn lower_direct_aggregate_array_literal_return_through_slot(
    literal: &ArrayLiteralExpr,
    expected_type: &AbiType,
    expected_layout: ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_array_literal_to_location(
        literal,
        expected_type,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

fn lower_direct_aggregate_struct_literal_return_through_slot(
    literal: &StructLiteralExpr,
    expected_layout: crate::abi::ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
        literal,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
        &mut temporaries,
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

fn aggregate_return_layout_and_destination(
    return_type: &Type,
) -> (crate::abi::ValueLayout, AggregateLocation) {
    match return_type {
        Type::Aggregate { layout } => (*layout, AggregateLocation::Return),
        Type::DirectAggregate { layout, .. } => (*layout, AggregateLocation::DirectReturn),
        _ => unreachable!("aggregate return lowering requires aggregate return type"),
    }
}

fn validate_aggregate_call_success_return_passing(
    target: &CallTarget,
    return_type: &Type,
    function_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(actual) = context.call_success_return_passing(target) else {
        return Ok(());
    };
    let Some(expected) = return_type.success_return_passing() else {
        return Ok(());
    };
    if actual == expected {
        return Ok(());
    }

    Err(aggregate_call_return_abi_mismatch_diagnostic(
        function_name,
        expected,
        actual,
    ))
}

fn aggregate_call_return_abi_mismatch_diagnostic(
    function_name: &str,
    expected: crate::abi::ReturnPassing,
    actual: crate::abi::ReturnPassing,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 aggregate return ABI mismatch in function `{function_name}`: expected callee success return to use `{}`, got `{}`",
            expected.description(),
            actual.description(),
        ),
    )]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AggregateValueUse {
    ImplicitCopy,
    ExplicitMove,
}

fn unsupported_aggregate_return_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower aggregate returns from function `{function_name}` from a supported struct literal, an aggregate call, or a supported aggregate local slot"
        ),
    )]
}

fn macos_syscall_primitive_call(call: &crate::ast::CallExpr, context: &LoweringContext) -> bool {
    matches!(
        context.primitive_name_for_call(call),
        Some(
            "syscall0"
                | "syscall1"
                | "syscall2"
                | "syscall3"
                | "syscall4"
                | "syscall5"
                | "syscall6"
        )
    )
}

fn lower_fallible_failure(payload: ErrorPayload) -> Vec<Instruction> {
    payload.into_return_instructions()
}

fn lower_otherwise_scalar_return_with_scope_drops(
    expression: &Expr,
    success_type: &Type,
    return_type: &Type,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if !otherwise_return_supports_success_type(success_type) {
        return Ok(None);
    }

    let Expr::Otherwise(otherwise) = unwrap_group(expression) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };
    if !call_return_type_expr_is_top_level_optional(call, context) {
        return Ok(None);
    }

    mark_explicit_moves_in_expression(&otherwise.value, context);
    let failure_mode =
        lower_otherwise_return_failure_mode(&otherwise.fallback, context, diagnostic_code)?;
    if !context.pending_aggregate_drops().is_empty() {
        let mut instructions = lower_otherwise_scalar_return_call_to_temporary(
            call,
            success_type,
            context,
            failure_mode,
        )?;
        append_scope_drops_then_restore_scalar_return(
            &mut instructions,
            success_type,
            return_type,
            context,
        )?;
        return Ok(Some(instructions));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let mut instructions = lower_otherwise_scalar_return_call_to_return(
        call,
        success_type,
        context,
        &mut temporaries,
        failure_mode,
    )?;
    instructions.push(success_return_instruction(return_type));
    append_scope_end_drops_before_exit(instructions, context).map(Some)
}

fn otherwise_return_supports_success_type(success_type: &Type) -> bool {
    matches!(
        success_type,
        Type::I32 | Type::U8 | Type::Usize | Type::Bool | Type::Str | Type::Slice { .. }
    )
}

fn lower_otherwise_scalar_return_call_to_return(
    call: &CallExpr,
    success_type: &Type,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match success_type {
        Type::I32 => lower_fallible_i32_normal_call(
            call,
            I32Location::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::U8 => lower_fallible_u8_normal_call(
            call,
            U8Location::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Usize => lower_fallible_usize_normal_call(
            call,
            UsizeLocation::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Bool => lower_fallible_bool_normal_call(
            call,
            BoolLocation::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Str => lower_fallible_str_normal_call(
            call,
            StrLocation::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Slice { .. } => lower_fallible_slice_normal_call(
            call,
            SliceLocation::Return,
            context,
            temporaries,
            failure_mode,
        ),
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Borrow { .. }
        | Type::Void
        | Type::Never
        | Type::Fallible(_) => Err(vec![Diagnostic::error(
            "E8007",
            "IR v0 can only lower `otherwise` returns for scalar success types",
        )]),
    }
}

fn lower_otherwise_scalar_return_call_to_temporary(
    call: &CallExpr,
    success_type: &Type,
    context: &LoweringContext,
    failure_mode: FallibleFailureMode,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match success_type {
        Type::I32 => {
            let destination = context.next_i32_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_i32_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::U8 => {
            let destination = context.next_u8_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_u8_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Usize => {
            let destination = context.next_usize_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_usize_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Bool => {
            let destination = context.next_bool_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(1);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_bool_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Str => {
            let destination = context.next_str_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(2);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_str_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Slice { .. } => {
            let destination = context.next_slice_local_location()?;
            let expression_context = context.with_reserved_local_abi_words(2);
            let mut temporaries = TemporaryAllocator::new(&expression_context)?;
            lower_fallible_slice_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )
        }
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Borrow { .. }
        | Type::Void
        | Type::Never
        | Type::Fallible(_) => Err(vec![Diagnostic::error(
            "E8007",
            "IR v0 can only lower `otherwise` returns for scalar success types",
        )]),
    }
}

fn append_scope_drops_then_restore_scalar_return(
    instructions: &mut Vec<Instruction>,
    success_type: &Type,
    return_type: &Type,
    context: &mut LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let restore_return = match success_type {
        Type::I32 => vec![Instruction::SetI32 {
            destination: I32Location::Return,
            value: I32Value::Location(context.next_i32_local_location()?),
        }],
        Type::U8 => vec![Instruction::SetU8 {
            destination: U8Location::Return,
            value: U8Value::Location(context.next_u8_local_location()?),
        }],
        Type::Usize => vec![Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::Location(context.next_usize_local_location()?),
        }],
        Type::Bool => vec![Instruction::SetBool {
            destination: BoolLocation::Return,
            value: BoolValue::Location(context.next_bool_local_location()?),
        }],
        Type::Str => vec![Instruction::SetStr {
            destination: StrLocation::Return,
            value: StrValue::Location(context.next_str_local_location()?),
        }],
        Type::Slice { .. } => vec![Instruction::SetSlice {
            destination: SliceLocation::Return,
            value: SliceValue::Location(context.next_slice_local_location()?),
        }],
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Borrow { .. }
        | Type::Void
        | Type::Never
        | Type::Fallible(_) => {
            return Err(vec![Diagnostic::error(
                "E8007",
                "IR v0 can only restore `otherwise` returns for scalar success types",
            )]);
        }
    };
    append_scope_drops_then_restore_return(
        instructions,
        restore_return,
        scalar_return_temporary_abi_words(success_type)?,
        return_type,
        context,
    )
}

fn scalar_return_temporary_abi_words(success_type: &Type) -> Result<usize, Vec<Diagnostic>> {
    match success_type {
        Type::I32 | Type::U8 | Type::Usize | Type::Bool => Ok(1),
        Type::Str | Type::Slice { .. } => Ok(2),
        Type::Aggregate { .. }
        | Type::DirectAggregate { .. }
        | Type::Error
        | Type::Borrow { .. }
        | Type::Void
        | Type::Never
        | Type::Fallible(_) => Err(vec![Diagnostic::error(
            "E8007",
            "IR v0 can only restore `otherwise` returns for scalar success types",
        )]),
    }
}

fn lower_otherwise_aggregate_return_with_scope_drops(
    expression: &Expr,
    success_type: &Type,
    function_return_type: &Type,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    if context.pending_aggregate_drops().is_empty() {
        return Ok(None);
    }
    let Some(layout) = aggregate_type_layout(success_type) else {
        return Ok(None);
    };
    if !supported_aggregate_copy_layout(layout) {
        return Ok(None);
    }

    let Expr::Otherwise(otherwise) = unwrap_group(expression) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };
    if !call_return_type_expr_is_top_level_optional(call, context) {
        return Ok(None);
    }

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    };
    let function_name = context.function_name().to_string();
    let (_, destination) = aggregate_return_layout_and_destination(success_type);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let staged_destination = AggregateLocation::Slot(slot_index);
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];

    mark_explicit_moves_in_expression(&otherwise.value, context);
    let failure_mode = lower_aggregate_otherwise_return_failure_mode_with_scope_drops(
        &otherwise.fallback,
        success_type,
        function_return_type,
        slot_index,
        destination,
        &function_name,
        resolved,
        context,
    )?;
    instructions.extend(lower_aggregate_fallible_call_return_to_location(
        call,
        success_type,
        staged_destination,
        &function_name,
        context,
        failure_mode,
    )?);
    append_scope_drops_then_restore_aggregate_return(
        &mut instructions,
        slot_index,
        layout,
        destination,
        function_return_type,
        context,
    )?;
    Ok(Some(instructions))
}

fn lower_aggregate_otherwise_return_failure_mode_with_scope_drops(
    fallback: &Block,
    success_type: &Type,
    function_return_type: &Type,
    slot_index: usize,
    destination: AggregateLocation,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let mut fallback_context = context.clone();
    mark_explicit_moves_in_block(fallback, &mut fallback_context);
    let layout = aggregate_type_layout(success_type)
        .ok_or_else(|| unsupported_aggregate_return_diagnostic(function_name))?;
    let (mut instructions, exits) = lower_aggregate_otherwise_fallback_to_location(
        fallback,
        success_type,
        AggregateLocation::Slot(slot_index),
        function_name,
        resolved,
        &mut fallback_context,
    )?;
    if !exits {
        append_scope_drops_then_restore_aggregate_return(
            &mut instructions,
            slot_index,
            layout,
            destination,
            function_return_type,
            &mut fallback_context,
        )?;
    }
    Ok(FallibleFailureMode::Handle { instructions })
}

fn append_scope_drops_then_restore_aggregate_return(
    instructions: &mut Vec<Instruction>,
    slot_index: usize,
    layout: ValueLayout,
    destination: AggregateLocation,
    function_return_type: &Type,
    context: &mut LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let mut tail = append_scope_end_drops_before_exit(
        vec![success_return_instruction(function_return_type)],
        context,
    )?;
    let Some(return_index) = tail.iter().rposition(is_scope_exit_instruction) else {
        return Ok(());
    };
    tail.insert(
        return_index,
        Instruction::CopyAggregate {
            destination,
            source: AggregateLocation::Slot(slot_index),
            layout,
        },
    );
    instructions.extend(tail);
    Ok(())
}

fn call_return_type_expr_is_top_level_optional(call: &CallExpr, context: &LoweringContext) -> bool {
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return false;
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return false;
    };
    return_type_expr_is_top_level_optional_with_resolver(&return_type, resolved, |source| {
        context.resolved_source(source)
    })
}

fn lower_otherwise_return_failure_mode(
    fallback: &Block,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let mut fallback_context = context.clone();
    let instructions =
        lower_otherwise_return_block(fallback, &mut fallback_context, diagnostic_code)?;
    Ok(FallibleFailureMode::Handle { instructions })
}

fn lower_otherwise_return_block(
    block: &Block,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        let mut instructions =
            lower_otherwise_return_leading_statements(block, context, diagnostic_code)?;
        if let Some(terminating_instructions) =
            lower_never_expression_with_scope_drops(result, context)?
        {
            instructions.extend(terminating_instructions);
            return Ok(instructions);
        }
        let fallback_return = ReturnStmt {
            span: result.span(),
            expression: Some((**result).clone()),
        };
        instructions.extend(lower_return_statement_with_scope_drops(
            &fallback_return,
            context,
            diagnostic_code,
        )?);
        return Ok(instructions);
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code));
    };
    let mut instructions =
        lower_otherwise_return_statement_prefix(leading, context, diagnostic_code)?;
    match terminal {
        Stmt::Return(statement) => {
            instructions.extend(lower_return_statement_with_scope_drops(
                statement,
                context,
                diagnostic_code,
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_expression_with_scope_drops(&statement.expression, context)?
            else {
                return Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code)),
    }
}

fn lower_otherwise_return_leading_statements(
    block: &Block,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_otherwise_return_statement_prefix(&block.statements, context, diagnostic_code)
}

fn lower_otherwise_return_statement_prefix(
    statements: &[Stmt],
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context)?)
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context)?)
            }
            Stmt::Drop(statement) => instructions.extend(lower_drop_statement(statement, context)?),
            Stmt::Expression(statement) => {
                let Some(effect) = lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code));
                };
                instructions.extend(effect);
            }
            _ => return Err(unsupported_otherwise_fallback_diagnostic(diagnostic_code)),
        }
    }
    Ok(instructions)
}

fn unsupported_otherwise_fallback_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower `otherwise` fallback blocks with local bindings, assignments, drops, effect-only calls, and a value, `return`, or `never` tail",
    )]
}

fn expression_is_none_literal(expression: &Expr) -> bool {
    matches!(unwrap_group(expression), Expr::NoneLiteral(_))
}

fn unsupported_function_body_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower function `{function_name}` bodies containing leading scalar local bindings, scalar assignments, or effect-only call statements followed by `return`"
        ),
    )]
}

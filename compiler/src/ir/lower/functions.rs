use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    aggregate_type_layout, lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_with_temporaries, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
    type_expr_is_copy_struct,
};
use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{
    AggregateBorrowParameter, AggregateFieldKind, AggregateParameterSource, ErrorPayloads,
    FunctionNames, FunctionSignatures, LoweringAggregateParameter, LoweringContext,
    LoweringParameterSlots, PendingAggregateDrop, drop_glue_for_type_expr,
};
use super::control_flow::{
    TerminalBranch, lower_nonterminal_for_range_statement, lower_nonterminal_if_statement,
    lower_nonterminal_loop_statement, lower_nonterminal_while_statement,
    lower_terminal_bool_if_statement, lower_terminal_branch_leading_statements,
    lower_terminal_condition, lower_terminal_i32_if_statement, lower_terminal_slice_if_statement,
    lower_terminal_str_if_statement, lower_terminal_u8_if_statement,
    lower_terminal_usize_if_statement, lower_terminal_void_if_statement,
    split_terminal_branch_block,
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
    borrow_inner_type, borrow_type_from_type_expr, parameter_type_from_type_expr,
    return_type_expr_is_top_level_optional, return_type_from_type_expr, type_expr_with_self_type,
    view_element_type_from_type_expr,
};
use crate::abi::{
    AbiType, AbiValue, ValueClassification, ValueLayout, abi_value_from_type_expr,
    function_parameter_abi_word_count_from_signature,
};
use crate::ast::{
    BinaryExpr, BinaryOperator, Block, CallExpr, DropDecl, DropStmt, Expr, FunctionDecl,
    IdentifierExpr, IfIsStmt, IfStmt, MemberExpr, MethodDecl, Parameter, ReturnStmt, Stmt,
    StructLiteralExpr, SwitchArm, SwitchStmt, TypeExpr, TypeReference, UnaryOperator,
    substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, BoolLocation, BoolValue, BorrowArgument, BorrowSource, CallTarget,
    FallibleFailureMode, Function, I32Location, I32Value, Instruction, ScalarArgument,
    SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation,
    UsizeValue,
};
use crate::resolve::{
    FunctionSignature as ResolvedFunctionSignature, ParameterSignature, ResolveOutput,
};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{TypecheckFacts, TypecheckSliceElementKind};
use std::collections::HashMap;

pub(super) fn lower_function(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
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
    let parameter_slots =
        lower_scalar_parameters(&name, &parameters, root_source, resolved, sources).map_err(
            |diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, function.parameters.span)
            },
        )?;
    validate_parameter_slots_match_function_abi(
        &name,
        &parameters,
        &return_type_expr,
        resolved,
        &parameter_slots,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, function.parameters.span)
    })?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameter_slots);
    let return_type = match lower_function_return_type(&return_type_expr, &name, resolved) {
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
    .with_function_returns_optional(return_type_expr_is_top_level_optional(
        &return_type_expr,
        resolved,
    ))
    .with_call_resolution(root_source, resolved, typecheck_facts, function_names)
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

pub(super) fn lower_drop_function(
    drop_: &DropDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
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
        sources,
    )
    .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, binding.span))?;
    validate_parameter_slots_match_function_abi(
        &name,
        std::slice::from_ref(&binding),
        &void_type_expr(drop_.span),
        resolved,
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
    .with_call_resolution(root_source, resolved, typecheck_facts, function_names)
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

pub(super) fn lower_method_function(
    method: &MethodDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
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
    let parameter_slots =
        lower_scalar_parameters(&name, &parameters, root_source, resolved, sources).map_err(
            |diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, method.parameters.span)
            },
        )?;
    validate_parameter_slots_match_function_abi(
        &name,
        &parameters,
        &return_type_expr,
        resolved,
        &parameter_slots,
    )
    .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, method.span))?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameter_slots);
    let return_type = match lower_function_return_type(&return_type_expr, &name, resolved) {
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
    .with_function_returns_optional(return_type_expr_is_top_level_optional(
        &return_type_expr,
        resolved,
    ))
    .with_call_resolution(root_source, resolved, typecheck_facts, function_names)
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
    sources: &SourceMap,
) -> Result<LoweringParameterSlots, Vec<Diagnostic>> {
    let mut slots = LoweringParameterSlots::default();
    for parameter in parameters {
        match lower_scalar_parameter_kind(parameter, function_name, root_source, resolved).map_err(
            |diagnostics| attach_primary_span_if_absent(diagnostics, sources, parameter.span),
        )? {
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
            ScalarParameterKind::Slice(element_kind) => {
                slots.push_slice_parameter(parameter.name.clone(), element_kind);
                slots.push_empty_abi_word();
            }
            ScalarParameterKind::Error => {
                slots.push_error_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Borrow => {
                slots.push_empty_abi_word();
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
                    is_copy: type_expr_is_copy_struct(&parameter.ty, resolved),
                    drop_glue: drop_glue_for_type_expr(&parameter.ty, root_source, resolved),
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
                    is_copy: type_expr_is_copy_struct(&parameter.ty, resolved),
                    drop_glue: drop_glue_for_type_expr(&parameter.ty, root_source, resolved),
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
    slots: &LoweringParameterSlots,
) -> Result<(), Vec<Diagnostic>> {
    let signature = resolved_function_signature(parameters, return_type.clone());
    let expected = function_parameter_abi_word_count_from_signature(&signature, resolved)
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
    Slice(TypecheckSliceElementKind),
    Error,
    Borrow,
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
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    match parameter_type_from_type_expr(&parameter.ty, resolved) {
        Some(Type::I32) => return Ok(ScalarParameterKind::I32),
        Some(Type::U8) => return Ok(ScalarParameterKind::U8),
        Some(Type::Usize) => return Ok(ScalarParameterKind::Usize),
        Some(Type::Bool) => return Ok(ScalarParameterKind::Bool),
        Some(Type::Str) => return Ok(ScalarParameterKind::Str),
        Some(Type::Slice { .. }) => {
            return Ok(ScalarParameterKind::Slice(
                slice_element_kind_from_type_expr(&parameter.ty, resolved),
            ));
        }
        Some(Type::Error) => return Ok(ScalarParameterKind::Error),
        _ => {}
    }

    let value = abi_value_from_type_expr(&parameter.ty, resolved)
        .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    match &value.ty {
        AbiType::Borrow => {
            lower_borrow_parameter_kind(parameter, function_name, root_source, resolved)
        }
        AbiType::Struct(_) => {
            lower_aggregate_parameter_kind(parameter, function_name, root_source, resolved, &value)
        }
        _ => Err(unsupported_parameter_type_diagnostic(function_name)),
    }
}

fn slice_element_kind_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> TypecheckSliceElementKind {
    match view_element_type_from_type_expr(ty, resolved) {
        Some(Type::I32) => TypecheckSliceElementKind::I32,
        Some(Type::U8) => TypecheckSliceElementKind::U8,
        Some(Type::Usize) => TypecheckSliceElementKind::Usize,
        Some(Type::Bool) => TypecheckSliceElementKind::Bool,
        Some(Type::Str) => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}

fn lower_borrow_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let Some(borrow) = borrow_type_from_type_expr(&parameter.ty, resolved) else {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    };
    match borrow_inner_type(&borrow.inner, resolved) {
        Some(Type::Aggregate { .. } | Type::DirectAggregate { .. }) => {
            lower_aggregate_borrow_parameter_kind(parameter, function_name, root_source, resolved)
        }
        Some(_) => Ok(ScalarParameterKind::Borrow),
        None => Err(unsupported_parameter_type_diagnostic(function_name)),
    }
}

fn lower_aggregate_borrow_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let Some(borrow) = borrow_type_from_type_expr(&parameter.ty, resolved) else {
        unreachable!("aggregate borrow parameter lowering requires a borrow type");
    };
    let value = abi_value_from_type_expr(&borrow.inner, resolved)
        .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    if !matches!(value.ty, AbiType::Struct(_)) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    let fields = aggregate_fields_from_type_expr(&borrow.inner, root_source, resolved)
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
    value: &AbiValue,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    if !matches!(value.ty, AbiType::Struct(_)) || !supported_aggregate_copy_layout(value.layout) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    let fields = aggregate_fields_from_type_expr(&parameter.ty, root_source, resolved)
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
            "IR v0 can only lower `i32`, `u8`, `usize`, `bool`, `&str`, `&[T]`, `&+[T]`, scalar borrow parameters, aggregate borrow parameters, and aggregate value parameters with non-empty ABI layouts for function `{function_name}`"
        ),
    )]
}

fn lower_function_return_type(
    ty: &TypeExpr,
    name: &str,
    resolved: &ResolveOutput,
) -> Result<Type, Vec<Diagnostic>> {
    return_type_from_type_expr(ty, resolved)
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
    let statements = body.statements.as_slice();

    if statements.is_empty() && body.result.is_none() && *success_type == Type::Void {
        return Ok(vec![success_return_instruction(return_type)]);
    }

    if let Some(result) = &body.result {
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
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8007")
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                })?;
            let Some(branch_instructions) = lower_terminal_if_statement_for_success_type(
                &if_statement,
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
        Stmt::Switch(statement) => {
            let switch = payloadless_switch_as_if_statement(statement, context, "E8007").map_err(
                |diagnostics| attach_primary_span_if_absent(diagnostics, sources, statement.span),
            )?;
            instructions.extend(switch.leading_instructions);
            let Some(branch_instructions) = lower_terminal_if_statement_for_success_type(
                &switch.if_statement,
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
        Expr::If(statement) => lower_terminal_if_statement_for_success_type(
            statement,
            context,
            function_name,
            return_type,
            "E8007",
            "functions",
            context
                .resolved_calls()
                .map(|(_, resolved)| resolved)
                .ok_or_else(|| unsupported_function_body_diagnostic(function_name))?,
            sources,
        ),
        Expr::IfIs(statement) => {
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8007")?;
            lower_terminal_if_statement_for_success_type(
                &if_statement,
                context,
                function_name,
                return_type,
                "E8007",
                "functions",
                context
                    .resolved_calls()
                    .map(|(_, resolved)| resolved)
                    .ok_or_else(|| unsupported_function_body_diagnostic(function_name))?,
                sources,
            )
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_if_statement(statement, context, "E8007")?;
            let Some(mut branch_instructions) = lower_terminal_if_statement_for_success_type(
                &switch.if_statement,
                context,
                function_name,
                return_type,
                "E8007",
                "functions",
                context
                    .resolved_calls()
                    .map(|(_, resolved)| resolved)
                    .ok_or_else(|| unsupported_function_body_diagnostic(function_name))?,
                sources,
            )?
            else {
                return Ok(None);
            };
            let mut instructions = switch.leading_instructions;
            instructions.append(&mut branch_instructions);
            Ok(Some(instructions))
        }
        _ => Ok(None),
    }
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
            let if_statement =
                payloadless_if_is_as_if_statement(statement, context, diagnostic_code)?;
            lower_terminal_if_statement_for_success_type(
                &if_statement,
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
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_if_statement(statement, context, diagnostic_code)?;
            let Some(mut branch_instructions) = lower_terminal_if_statement_for_success_type(
                &switch.if_statement,
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
            )?
            else {
                return Ok(None);
            };
            let mut instructions = switch.leading_instructions;
            instructions.append(&mut branch_instructions);
            Ok(Some(instructions))
        }
        _ => Ok(None),
    }
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
    let success_type = return_type.success_type();
    let branch_instructions = match success_type {
        Type::I32 => lower_terminal_i32_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Bool => lower_terminal_bool_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::U8 => lower_terminal_u8_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Usize => lower_terminal_usize_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Str => lower_terminal_str_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Slice { .. } => lower_terminal_slice_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Void => lower_terminal_void_if_statement(
            statement,
            context,
            return_type,
            diagnostic_code,
            subject,
            sources,
        )?,
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
            lower_terminal_aggregate_if_statement(
                statement,
                context,
                success_type,
                function_name,
                resolved,
                sources,
            )?
        }
        Type::Never | Type::Fallible(_) | Type::Borrow { .. } | Type::Error => return Ok(None),
    };

    Ok(Some(mark_fallible_success_returns(
        return_type,
        branch_instructions,
    )))
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

pub(super) struct LoweredPayloadlessSwitch {
    pub(super) leading_instructions: Vec<Instruction>,
    pub(super) if_statement: IfStmt,
}

pub(super) fn payloadless_switch_as_if_statement(
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
    let if_statement = payloadless_switch_if_chain(
        statement,
        target_expression,
        &variant_names,
        diagnostic_code,
    )?;

    Ok(LoweredPayloadlessSwitch {
        leading_instructions,
        if_statement,
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

fn payloadless_switch_variant_names(
    statement: &SwitchStmt,
    context: &LoweringContext,
) -> Option<Vec<String>> {
    let Some(first_arm) = statement.arms.first() else {
        return None;
    };
    let Some((_, resolved)) = context.resolved_calls() else {
        return None;
    };
    let Some(target_symbol) = resolved.type_symbol_by_name(&first_arm.enum_name) else {
        return None;
    };
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

fn payloadless_switch_if_chain(
    statement: &SwitchStmt,
    target: Expr,
    variant_names: &[String],
    diagnostic_code: &'static str,
) -> Result<IfStmt, Vec<Diagnostic>> {
    let Some((condition_arms, fallback)) =
        payloadless_switch_condition_arms_and_fallback(statement, variant_names)
    else {
        return Err(unsupported_switch_diagnostic(diagnostic_code));
    };

    let mut next_else = Some(fallback);
    let mut current = None;
    for arm in condition_arms.iter().rev() {
        let if_statement = IfStmt {
            span: arm.span,
            condition: Expr::Binary(BinaryExpr {
                span: arm.span,
                left: Box::new(target.clone()),
                operator: BinaryOperator::Equal,
                operator_span: arm.span,
                right: Box::new(payloadless_switch_variant_expression(arm)),
            }),
            then_block: arm.body.clone(),
            else_block: next_else,
        };
        next_else = Some(Block {
            span: if_statement.span,
            statements: vec![Stmt::If(if_statement.clone())],
            result: None,
        });
        current = Some(if_statement);
    }

    current.ok_or_else(|| unsupported_switch_diagnostic(diagnostic_code))
}

fn payloadless_switch_condition_arms_and_fallback<'a>(
    statement: &'a SwitchStmt,
    variant_names: &[String],
) -> Option<(&'a [SwitchArm], Block)> {
    if let Some(else_arm) = &statement.else_arm {
        return Some((&statement.arms, else_arm.body.clone()));
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
        "IR v0 can only lower payloadless `if is` enum pattern branches",
    )]
}

fn unsupported_switch_diagnostic(diagnostic_code: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        diagnostic_code,
        "IR v0 can only lower payloadless enum `match` statements",
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
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
    };

    let then_instructions = lower_terminal_aggregate_return_block(
        &statement.then_block,
        context,
        &statement.condition,
        success_type,
        function_name,
        resolved,
        sources,
    )?;
    let else_instructions = lower_terminal_aggregate_return_block(
        else_block,
        context,
        &statement.condition,
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

fn lower_terminal_aggregate_return_block(
    block: &Block,
    context: &LoweringContext,
    pre_moved_expression: &Expr,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) =
        split_terminal_branch_block(block, "E8007", "functions", "aggregate")?;
    let mut branch_context = context.clone();
    mark_explicit_moves_in_expression(pre_moved_expression, &mut branch_context);
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        "E8007",
        "functions",
        "aggregate",
        sources,
    )?;

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
            let if_statement =
                payloadless_if_is_as_if_statement(statement, &branch_context, "E8007")?;
            instructions.extend(lower_terminal_aggregate_if_statement(
                &if_statement,
                &branch_context,
                success_type,
                function_name,
                resolved,
                sources,
            )?);
            Ok(instructions)
        }
        TerminalBranch::Statement(Stmt::Switch(statement)) => {
            let switch =
                payloadless_switch_as_if_statement(statement, &mut branch_context, "E8007")?;
            instructions.extend(switch.leading_instructions);
            instructions.extend(lower_terminal_aggregate_if_statement(
                &switch.if_statement,
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
            let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8007")?;
            lower_terminal_aggregate_if_statement(
                &if_statement,
                context,
                success_type,
                function_name,
                resolved,
                sources,
            )
        }
        Expr::Match(statement) => {
            let switch = payloadless_switch_as_if_statement(statement, context, "E8007")?;
            let mut instructions = switch.leading_instructions;
            instructions.extend(lower_terminal_aggregate_if_statement(
                &switch.if_statement,
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
    return_type: &Type,
    context: &mut LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let mut tail =
        append_scope_end_drops_before_exit(vec![success_return_instruction(return_type)], context)?;
    let Some(return_index) = tail.iter().rposition(is_scope_exit_instruction) else {
        return Ok(());
    };
    tail.splice(return_index..return_index, restore_return);
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
                let if_statement = payloadless_if_is_as_if_statement(statement, context, "E8007")
                    .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.pattern_span)
                })?;
                instructions.extend(
                    lower_nonterminal_if_statement(
                        &if_statement,
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
            Stmt::Switch(statement) => {
                let switch = payloadless_switch_as_if_statement(statement, context, "E8007")
                    .map_err(|diagnostics| {
                        attach_primary_span_if_absent(diagnostics, sources, statement.span)
                    })?;
                instructions.extend(switch.leading_instructions);
                instructions.extend(
                    lower_nonterminal_if_statement(
                        &switch.if_statement,
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
    let Some(drop_glue) = local.drop_glue else {
        context.mark_aggregate_local_dropped(&statement.name);
        return Ok(Vec::new());
    };
    let Some(parameter_types) = context.call_parameter_types(&drop_glue.target) else {
        return Err(unsupported_drop_statement_diagnostic(&statement.name));
    };
    if parameter_types.len() != 1
        || !drop_parameter_matches_local(&parameter_types[0], local.layout)
    {
        return Err(unsupported_drop_statement_diagnostic(&statement.name));
    }

    context.mark_aggregate_local_dropped(&statement.name);
    Ok(vec![Instruction::CallVoid {
        target: drop_glue.target,
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(local.slot_index),
        })],
    }])
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
    Ok(vec![lower_pending_aggregate_drop(&drop_, context)?])
}

pub(super) fn lower_scope_end_drops_for_locals_since(
    context: &mut LoweringContext,
    local_mark: usize,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let pending = context.pending_aggregate_drops_since(local_mark);
    let mut instructions = Vec::with_capacity(pending.len());
    for drop_ in &pending {
        instructions.push(lower_pending_aggregate_drop(drop_, context)?);
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
    let mut instructions = Vec::with_capacity(pending.len());
    for drop_ in &pending {
        instructions.push(lower_pending_aggregate_drop(drop_, context)?);
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
) -> Result<Instruction, Vec<Diagnostic>> {
    let Some(parameter_types) = context.call_parameter_types(&drop_.drop_glue.target) else {
        return Err(unsupported_drop_statement_diagnostic(&drop_.name));
    };
    if parameter_types.len() != 1
        || !drop_parameter_matches_local(&parameter_types[0], drop_.layout)
    {
        return Err(unsupported_drop_statement_diagnostic(&drop_.name));
    }

    Ok(Instruction::CallVoid {
        target: drop_.drop_glue.target.clone(),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(drop_.slot_index),
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
            if let Some(arm) = &statement.else_arm {
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
            }) || statement.else_arm.as_ref().is_some_and(|arm| {
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
            }) || statement.else_arm.as_ref().is_some_and(|arm| {
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
        Expr::Call(call) => lower_aggregate_call_return_to_location(
            call,
            return_type,
            destination,
            function_name,
            context,
        ),
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
        Expr::Member(_) => lower_aggregate_member_return_to_location(
            expression,
            return_type,
            destination,
            function_name,
            context,
        ),
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
        || !supported_aggregate_copy_layout(local.layout)
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
    let AggregateFieldKind::Aggregate { layout, .. } = access.kind else {
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
    append_scope_drops_then_restore_return(instructions, restore_return, return_type, context)
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
    return_type_expr_is_top_level_optional(&return_type, resolved)
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

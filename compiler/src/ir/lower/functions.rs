use super::aggregates::{
    aggregate_call_return_layout_from_resolved, aggregate_fields_from_type_expr,
    lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_with_temporaries, push_aggregate_call_instruction,
    push_fallible_aggregate_call_instruction, supported_aggregate_copy_layout,
    type_expr_is_copy_struct,
};
use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{
    AggregateBorrowParameter, AggregateFieldKind, AggregateParameterSource, FunctionNames,
    FunctionSignatures, LoweringAggregateParameter, LoweringContext, LoweringParameterSlots,
    PendingAggregateDrop, drop_glue_for_type_expr,
};
use super::control_flow::{
    lower_nonterminal_if_statement, lower_nonterminal_while_statement,
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
    lower_catch_failure_mode, lower_i32_expression_to_location, lower_i32_return_expression,
    lower_macos_syscall_primitive_call_to_location, lower_never_return_expression,
    lower_slice_expression_to_location, lower_slice_return_expression,
    lower_str_expression_to_location, lower_str_return_expression, lower_u8_expression_to_location,
    lower_u8_return_expression, lower_usize_expression_to_location, lower_usize_return_expression,
    lower_void_expression_statement, mark_fallible_success_returns, success_return_instruction,
};
use crate::abi::{
    AbiType, AbiValue, ValueClassification, abi_value_from_type_expr,
    function_parameter_abi_word_count_from_signature,
};
use crate::ast::{
    ArrayType, Block, BorrowType, DropDecl, DropStmt, Expr, FallibleType, FunctionDecl,
    GenericType, IfStmt, MethodDecl, OptionalType, Parameter, PointerType, ReturnStmt, Stmt,
    StructLiteralExpr, TypeExpr, TypeReference, UnaryOperator, ViewType,
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
use crate::typecheck::TypecheckFacts;

pub(super) fn lower_function(
    function: &FunctionDecl,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() {
        return Err(attach_primary_span_if_absent(
            vec![Diagnostic::error(
                "E8007",
                format!(
                    "IR v0 can only lower non-generic functions, got `{}`",
                    function.name
                ),
            )],
            sources,
            function.generics.span.unwrap_or(function.span),
        ));
    }

    let parameters = lower_scalar_parameters(
        &function.name,
        &function.parameters.parameters,
        root_source,
        resolved,
        sources,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, function.parameters.span)
    })?;
    validate_parameter_slots_match_function_abi(
        &function.name,
        &function.parameters.parameters,
        &function.return_type,
        resolved,
        &parameters,
    )
    .map_err(|diagnostics| {
        attach_primary_span_if_absent(diagnostics, sources, function.parameters.span)
    })?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameters);
    let return_type =
        match lower_function_return_type(&function.return_type, &function.name, resolved) {
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
        function.name.clone(),
        success_type,
        function_signatures,
        parameters,
    )
    .with_function_return_type(return_type.clone())
    .with_call_resolution(root_source, resolved, typecheck_facts, function_names);
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
        name: function.name.clone(),
        target,
        return_type,
        instructions,
    })
}

pub(super) fn lower_drop_function(
    drop_: &DropDecl,
    self_ty: &TypeExpr,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
) -> Result<Function, Vec<Diagnostic>> {
    let binding = Parameter {
        span: drop_.binding.span,
        name: drop_.binding.name.clone(),
        name_span: drop_.binding.name_span,
        ty: type_expr_with_self_type(&drop_.binding.ty, self_ty),
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
    .with_call_resolution(root_source, resolved, typecheck_facts, function_names);
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
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
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

    let parameters = method_parameters(method, self_ty);
    let return_type_expr = type_expr_with_self_type(&method.return_type, self_ty);
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
    .with_call_resolution(root_source, resolved, typecheck_facts, function_names);
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

fn method_parameters(method: &MethodDecl, self_ty: &TypeExpr) -> Vec<Parameter> {
    let mut parameters = Vec::with_capacity(method.parameters.parameters.len() + 1);
    parameters.push(Parameter {
        span: method.receiver.span,
        name: method.receiver.name.clone(),
        name_span: method.receiver.name_span,
        ty: type_expr_with_self_type(&method.receiver.ty, self_ty),
    });
    parameters.extend(method.parameters.parameters.iter().cloned());
    parameters
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
        match lower_scalar_parameter_kind(parameter, function_name, resolved).map_err(
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
            ScalarParameterKind::Slice => {
                slots.push_slice_parameter(parameter.name.clone());
                slots.push_empty_abi_word();
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
    Slice,
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
    resolved: &ResolveOutput,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    match &parameter.ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(ScalarParameterKind::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Ok(ScalarParameterKind::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => {
            Ok(ScalarParameterKind::Usize)
        }
        TypeExpr::Reference(reference) if reference.name == "bool" => Ok(ScalarParameterKind::Bool),
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str") =>
        {
            Ok(ScalarParameterKind::Str)
        }
        TypeExpr::Borrow(borrow) if is_u8_slice_data_type(&borrow.inner) => {
            Ok(ScalarParameterKind::Slice)
        }
        TypeExpr::Borrow(borrow)
            if matches!(
                borrow_inner_type(&borrow.inner, resolved),
                Some(Type::Aggregate { .. } | Type::DirectAggregate { .. })
            ) =>
        {
            lower_aggregate_borrow_parameter_kind(parameter, function_name, resolved)
        }
        TypeExpr::Borrow(borrow) if borrow_inner_type(&borrow.inner, resolved).is_some() => {
            Ok(ScalarParameterKind::Borrow)
        }
        _ => lower_aggregate_parameter_kind(parameter, function_name, resolved),
    }
}

fn lower_aggregate_borrow_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    resolved: &ResolveOutput,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let TypeExpr::Borrow(borrow) = &parameter.ty else {
        unreachable!("aggregate borrow parameter lowering requires a borrow type");
    };
    let value = abi_value_from_type_expr(&borrow.inner, resolved)
        .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    if !matches!(value.ty, AbiType::Struct(_)) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    let fields = aggregate_fields_from_type_expr(&borrow.inner, resolved)
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
    resolved: &ResolveOutput,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let value = abi_value_from_type_expr(&parameter.ty, resolved)
        .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    if !matches!(value.ty, AbiType::Struct(_)) || !supported_aggregate_copy_layout(value.layout) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    let fields = aggregate_fields_from_type_expr(&parameter.ty, resolved)
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
            "IR v0 can only lower `i32`, `u8`, `usize`, `bool`, `&str`, `&[u8]`, `&+[u8]`, scalar borrow parameters, aggregate borrow parameters, and aggregate value parameters with non-empty ABI layouts for function `{function_name}`"
        ),
    )]
}

fn lower_function_return_type(
    ty: &TypeExpr,
    name: &str,
    resolved: &ResolveOutput,
) -> Result<Type, Vec<Diagnostic>> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Ok(Type::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => Ok(Type::Usize),
        TypeExpr::Reference(reference) if reference.name == "bool" => Ok(Type::Bool),
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str") =>
        {
            Ok(Type::Str)
        }
        TypeExpr::Borrow(borrow) if is_u8_slice_data_type(&borrow.inner) => Ok(Type::Slice {
            is_readwrite: borrow.is_readwrite,
        }),
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(Type::Void),
        TypeExpr::Reference(reference) if reference.name == "never" => Ok(Type::Never),
        TypeExpr::Fallible(fallible) => {
            lower_function_return_type(&fallible.success, name, resolved)
                .map(|success| Type::Fallible(Box::new(success)))
        }
        _ => lower_aggregate_function_return_type(ty, name, resolved),
    }
}

fn lower_aggregate_function_return_type(
    ty: &TypeExpr,
    name: &str,
    resolved: &ResolveOutput,
) -> Result<Type, Vec<Diagnostic>> {
    let value = abi_value_from_type_expr(ty, resolved)
        .map_err(|_error| unsupported_function_return_type_diagnostic(name))?;
    aggregate_type_from_abi_value(&value)
        .ok_or_else(|| unsupported_function_return_type_diagnostic(name))
}

fn unsupported_function_return_type_diagnostic(name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower function `{name}` return type `i32`, `u8`, `usize`, `bool`, `&str`, `&[u8]`, `&+[u8]`, `void`, `never`, aggregates, or a fallible form of those types"
        ),
    )]
}

fn is_u8_slice_data_type(ty: &TypeExpr) -> bool {
    matches!(
        ty,
        TypeExpr::View(view)
            if !view.is_readwrite
                && matches!(view.element.as_ref(), TypeExpr::Reference(reference) if reference.name == "u8")
    )
}

fn borrow_inner_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    let scalar = match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Some(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Some(Type::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => Some(Type::Usize),
        TypeExpr::Reference(reference) if reference.name == "bool" => Some(Type::Bool),
        _ => None,
    };
    if scalar.is_some() {
        return scalar;
    }

    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    aggregate_type_from_abi_value(&value)
}

fn aggregate_type_from_abi_value(value: &AbiValue) -> Option<Type> {
    if !matches!(value.ty, AbiType::Struct(_)) {
        return None;
    }

    match value.classification {
        ValueClassification::Indirect => Some(Type::Aggregate {
            layout: value.layout,
        }),
        ValueClassification::Direct { words } => Some(Type::DirectAggregate {
            layout: value.layout,
            words,
        }),
    }
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

    if statements.is_empty() && *success_type == Type::Void {
        return Ok(vec![success_return_instruction(return_type)]);
    }

    let Some((last, leading)) = statements.split_last() else {
        return Err(attach_primary_span_if_absent(
            unsupported_function_body_diagnostic(function_name),
            sources,
            body.span,
        ));
    };

    let mut instructions = lower_leading_bindings(leading, context).map_err(|diagnostics| {
        let span = leading
            .first()
            .map_or(body.span, |statement| statement.span());
        attach_primary_span_if_absent(diagnostics, sources, span)
    })?;

    match last {
        Stmt::Return(statement) => {
            let return_instructions = lower_return_statement_with_scope_drops(
                statement, context, "E8007",
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
        Stmt::If(statement) if success_type == &Type::I32 => {
            let branch_instructions = lower_terminal_i32_if_statement(
                statement,
                context,
                return_type,
                "E8007",
                "functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::Bool => {
            let branch_instructions = lower_terminal_bool_if_statement(
                statement,
                context,
                return_type,
                "E8007",
                "functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::U8 => {
            let branch_instructions = lower_terminal_u8_if_statement(
                statement,
                context,
                return_type,
                "E8007",
                "functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::Usize => {
            let branch_instructions = lower_terminal_usize_if_statement(
                statement,
                context,
                return_type,
                "E8007",
                "functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::Str => {
            let branch_instructions = lower_terminal_str_if_statement(
                statement,
                context,
                return_type,
                "E8007",
                "functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if matches!(success_type, Type::Slice { .. }) => {
            let branch_instructions = lower_terminal_slice_if_statement(
                statement,
                context,
                return_type,
                "E8007",
                "functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::Void => {
            let branch_instructions = lower_terminal_void_if_statement(
                statement,
                context,
                return_type,
                "E8007",
                "functions",
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement)
            if matches!(
                success_type,
                Type::Aggregate { .. } | Type::DirectAggregate { .. }
            ) =>
        {
            let branch_instructions = lower_terminal_aggregate_if_statement(
                statement,
                context,
                success_type,
                function_name,
                resolved,
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, statement.span)
            })?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) = lower_never_return_expression(
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
        _ => Err(attach_primary_span_if_absent(
            unsupported_function_body_diagnostic(function_name),
            sources,
            last.span(),
        )),
    }
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
        && let Some(return_instructions) = lower_never_return_expression(expression, context)?
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
        && let Some(return_instructions) =
            lower_value_return_with_scope_drops(&success_type, expression, &return_type, context)?
    {
        return Ok(return_instructions);
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
        (Type::Fallible(_), _) => unreachable!("fallible success type must be unwrapped"),
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
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(else_block) = &statement.else_block else {
        return Err(unsupported_terminal_aggregate_if_diagnostic(function_name));
    };

    lower_terminal_condition(
        &statement.condition,
        lower_terminal_aggregate_return_block(
            &statement.then_block,
            context,
            success_type,
            function_name,
            resolved,
        )?,
        lower_terminal_aggregate_return_block(
            else_block,
            context,
            success_type,
            function_name,
            resolved,
        )?,
        context,
        "E8007",
    )
}

fn lower_terminal_aggregate_return_block(
    block: &Block,
    context: &LoweringContext,
    success_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (terminal, leading) =
        split_terminal_branch_block(block, "E8007", "functions", "aggregate")?;
    let mut branch_context = context.clone();
    let mut instructions = lower_terminal_branch_leading_statements(
        leading,
        &mut branch_context,
        "E8007",
        "functions",
        "aggregate",
    )?;

    match terminal {
        Stmt::Return(statement) => {
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
        Stmt::If(statement) => {
            instructions.extend(lower_terminal_aggregate_if_statement(
                statement,
                &branch_context,
                success_type,
                function_name,
                resolved,
            )?);
            Ok(instructions)
        }
        _ => Err(unsupported_terminal_aggregate_if_diagnostic(function_name)),
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
    let Some(return_index) = tail
        .iter()
        .rposition(|instruction| is_scope_exit_instruction(instruction))
    else {
        return Ok(());
    };
    tail.splice(return_index..return_index, restore_return);
    instructions.extend(tail);
    Ok(())
}

pub(super) fn type_expr_with_self_type(ty: &TypeExpr, self_ty: &TypeExpr) -> TypeExpr {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "Self" => self_ty.clone(),
        TypeExpr::Reference(_) => ty.clone(),
        TypeExpr::Generic(generic) => TypeExpr::Generic(GenericType {
            span: generic.span,
            name: generic.name.clone(),
            name_span: generic.name_span,
            arguments: generic
                .arguments
                .iter()
                .map(|argument| type_expr_with_self_type(argument, self_ty))
                .collect(),
        }),
        TypeExpr::Pointer(pointer) => TypeExpr::Pointer(PointerType {
            span: pointer.span,
            inner: Box::new(type_expr_with_self_type(&pointer.inner, self_ty)),
        }),
        TypeExpr::Borrow(borrow) => TypeExpr::Borrow(BorrowType {
            span: borrow.span,
            is_readwrite: borrow.is_readwrite,
            inner: Box::new(type_expr_with_self_type(&borrow.inner, self_ty)),
        }),
        TypeExpr::View(view) => TypeExpr::View(ViewType {
            span: view.span,
            is_readwrite: view.is_readwrite,
            element: Box::new(type_expr_with_self_type(&view.element, self_ty)),
        }),
        TypeExpr::Array(array) => TypeExpr::Array(ArrayType {
            span: array.span,
            element: Box::new(type_expr_with_self_type(&array.element, self_ty)),
            length: array.length.clone(),
        }),
        TypeExpr::Optional(optional) => TypeExpr::Optional(OptionalType {
            span: optional.span,
            inner: Box::new(type_expr_with_self_type(&optional.inner, self_ty)),
        }),
        TypeExpr::Fallible(fallible) => TypeExpr::Fallible(FallibleType {
            span: fallible.span,
            success: Box::new(type_expr_with_self_type(&fallible.success, self_ty)),
            error: Box::new(type_expr_with_self_type(&fallible.error, self_ty)),
        }),
    }
}

fn lower_leading_bindings(
    statements: &[Stmt],
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context)?);
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context)?);
            }
            Stmt::Expression(statement) => {
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(vec![Diagnostic::error(
                        "E8007",
                        "IR v0 can only lower leading scalar local bindings, scalar assignments, drop statements, or void call statements before `return`",
                    )]);
                };
                instructions.extend(void_instructions);
            }
            Stmt::Drop(statement) => {
                instructions.extend(lower_drop_statement(statement, context)?);
            }
            Stmt::If(statement) => {
                instructions.extend(lower_nonterminal_if_statement(
                    statement,
                    context,
                    None,
                    "E8007",
                    "functions",
                )?);
            }
            Stmt::While(statement) => {
                instructions.extend(lower_nonterminal_while_statement(
                    statement,
                    context,
                    "E8007",
                    "functions",
                )?);
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "E8007",
                    "IR v0 can only lower leading scalar local bindings, scalar assignments, drop statements, void call statements, or supported non-terminal `if`/`while` statements before `return`",
                )]);
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

pub(super) fn append_scope_end_drops_before_exit(
    mut instructions: Vec<Instruction>,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(return_index) = instructions
        .iter()
        .rposition(|instruction| is_scope_exit_instruction(instruction))
    else {
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
        | Stmt::IfLet(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::While(_)
        | Stmt::WhileLet(_)
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
        Expr::OptionalDefault(default) => {
            mark_explicit_moves_in_expression(&default.value, context);
            mark_explicit_moves_in_expression(&default.default, context);
        }
        Expr::PatternConditional(conditional) => {
            mark_explicit_moves_in_expression(&conditional.target, context);
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
        Expr::OptionalDefault(default) => {
            expression_contains_explicit_aggregate_move_matching(
                &default.value,
                context,
                matches_move,
            ) || expression_contains_explicit_aggregate_move_matching(
                &default.default,
                context,
                matches_move,
            )
        }
        Expr::PatternConditional(conditional) => {
            expression_contains_explicit_aggregate_move_matching(
                &conditional.target,
                context,
                matches_move,
            )
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

fn lower_aggregate_return_expression(
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
    let (code, message) = payload.into_str_values();
    vec![Instruction::ReturnFallibleFailure { code, message }]
}

fn unsupported_function_body_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower function `{function_name}` bodies containing leading scalar local bindings, scalar assignments, or void call statements followed by `return`"
        ),
    )]
}

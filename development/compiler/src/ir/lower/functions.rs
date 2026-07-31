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

mod aggregate_returns;
mod callable_body;
mod diagnostics;
mod otherwise_returns;
mod parameters;
mod scope_drops;
mod switches;
mod value_returns;

use aggregate_returns::*;
use callable_body::*;
use diagnostics::*;
use otherwise_returns::*;
use parameters::*;
use scope_drops::*;
use switches::*;
use value_returns::*;

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
    pub(super) target_cleanup: Option<PatternTargetCleanup>,
}

#[derive(Clone, Copy)]
pub(super) struct PatternTargetCleanup {
    local_mark: usize,
}

impl PatternTargetCleanup {
    pub(super) fn append_to(
        self,
        instructions: &mut Vec<Instruction>,
        context: &mut LoweringContext,
    ) -> Result<(), Vec<Diagnostic>> {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            context,
            self.local_mark,
        )?);
        Ok(())
    }
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
                target_cleanup: None,
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
    let source = lower_payload_enum_pattern_target(
        &statement.expression,
        context,
        diagnostic_code,
        unsupported_if_is_diagnostic,
    )?;
    let then_prologue =
        tag_only_if_is_then_prologue(statement, source.slot_index, context, diagnostic_code)?;
    let target_name = tag_only_if_is_target_name(statement);
    let target = context.next_u8_local_location()?;
    context.define_u8_local(target_name.clone());

    let mut leading_instructions = source.leading_instructions;
    leading_instructions.push(Instruction::LoadAggregateU8 {
        destination: target,
        source: AggregateLocation::Slot(source.slot_index),
        offset: 0,
    });

    Ok(LoweredTagOnlyIfIs {
        leading_instructions,
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
        target_cleanup: source.cleanup,
    })
}

pub(super) struct LoweredPayloadlessSwitch {
    pub(super) leading_instructions: Vec<Instruction>,
    pub(super) body: LoweredPayloadlessSwitchBody,
    pub(super) target_cleanup: Option<PatternTargetCleanup>,
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
        target_cleanup: None,
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
    let source = lower_payload_enum_pattern_target(
        &statement.expression,
        context,
        diagnostic_code,
        unsupported_switch_diagnostic,
    )?;

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
        source.slot_index,
        context,
        diagnostic_code,
    )?;

    let mut leading_instructions = source.leading_instructions;
    leading_instructions.push(Instruction::LoadAggregateU8 {
        destination: target,
        source: AggregateLocation::Slot(source.slot_index),
        offset: 0,
    });

    Ok(LoweredPayloadlessSwitch {
        leading_instructions,
        body,
        target_cleanup: source.cleanup,
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

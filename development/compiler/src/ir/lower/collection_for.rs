//! Plan-driven lowering for protocol-based collection iteration.
//!
//! Protocol discovery stays in typecheck. This module materializes the
//! compiler-owned iterator, consumes one optional step result per iteration,
//! and delegates body exits to the ordinary scope/drop machinery.

use super::aggregates::{
    aggregate_fields_from_type_expr_with_resolver, push_aggregate_call_instruction,
    type_expr_is_copy_aggregate_value_with_resolver,
};
use super::context::LoweringContext;
use super::control_flow::instruction_list_ends_execution;
use super::control_flow::nonterminal::lower_nonterminal_loop_block_statements;
use super::expressions::{TemporaryAllocator, lower_call_arguments_with_explicit_types};
use super::functions::{
    lower_aggregate_return_expression_to_location, lower_scope_end_drops_for_locals_since,
};
use super::regions::CleanupScopeMark;
use crate::abi::AbiType;
use crate::ast::{
    BorrowExpr, BorrowType, CallExpr, CollectionForStmt, Expr, IdentifierExpr, MethodReceiverMode,
    TypeExpr, UnaryExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, BoolValue, ComposedOutcomeDestination, Instruction, Type};
use crate::outcomes::{OutcomeLayer, storage::outcome_storage_layout};
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::{
    TypecheckCollectionForPlan, TypecheckCollectionForSourceMode, TypecheckIterationMethod,
    TypecheckScalarViewKind,
};

pub(in crate::ir::lower) fn lower_collection_for_statement(
    statement: &CollectionForStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let plan = context
        .collection_for_plan(statement.span)
        .ok_or_else(|| iteration_diagnostic("the typecheck iteration plan is unavailable"))?;
    let iterator_mark = context.local_mark();
    let iterator_name = hidden_name(statement, "iterator");
    let (iterator_slot, iterator_layout, mut instructions) =
        materialize_iterator(statement, &plan, &iterator_name, context)?;

    let outcome_slot = context.reserve_aggregate_slot_index();
    let item_abi = context
        .abi_value_for_type_expr(&plan.item_type)
        .ok_or_else(|| iteration_diagnostic("the yielded item ABI layout is unavailable"))?;
    let item_ir = context
        .ir_type_for_type_expr(&plan.item_type)
        .ok_or_else(|| iteration_diagnostic("the yielded item IR type is unavailable"))?;
    let outcome_storage = outcome_storage_layout(&[OutcomeLayer::Optional], item_abi.layout);
    instructions.push(Instruction::ReserveAggregateSlot {
        slot_index: outcome_slot,
        layout: outcome_storage.layout,
    });

    let mut body_context = context.clone();
    let body_mark = body_context.local_mark();
    let region_mark = body_context.region_cleanup_mark();
    let item = define_item_binding(
        statement,
        &plan.item_type,
        &item_ir,
        &item_abi.ty,
        &mut body_context,
    )?;
    if let Some((slot_index, layout)) = item.aggregate_slot {
        instructions.push(Instruction::ReserveAggregateSlot { slot_index, layout });
    }

    let condition = body_context.next_bool_local_location()?;
    body_context.define_bool_local(hidden_name(statement, "present"));
    let mut condition_instructions = lower_step_call(
        statement,
        &plan,
        &iterator_name,
        outcome_slot,
        &outcome_storage,
        &item_ir,
        &body_context,
    )?;
    let tag_offset = checked_offset(outcome_storage.layers[0].tag_offset, "optional tag")?;
    condition_instructions.push(Instruction::SetBool {
        destination: condition,
        value: BoolValue::Const(false),
    });
    let mut success_instructions = item.load_instructions(
        AggregateLocation::Slot(outcome_slot),
        checked_offset(outcome_storage.payload_offset, "optional payload")?,
        item_abi.layout,
    );
    success_instructions.push(Instruction::SetBool {
        destination: condition,
        value: BoolValue::Const(true),
    });
    condition_instructions.push(Instruction::IfStoredOutcomeTag {
        source: AggregateLocation::Slot(outcome_slot),
        tag_offset,
        success_instructions,
        outcome_instructions: Vec::new(),
    });

    let lowered_body = lower_nonterminal_loop_block_statements(
        &statement.body.statements,
        statement.body.result.as_deref(),
        &mut body_context,
        body_mark,
        Some(CleanupScopeMark {
            locals: body_mark,
            regions: region_mark,
        }),
        &[],
        diagnostic_code,
        subject,
        sources,
    )?;
    let mut body_instructions = lowered_body.instructions;
    if !lowered_body.ends_execution && !instruction_list_ends_execution(&body_instructions) {
        body_instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut body_context,
            body_mark,
        )?);
    }
    instructions.push(Instruction::While {
        condition_instructions,
        condition: BoolValue::Location(condition),
        body_instructions,
    });

    instructions.extend(lower_scope_end_drops_for_locals_since(
        context,
        iterator_mark,
    )?);
    debug_assert!(context.aggregate_local_by_slot(iterator_slot).is_some());
    let _ = iterator_layout;
    Ok(instructions)
}

fn materialize_iterator(
    statement: &CollectionForStmt,
    plan: &TypecheckCollectionForPlan,
    iterator_name: &str,
    context: &mut LoweringContext,
) -> Result<(usize, crate::abi::ValueLayout, Vec<Instruction>), Vec<Diagnostic>> {
    let value = context
        .abi_value_for_type_expr(&plan.iterator_type)
        .ok_or_else(|| iteration_diagnostic("the concrete iterator ABI layout is unavailable"))?;
    if !matches!(
        value.ty,
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_)
    ) {
        return Err(iteration_diagnostic(
            "the concrete iterator is not an aggregate value",
        ));
    }
    let (root_source, resolved) = context
        .resolved_calls()
        .ok_or_else(|| iteration_diagnostic("resolution facts are unavailable"))?;
    let is_copy =
        type_expr_is_copy_aggregate_value_with_resolver(&plan.iterator_type, resolved, |source| {
            context.resolved_source(source)
        });
    let drop_kind = context.aggregate_drop_for_type_expr(&plan.iterator_type);
    let fields = aggregate_fields_from_type_expr_with_resolver(
        &plan.iterator_type,
        root_source,
        resolved,
        |source| context.resolved_source(source),
    )
    .unwrap_or_default();
    let slot_index = context.define_aggregate_local(
        iterator_name.to_string(),
        value.layout,
        is_copy,
        drop_kind,
        fields,
    );
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];

    match plan.source_mode {
        TypecheckCollectionForSourceMode::Direct => {
            let source = implicit_direct_move(&statement.source);
            let iterator_ir = context
                .ir_type_for_type_expr(&plan.iterator_type)
                .ok_or_else(|| {
                    iteration_diagnostic("the concrete iterator IR type is unavailable")
                })?;
            instructions.extend(lower_aggregate_return_expression_to_location(
                &source,
                &iterator_ir,
                AggregateLocation::Slot(slot_index),
                context.function_name(),
                resolved,
                context,
            )?);
            if let Some(name) = consumed_identifier_name(&statement.source) {
                context.mark_aggregate_local_moved(name);
            }
        }
        TypecheckCollectionForSourceMode::ReadonlyConversion
        | TypecheckCollectionForSourceMode::OwnedConversion => {
            let conversion = plan
                .conversion
                .as_ref()
                .ok_or_else(|| iteration_diagnostic("the collection conversion plan is missing"))?;
            let target = context.iteration_method_target(conversion).ok_or_else(|| {
                iteration_diagnostic("the collection conversion target is unavailable")
            })?;
            let return_type = context.call_return_type(&target).cloned().ok_or_else(|| {
                iteration_diagnostic("the collection conversion ABI is unavailable")
            })?;
            let call = synthetic_call(
                statement.span,
                &conversion.target_name,
                vec![statement.source.clone()],
            );
            let parameter_types = vec![receiver_parameter_type(conversion, statement.span)];
            let mut temporaries = TemporaryAllocator::new(context)?;
            let (argument_instructions, arguments) = lower_call_arguments_with_explicit_types(
                &call,
                &target,
                &conversion.target_name,
                context,
                &mut temporaries,
                Some(&parameter_types),
            )?;
            instructions.extend(argument_instructions);
            push_aggregate_call_instruction(
                &mut instructions,
                &return_type,
                AggregateLocation::Slot(slot_index),
                target,
                arguments,
                value.layout,
            );
            if plan.source_mode == TypecheckCollectionForSourceMode::OwnedConversion
                && let Some(name) = consumed_identifier_name(&statement.source)
            {
                context.mark_aggregate_local_moved(name);
            }
        }
    }
    Ok((slot_index, value.layout, instructions))
}

fn lower_step_call(
    statement: &CollectionForStmt,
    plan: &TypecheckCollectionForPlan,
    iterator_name: &str,
    outcome_slot: usize,
    storage: &crate::outcomes::storage::OutcomeStorageLayout,
    item_ir: &Type,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let target = context
        .iteration_method_target(&plan.step)
        .ok_or_else(|| iteration_diagnostic("the iterator step target is unavailable"))?;
    let receiver = receiver_expression(&plan.step, iterator_name, statement.name_span);
    let call = synthetic_call(statement.span, &plan.step.target_name, vec![receiver]);
    let parameter_types = vec![receiver_parameter_type(&plan.step, statement.span)];
    let mut temporaries = TemporaryAllocator::new(context)?;
    let (mut instructions, arguments) = lower_call_arguments_with_explicit_types(
        &call,
        &target,
        &plan.step.target_name,
        context,
        &mut temporaries,
        Some(&parameter_types),
    )?;
    instructions.push(Instruction::CallStoredOutcome {
        destination: AggregateLocation::Slot(outcome_slot),
        target,
        arguments,
        storage: storage.clone(),
        payload_type: item_ir.clone(),
    });
    Ok(instructions)
}

struct ItemBinding {
    destination: Option<ComposedOutcomeDestination>,
    aggregate_slot: Option<(usize, crate::abi::ValueLayout)>,
}

impl ItemBinding {
    fn load_instructions(
        &self,
        source: AggregateLocation,
        payload_offset: u32,
        layout: crate::abi::ValueLayout,
    ) -> Vec<Instruction> {
        if let Some(destination) = self.destination {
            vec![Instruction::LoadStoredOutcomePayload {
                destination,
                source,
                offset: payload_offset,
            }]
        } else if let Some((slot_index, _)) = self.aggregate_slot {
            vec![Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(slot_index),
                destination_offset: 0,
                source,
                source_offset: payload_offset,
                layout,
            }]
        } else {
            unreachable!("item binding has a scalar/view or aggregate destination")
        }
    }
}

fn define_item_binding(
    statement: &CollectionForStmt,
    item_ty: &TypeExpr,
    item_ir: &Type,
    item_abi: &AbiType,
    context: &mut LoweringContext,
) -> Result<ItemBinding, Vec<Diagnostic>> {
    let name = statement.name.clone();
    let destination = match item_ir {
        Type::I32 => {
            let location = context.next_i32_local_location()?;
            context.define_i32_local(name);
            Some(ComposedOutcomeDestination::I32(location))
        }
        Type::U8 => {
            let location = context.next_u8_local_location()?;
            context.define_u8_local(name);
            Some(ComposedOutcomeDestination::U8(location))
        }
        Type::Usize => {
            let location = context.next_usize_local_location()?;
            context.define_usize_local(name);
            Some(ComposedOutcomeDestination::Usize(location))
        }
        Type::Bool => {
            let location = context.next_bool_local_location()?;
            context.define_bool_local(name);
            Some(ComposedOutcomeDestination::Bool(location))
        }
        Type::Str => {
            let location = context.next_str_local_location()?;
            context.define_str_local(name);
            Some(ComposedOutcomeDestination::Str(location))
        }
        Type::Slice { .. } => {
            let location = context.next_slice_local_location()?;
            let TypecheckScalarViewKind::Slice(kind) = context
                .binding_scalar_view_kind(statement.name_span)
                .ok_or_else(|| iteration_diagnostic("the yielded slice kind is unavailable"))?
            else {
                return Err(iteration_diagnostic("the yielded item is not a slice fact"));
            };
            context.define_slice_local(name, kind, Some(item_ty.clone()));
            Some(ComposedOutcomeDestination::Slice(location))
        }
        Type::Borrow {
            is_readwrite,
            inner,
        } => {
            let location = context.next_usize_local_location()?;
            context.define_borrow_local(name, *is_readwrite, inner.as_ref().clone());
            Some(ComposedOutcomeDestination::Borrow(location))
        }
        Type::Aggregate { layout } | Type::DirectAggregate { layout, .. } => {
            let (root_source, resolved) = context
                .resolved_calls()
                .ok_or_else(|| iteration_diagnostic("resolution facts are unavailable"))?;
            let is_copy =
                type_expr_is_copy_aggregate_value_with_resolver(item_ty, resolved, |source| {
                    context.resolved_source(source)
                });
            let drop_kind = context.aggregate_drop_for_type_expr(item_ty);
            let fields = aggregate_fields_from_type_expr_with_resolver(
                item_ty,
                root_source,
                resolved,
                |source| context.resolved_source(source),
            )
            .unwrap_or_default();
            let slot = context.define_aggregate_local(name, *layout, is_copy, drop_kind, fields);
            return Ok(ItemBinding {
                destination: None,
                aggregate_slot: Some((slot, *layout)),
            });
        }
        _ => {
            return Err(iteration_diagnostic(
                "the yielded item kind is not supported by native lowering",
            ));
        }
    };
    let _ = item_abi;
    Ok(ItemBinding {
        destination,
        aggregate_slot: None,
    })
}

fn receiver_expression(method: &TypecheckIterationMethod, name: &str, span: ByteSpan) -> Expr {
    let identifier = Expr::Identifier(IdentifierExpr {
        span,
        name: name.to_string(),
    });
    match method.receiver_mode {
        MethodReceiverMode::Owned => Expr::Unary(UnaryExpr {
            span,
            operator: UnaryOperator::Move,
            operator_span: span,
            operand: Box::new(identifier),
        }),
        MethodReceiverMode::ReadonlyBorrow | MethodReceiverMode::ReadwriteBorrow => {
            Expr::Borrow(BorrowExpr {
                span,
                operator_span: span,
                is_readwrite: method.receiver_mode == MethodReceiverMode::ReadwriteBorrow,
                expression: Box::new(identifier),
            })
        }
    }
}

fn receiver_parameter_type(method: &TypecheckIterationMethod, span: ByteSpan) -> TypeExpr {
    match method.receiver_mode {
        MethodReceiverMode::Owned => method.self_ty.clone(),
        MethodReceiverMode::ReadonlyBorrow | MethodReceiverMode::ReadwriteBorrow => {
            TypeExpr::Borrow(BorrowType {
                span,
                is_readwrite: method.receiver_mode == MethodReceiverMode::ReadwriteBorrow,
                inner: Box::new(method.self_ty.clone()),
            })
        }
    }
}

fn synthetic_call(span: ByteSpan, target_name: &str, arguments: Vec<Expr>) -> CallExpr {
    CallExpr {
        span,
        callee: Box::new(Expr::Identifier(IdentifierExpr {
            span,
            name: target_name.to_string(),
        })),
        arguments_span: span,
        arguments,
    }
}

fn implicit_direct_move(source: &Expr) -> Expr {
    match source.without_groups() {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => source.clone(),
        expression => Expr::Unary(UnaryExpr {
            span: source.span(),
            operator: UnaryOperator::Move,
            operator_span: source.span(),
            operand: Box::new(expression.clone()),
        }),
    }
}

fn consumed_identifier_name(source: &Expr) -> Option<&str> {
    match source.without_groups() {
        Expr::Identifier(identifier) => Some(identifier.name.as_str()),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            match unary.operand.without_groups() {
                Expr::Identifier(identifier) => Some(identifier.name.as_str()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn hidden_name(statement: &CollectionForStmt, role: &str) -> String {
    format!(
        "<collection-for:{}:{}:{role}>",
        statement.name_span.start, statement.name_span.end
    )
}

fn checked_offset(offset: u64, role: &str) -> Result<u32, Vec<Diagnostic>> {
    u32::try_from(offset).map_err(|_| iteration_diagnostic(&format!("{role} exceeds u32")))
}

fn iteration_diagnostic(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8011",
        format!("collection iteration lowering failed: {detail}"),
    )]
}

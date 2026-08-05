//! Explicit IR lowering for lexical allocation regions and their cleanup obligations.

use super::aggregates::aggregate_fields_from_type_expr_with_resolver;
use super::context::{AggregateFieldKind, LoweringContext};
use super::control_flow::nonterminal::lower_nonterminal_region_body;
use super::functions::lower_scope_end_drops_for_locals_since;
use crate::abi::abi_value_from_type_expr_with_resolver;
use crate::ast::RegionStmt;
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, Instruction, UsizeLocation, UsizeValue};
use crate::source::SourceMap;

const REGION_ALLOCATOR_KIND: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CleanupScopeMark {
    pub(super) locals: usize,
    pub(super) regions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegionCleanup {
    state: UsizeLocation,
    parent_state: UsizeLocation,
    parent_kind: UsizeLocation,
}

impl RegionCleanup {
    fn instruction(&self) -> Instruction {
        Instruction::RegionRelease {
            state: UsizeValue::Location(self.state),
            parent_state: UsizeValue::Location(self.parent_state),
            parent_kind: UsizeValue::Location(self.parent_kind),
        }
    }
}

impl LoweringContext<'_> {
    pub(in crate::ir::lower) fn region_cleanup_mark(&self) -> usize {
        self.region_cleanups.len()
    }

    pub(in crate::ir::lower) fn push_region_cleanup(&mut self, cleanup: RegionCleanup) {
        self.region_cleanups.push(cleanup);
    }

    pub(in crate::ir::lower) fn region_cleanup_instructions_since(
        &self,
        mark: usize,
    ) -> Vec<Instruction> {
        self.region_cleanups
            .get(mark..)
            .unwrap_or(&[])
            .iter()
            .rev()
            .map(RegionCleanup::instruction)
            .collect()
    }

    pub(in crate::ir::lower) fn all_region_cleanup_instructions(&self) -> Vec<Instruction> {
        self.region_cleanup_instructions_since(0)
    }
}

pub(in crate::ir::lower) fn lower_nonterminal_region_statement(
    statement: &RegionStmt,
    context: &LoweringContext,
    loop_scope_mark: Option<CleanupScopeMark>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut body_context = context.clone();
    let local_mark = body_context.local_mark();
    let cleanup_mark = body_context.region_cleanup_mark();
    let mut instructions = lower_region_entry(statement, &mut body_context)?;

    let lowered = lower_nonterminal_region_body(
        &statement.body,
        &mut body_context,
        local_mark,
        loop_scope_mark,
        continue_instructions,
        diagnostic_code,
        subject,
        sources,
    )?;
    instructions.extend(lowered.instructions);
    if !lowered.ends_execution {
        instructions.extend(lower_scope_end_drops_for_locals_since(
            &mut body_context,
            local_mark,
        )?);
        instructions.extend(body_context.region_cleanup_instructions_since(cleanup_mark));
    }
    Ok(instructions)
}

fn lower_region_entry(
    statement: &RegionStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let parent_state = reserve_hidden_usize(context, "parent-state")?;
    let parent_kind = reserve_hidden_usize(context, "parent-kind")?;
    let state = reserve_hidden_usize(context, "state")?;

    let ty = context
        .binding_type_expr(statement.name_span)
        .ok_or_else(|| {
            region_lowering_diagnostic("the region allocator binding has no resolved type")
        })?;
    let Some((root_source, resolved)) = context.resolved_calls() else {
        return Err(region_lowering_diagnostic(
            "resolved type information is unavailable",
        ));
    };
    let value = abi_value_from_type_expr_with_resolver(&ty, resolved, |source| {
        context.resolved_source(source)
    })
    .map_err(|_| region_lowering_diagnostic("the allocator capability has no ABI layout"))?;
    let fields =
        aggregate_fields_from_type_expr_with_resolver(&ty, root_source, resolved, |source| {
            context.resolved_source(source)
        })
        .ok_or_else(|| {
            region_lowering_diagnostic("the allocator capability fields are unavailable")
        })?;
    let state_offset = usize_field_offset(&fields, "state")?;
    let kind_offset = usize_field_offset(&fields, "kind")?;
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), value.layout, false, None, fields);

    context.push_region_cleanup(RegionCleanup {
        state,
        parent_state,
        parent_kind,
    });

    Ok(vec![
        Instruction::SetUsize {
            destination: parent_state,
            value: UsizeValue::CurrentAllocationState,
        },
        Instruction::SetUsize {
            destination: parent_kind,
            value: UsizeValue::CurrentAllocationKind,
        },
        Instruction::RegionEnter { destination: state },
        Instruction::ReserveAggregateSlot {
            slot_index,
            layout: value.layout,
        },
        Instruction::StoreAggregateUsize {
            destination: AggregateLocation::Slot(slot_index),
            offset: state_offset,
            value: UsizeValue::Location(state),
        },
        Instruction::StoreAggregateUsize {
            destination: AggregateLocation::Slot(slot_index),
            offset: kind_offset,
            value: UsizeValue::Const(REGION_ALLOCATOR_KIND),
        },
        Instruction::SetCurrentAllocationContext {
            state: UsizeValue::Location(state),
            kind: UsizeValue::Const(REGION_ALLOCATOR_KIND),
        },
    ])
}

fn reserve_hidden_usize(
    context: &mut LoweringContext,
    purpose: &str,
) -> Result<UsizeLocation, Vec<Diagnostic>> {
    let location = context.next_usize_local_location()?;
    let UsizeLocation::Local(index) = location else {
        return Err(region_lowering_diagnostic(
            "region state requires a local scalar slot",
        ));
    };
    context.define_usize_local(format!("<region-{purpose}-{index}>"));
    Ok(location)
}

fn usize_field_offset(
    fields: &[super::context::AggregateField],
    name: &str,
) -> Result<u32, Vec<Diagnostic>> {
    fields
        .iter()
        .find(|field| field.name == name && field.kind == AggregateFieldKind::Usize)
        .map(|field| field.offset)
        .ok_or_else(|| {
            region_lowering_diagnostic(&format!("allocator capability is missing `{name}: usize`"))
        })
}

fn region_lowering_diagnostic(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8012",
        format!("IR cannot lower lexical region: {detail}"),
    )]
}

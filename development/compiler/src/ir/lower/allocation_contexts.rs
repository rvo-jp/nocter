//! Scoped current-allocation-context overrides used by typed literal construction.

use super::context::{AggregateFieldKind, LoweringContext};
use crate::ast::Expr;
use crate::diagnostics::Diagnostic;
use crate::ir::{Instruction, UsizeLocation, UsizeValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AllocationContextRestore {
    state: UsizeLocation,
    kind: UsizeLocation,
}

impl AllocationContextRestore {
    fn instruction(&self) -> Instruction {
        Instruction::SetCurrentAllocationContext {
            state: UsizeValue::Location(self.state),
            kind: UsizeValue::Location(self.kind),
        }
    }
}

pub(super) struct LoweredAllocationContextOverride<'a> {
    pub(super) enter: Vec<Instruction>,
    pub(super) context: LoweringContext<'a>,
    pub(super) restore: Instruction,
}

impl LoweringContext<'_> {
    pub(in crate::ir::lower) fn allocation_context_restore_instructions(&self) -> Vec<Instruction> {
        self.allocation_context_restores
            .iter()
            .rev()
            .map(AllocationContextRestore::instruction)
            .collect()
    }
}

pub(super) fn lower_allocation_context_override<'a>(
    allocator: &Expr,
    context: &mut LoweringContext<'a>,
) -> Result<LoweredAllocationContextOverride<'a>, Vec<Diagnostic>> {
    let parent_state = reserve_hidden_usize(context, "parent-state")?;
    let parent_kind = reserve_hidden_usize(context, "parent-kind")?;
    let selected_state = reserve_hidden_usize(context, "selected-state")?;
    let selected_kind = reserve_hidden_usize(context, "selected-kind")?;
    let (root, path) = established_place_path(allocator).ok_or_else(|| {
        allocation_context_diagnostic("the override operand is not an established place")
    })?;
    let state_path = append_field(&path, "state");
    let kind_path = append_field(&path, "kind");
    let state = context.aggregate_field(root, &state_path).ok_or_else(|| {
        allocation_context_diagnostic("the allocator capability has no `state` field")
    })?;
    let kind = context.aggregate_field(root, &kind_path).ok_or_else(|| {
        allocation_context_diagnostic("the allocator capability has no `kind` field")
    })?;
    if state.kind != AggregateFieldKind::Usize || kind.kind != AggregateFieldKind::Usize {
        return Err(allocation_context_diagnostic(
            "allocator `state` and `kind` fields must be `usize`",
        ));
    }

    let restore = AllocationContextRestore {
        state: parent_state,
        kind: parent_kind,
    };
    let mut override_context = context.clone();
    override_context
        .allocation_context_restores
        .push(restore.clone());
    Ok(LoweredAllocationContextOverride {
        enter: vec![
            Instruction::SetUsize {
                destination: parent_state,
                value: UsizeValue::CurrentAllocationState,
            },
            Instruction::SetUsize {
                destination: parent_kind,
                value: UsizeValue::CurrentAllocationKind,
            },
            Instruction::LoadAggregateUsize {
                destination: selected_state,
                source: state.source,
                offset: state.offset,
            },
            Instruction::LoadAggregateUsize {
                destination: selected_kind,
                source: kind.source,
                offset: kind.offset,
            },
            Instruction::SetCurrentAllocationContext {
                state: UsizeValue::Location(selected_state),
                kind: UsizeValue::Location(selected_kind),
            },
        ],
        context: override_context,
        restore: restore.instruction(),
    })
}

fn reserve_hidden_usize(
    context: &mut LoweringContext<'_>,
    purpose: &str,
) -> Result<UsizeLocation, Vec<Diagnostic>> {
    let location = context.next_usize_local_location()?;
    let UsizeLocation::Local(index) = location else {
        return Err(allocation_context_diagnostic(
            "allocation context state requires a local scalar slot",
        ));
    };
    context.define_usize_local(format!("<literal-context-{purpose}-{index}>"));
    Ok(location)
}

fn established_place_path(expression: &Expr) -> Option<(&str, String)> {
    match expression {
        Expr::Identifier(identifier) => Some((&identifier.name, String::new())),
        Expr::Member(member) => {
            let (root, path) = established_place_path(&member.object)?;
            Some((root, append_field(&path, &member.member)))
        }
        Expr::Group(group) => established_place_path(&group.expression),
        _ => None,
    }
}

fn append_field(path: &str, field: &str) -> String {
    if path.is_empty() {
        field.to_string()
    } else {
        format!("{path}.{field}")
    }
}

fn allocation_context_diagnostic(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8013",
        format!("IR cannot lower typed literal allocation context: {detail}"),
    )]
}

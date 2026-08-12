//! Editor presentation for typechecked collection-iteration plans.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::resolve::LocalSymbolKind;
use crate::source::ByteSpan;
use crate::typecheck::{TypecheckCollectionForPlan, TypecheckCollectionForSourceMode};
use crate::typecheck::{TypecheckSequenceSpreadMode, TypecheckSequenceSpreadPlan};

pub(crate) fn iteration_markdown_at_offset(
    _analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<String> {
    if let Some(plan) = file
        .typecheck_facts
        .sequence_spread_plans()
        .map(|(_, plan)| plan)
        .filter(|plan| {
            span_contains(plan.spread_span, offset) || span_contains(plan.source_span, offset)
        })
        .min_by_key(|plan| (plan.spread_span.len(), plan.spread_span.start))
    {
        return Some(sequence_spread_markdown(plan, &file.resolved));
    }
    let plan = file
        .typecheck_facts
        .collection_for_plans()
        .map(|(_, plan)| plan)
        .filter(|plan| {
            span_contains(plan.binding_span, offset) || span_contains(plan.source_span, offset)
        })
        .min_by_key(|plan| {
            let span = if span_contains(plan.binding_span, offset) {
                plan.binding_span
            } else {
                plan.source_span
            };
            (span.len(), span.start)
        })?;

    if span_contains(plan.binding_span, offset)
        && !file.resolved.local_symbols().any(|symbol| {
            symbol.name_span == plan.binding_span && symbol.kind == LocalSymbolKind::CollectionFor
        })
    {
        return None;
    }

    Some(iteration_markdown(plan, &file.resolved))
}

pub(crate) fn sequence_spread_operator_hover(
    _analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<(ByteSpan, String)> {
    let plan = file
        .typecheck_facts
        .sequence_spread_plans()
        .map(|(_, plan)| plan)
        .filter_map(|plan| {
            let operator_span = plan.operator_span;
            (operator_span.start <= offset && offset < operator_span.end)
                .then_some((operator_span, plan))
        })
        .min_by_key(|(_, plan)| (plan.spread_span.len(), plan.spread_span.start))?;
    Some((plan.0, sequence_spread_markdown(plan.1, &file.resolved)))
}

fn sequence_spread_markdown(
    plan: &TypecheckSequenceSpreadPlan,
    resolved: &crate::resolve::ResolveOutput,
) -> String {
    let mode = match plan.mode {
        TypecheckSequenceSpreadMode::Copy => "copy from readonly iteration",
        TypecheckSequenceSpreadMode::Readonly => "readonly reference spread",
        TypecheckSequenceSpreadMode::Move => "owned element transfer",
    };
    let mut lines = vec![
        format!("**Sequence spread:** {mode}."),
        format!(
            "**Source:** `{}`.",
            crate::typecheck::type_expr_presentation_label(&plan.source_type, resolved)
        ),
        format!(
            "**Iterator:** `{}`; **iterator item:** `{}`; **pack item:** `{}`.",
            crate::typecheck::type_expr_presentation_label(&plan.iterator_type, resolved),
            crate::typecheck::type_expr_presentation_label(&plan.iterator_item_type, resolved),
            crate::typecheck::type_expr_presentation_label(&plan.pack_item_type, resolved),
        ),
    ];
    lines.push(expansion_contract_markdown(
        plan.source_mode,
        &plan.source_type,
        &plan.iterator_type,
        resolved,
    ));
    lines.push(format!(
        "**Exact-count target:** `{}`; **step target:** `{}`.",
        plan.exact_size.target_name, plan.step.target_name
    ));
    lines.join("\n\n")
}

fn iteration_markdown(
    plan: &TypecheckCollectionForPlan,
    resolved: &crate::resolve::ResolveOutput,
) -> String {
    let mode = match plan.source_mode {
        TypecheckCollectionForSourceMode::Direct => "direct iterator transfer",
        TypecheckCollectionForSourceMode::ReadonlyConversion => "readonly source borrow",
        TypecheckCollectionForSourceMode::ReadwriteConversion => "readwrite source borrow",
        TypecheckCollectionForSourceMode::OwnedConversion => "owned source transfer",
    };
    let mut lines = vec![
        format!("**Iteration source:** {mode}."),
        format!(
            "**Iterator:** `{}`; **item:** `{}`.",
            crate::typecheck::type_expr_presentation_label(&plan.iterator_type, resolved),
            crate::typecheck::type_expr_presentation_label(&plan.item_type, resolved)
        ),
    ];

    lines.push(expansion_contract_markdown(
        plan.source_mode,
        &plan.source_type,
        &plan.iterator_type,
        resolved,
    ));
    lines.push(format!(
        "**Step target:** `{}` (statically selected conformance).",
        plan.step.target_name
    ));

    lines.join("\n\n")
}

fn expansion_contract_markdown(
    mode: TypecheckCollectionForSourceMode,
    source: &crate::ast::TypeExpr,
    iterator: &crate::ast::TypeExpr,
    resolved: &crate::resolve::ResolveOutput,
) -> String {
    if mode == TypecheckCollectionForSourceMode::Direct {
        return "**Expansion:** none; the source already is an iterator.".to_string();
    }
    let prefix = match mode {
        TypecheckCollectionForSourceMode::Direct => unreachable!(),
        TypecheckCollectionForSourceMode::ReadonlyConversion => "&",
        TypecheckCollectionForSourceMode::ReadwriteConversion => "&+",
        TypecheckCollectionForSourceMode::OwnedConversion => "",
    };
    format!(
        "**Expansion:** `operator (...{prefix}{}): {}`.",
        crate::typecheck::type_expr_presentation_label(source, resolved),
        crate::typecheck::type_expr_presentation_label(iterator, resolved),
    )
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

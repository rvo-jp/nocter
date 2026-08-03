//! Editor presentation for typechecked collection-iteration plans.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::type_expr_display_lossy;
use crate::resolve::LocalSymbolKind;
use crate::source::ByteSpan;
use crate::typecheck::{TypecheckCollectionForPlan, TypecheckCollectionForSourceMode};

pub(crate) fn iteration_markdown_at_offset(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<String> {
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

    Some(iteration_markdown(analysis, plan))
}

fn iteration_markdown(analysis: &CompileUnitAnalysis, plan: &TypecheckCollectionForPlan) -> String {
    let mode = match plan.source_mode {
        TypecheckCollectionForSourceMode::Direct => "direct iterator transfer",
        TypecheckCollectionForSourceMode::ReadonlyConversion => "readonly source borrow",
        TypecheckCollectionForSourceMode::OwnedConversion => "owned source transfer",
    };
    let mut lines = vec![
        format!("**Iteration source:** {mode}."),
        format!(
            "**Iterator:** `{}`; **item:** `{}`.",
            type_expr_display_lossy(&plan.iterator_type),
            type_expr_display_lossy(&plan.item_type)
        ),
    ];

    if let Some(conversion) = &plan.conversion {
        lines.push(format!(
            "**Conversion target:** `{}` (statically selected conformance).",
            conversion.target_name
        ));
    } else {
        lines.push("**Conversion target:** none; the source already is an iterator.".to_string());
    }
    lines.push(format!(
        "**Step target:** `{}` (statically selected conformance).",
        plan.step.target_name
    ));

    let mut allocation_roles = Vec::new();
    if plan.conversion.as_ref().is_some_and(|method| {
        callable_uses_current_allocation_context(analysis, method.declaration_span)
    }) {
        allocation_roles.push("conversion");
    }
    if callable_uses_current_allocation_context(analysis, plan.step.declaration_span) {
        allocation_roles.push("step");
    }
    if !allocation_roles.is_empty() {
        lines.push(format!(
            "**Allocation effect:** {} uses the current allocation context.",
            allocation_roles.join(" and ")
        ));
    }

    lines.join("\n\n")
}

fn callable_uses_current_allocation_context(
    analysis: &CompileUnitAnalysis,
    declaration_span: ByteSpan,
) -> bool {
    analysis
        .callable_semantic_facts
        .get(declaration_span)
        .is_some_and(|facts| facts.needs_current_allocation_context)
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

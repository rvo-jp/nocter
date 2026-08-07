//! Editor presentation and navigation for persisted conversion plans.

use super::FileAnalysis;
use crate::analysis::editor_targets::SourceTarget;
use crate::source::ByteSpan;
use crate::typecheck::{TypecheckConversionKind, TypecheckConversionPlan, TypecheckFacts};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConversionEditorInfo {
    pub(crate) focus_span: ByteSpan,
    pub(crate) label: String,
    pub(crate) documentation: String,
}

pub(crate) fn conversion_editor_info_at_offset(
    file: &FileAnalysis,
    offset: usize,
) -> Option<ConversionEditorInfo> {
    let plan = conversion_plan_at_offset(&file.typecheck_facts, offset)?;
    let source = crate::typecheck::type_expr_presentation_label(&plan.source_ty, &file.resolved);
    let target = crate::typecheck::type_expr_presentation_label(&plan.target_ty, &file.resolved);
    let focus_span = plan.operator_span?;
    let (kind, detail) = match &plan.kind {
        TypecheckConversionKind::LosslessInteger => (
            "lossless integer conversion",
            "Every value representable by the source type is representable by the target type."
                .to_string(),
        ),
        TypecheckConversionKind::CapabilityWeakening => (
            "borrow capability weakening",
            "The result keeps the same loan while exposing readonly access.".to_string(),
        ),
        TypecheckConversionKind::BorrowCoercion(coercion) => {
            let owner =
                crate::typecheck::type_expr_presentation_label(&coercion.self_ty, &file.resolved);
            let target =
                crate::typecheck::type_expr_presentation_label(&coercion.target_ty, &file.resolved);
            (
                "type-owned borrow coercion",
                format!(
                    "Selected `{}{} as {target} from self`. The result remains attached to the source loan.",
                    coercion.receiver_mode.source_prefix(),
                    owner,
                ),
            )
        }
    };
    Some(ConversionEditorInfo {
        focus_span,
        label: format!("{source} as {target}"),
        documentation: format!("**Conversion:** {kind}.\n\n{detail}"),
    })
}

pub(crate) fn conversion_definition_target_at_offset(
    facts: &TypecheckFacts,
    offset: usize,
) -> Option<SourceTarget> {
    let plan = conversion_plan_at_offset(facts, offset)?;
    let TypecheckConversionKind::BorrowCoercion(coercion) = &plan.kind else {
        return None;
    };
    Some(SourceTarget::new(plan.operator_span?, coercion.focus_span))
}

fn conversion_plan_at_offset(
    facts: &TypecheckFacts,
    offset: usize,
) -> Option<&TypecheckConversionPlan> {
    facts
        .conversion_plans()
        .filter_map(|(_, plan)| plan.operator_span.map(|span| (span, plan)))
        .filter(|(span, _)| span.start <= offset && offset < span.end)
        .min_by_key(|(span, _)| (span.len(), span.start))
        .map(|(_, plan)| plan)
}

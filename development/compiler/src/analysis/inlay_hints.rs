//! Inlay hints projected from retained compiler facts without editor-only inference.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::analysis::presentation::SemanticDetail;
use crate::source::{ByteSpan, SourceMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlayHintKind {
    Type,
    Semantic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlayHintInfo {
    pub(crate) offset: usize,
    pub(crate) label: String,
    pub(crate) kind: InlayHintKind,
    pub(crate) tooltip: Option<String>,
}

pub(crate) fn inlay_hints_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    requested: std::ops::RangeInclusive<usize>,
) -> Vec<InlayHintInfo> {
    let Some(source) = sources.get(file.ast.span.source) else {
        return Vec::new();
    };
    let text = source.text();
    let mut hints = file
        .typecheck_facts
        .binding_type_label_entries()
        .filter(|(span, _)| span.source == file.ast.span.source)
        .filter(|(span, _)| requested.contains(&span.end))
        .filter(|(span, _)| !has_explicit_type_annotation(text, *span))
        .map(|(span, label)| InlayHintInfo {
            offset: span.end,
            label: format!(": {label}"),
            kind: InlayHintKind::Type,
            tooltip: Some("Inferred type".to_string()),
        })
        .collect::<Vec<_>>();

    for (declaration, fact) in analysis.callable_semantic_facts.entries() {
        let Some(anchors) = file.callable_declarations.get(declaration) else {
            continue;
        };
        if declaration.source != file.ast.span.source || !requested.contains(&anchors.signature_end)
        {
            continue;
        }
        if fact.needs_current_allocation_context {
            hints.push(InlayHintInfo {
                offset: anchors.signature_end,
                label: " allocates".to_string(),
                kind: InlayHintKind::Semantic,
                tooltip: Some(
                    "Uses the current allocation context directly or transitively.".to_string(),
                ),
            });
        }
        if anchors.explicit_result_provenance.is_none()
            && let Some(provenance) =
                crate::analysis::presentation::semantic_details_for_callable(sources, fact)
                    .into_iter()
                    .find_map(|detail| match detail {
                        SemanticDetail::ResultProvenance(provenance) => Some(provenance),
                        SemanticDetail::AllocationEffect(_) => None,
                    })
        {
            hints.push(InlayHintInfo {
                offset: anchors.signature_end,
                label: " from inferred storage".to_string(),
                kind: InlayHintKind::Semantic,
                tooltip: Some(provenance.render_markdown()),
            });
        }
    }

    hints.sort_by(|left, right| {
        (left.offset, hint_order(left.kind), &left.label).cmp(&(
            right.offset,
            hint_order(right.kind),
            &right.label,
        ))
    });
    hints
}

fn has_explicit_type_annotation(text: &str, name_span: ByteSpan) -> bool {
    text.get(name_span.end..)
        .and_then(|suffix| suffix.chars().find(|character| !character.is_whitespace()))
        == Some(':')
}

const fn hint_order(kind: InlayHintKind) -> u8 {
    match kind {
        InlayHintKind::Type => 0,
        InlayHintKind::Semantic => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn reports_only_inferred_binding_types() {
        let text = "func main(): i32 {\n    let inferred = 1\n    let explicit: i32 = 2\n    return inferred\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().unwrap();
        let hints = inlay_hints_for_file_analysis(&sources, &analysis, file, 0..=text.len());
        assert!(hints.iter().any(|hint| hint.label == ": i32"));
        assert_eq!(
            hints
                .iter()
                .filter(|hint| hint.kind == InlayHintKind::Type)
                .count(),
            1
        );
    }

    #[test]
    fn reports_retained_allocation_and_provenance_facts() {
        let text = r#"primitive allocate(): usize

func build(): usize {
    return allocate()
}

func label(): &str {
    return "static"
}
"#;
        let (sources, analysis) =
            crate::analysis::test_support::analyze_text_with_trusted_current_allocation_operation(
                text, "allocate",
            );
        let file = analysis.root_file().unwrap();
        let hints = inlay_hints_for_file_analysis(&sources, &analysis, file, 0..=text.len());
        assert!(hints.iter().any(|hint| {
            hint.offset == text.find("usize {").unwrap() + "usize".len()
                && hint.label == " allocates"
        }));
        assert!(hints.iter().any(|hint| {
            hint.offset == text.find("&str {").unwrap() + "&str".len()
                && hint.label == " from inferred storage"
                && hint
                    .tooltip
                    .as_deref()
                    .is_some_and(|tooltip| tooltip.contains("static storage"))
        }));
    }

    #[test]
    fn suppresses_inferred_provenance_from_the_ast_clause() {
        let text = r#"func label(): &str from static {
    return "static"
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().unwrap();
        let hints = inlay_hints_for_file_analysis(&sources, &analysis, file, 0..=text.len());
        assert!(
            hints
                .iter()
                .all(|hint| hint.label != " from inferred storage")
        );
    }
}

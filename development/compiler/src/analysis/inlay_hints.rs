//! Inlay hints projected from retained compiler facts without editor-only inference.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::source::{ByteSpan, SourceMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlayHintKind {
    Type,
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
        .typed_hir
        .binding_type_label_entries()
        .filter_map(|(symbol, label)| {
            file.resolved
                .local_symbol(symbol)
                .map(|symbol| (symbol.name_span, label))
        })
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

    let _ = analysis;
    hints.sort_by(|left, right| (left.offset, &left.label).cmp(&(right.offset, &right.label)));
    hints
}

fn has_explicit_type_annotation(text: &str, name_span: ByteSpan) -> bool {
    text.get(name_span.end..)
        .and_then(|suffix| suffix.chars().find(|character| !character.is_whitespace()))
        == Some(':')
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
    fn does_not_expose_inferred_allocation_or_provenance_facts() {
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
        assert!(hints.is_empty(), "{hints:?}");
    }

    #[test]
    fn inferred_opaque_binding_hint_hides_witness() {
        let text = r#"interface Source {
    pub type Item
    pub method &self.get(): Self.Item
}
struct Number { value: i32 }
conform Source for Number {
    type Item = i32
    method &self.get(): i32 { return self.value }
}
func make(): some Source<Item = i32> { return Number { value: 7 } }
func read(): i32 {
    let source = make()
    return source.get()
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().unwrap();
        let hints = inlay_hints_for_file_analysis(&sources, &analysis, file, 0..=text.len());
        let source_hint = hints
            .iter()
            .find(|hint| hint.label.contains("some Source"))
            .expect("expected opaque binding hint");
        assert_eq!(source_hint.label, ": some Source<Item = i32>");
        assert!(!source_hint.label.contains("Number"));
    }
}

//! Editor-facing presentation of shared value-provenance facts.

use super::CompileUnitAnalysis;
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::{StorageOriginFact, ValueProvenanceFact};

pub(crate) fn result_provenance_markdown(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    declaration: ByteSpan,
) -> Option<String> {
    let fact = analysis.callable_semantic_facts.get(declaration)?;
    let provenance = fact.result.as_ref()?;
    if matches!(provenance, ValueProvenanceFact::Independent) {
        return None;
    }
    Some(format!(
        "**Result provenance:** {}.",
        value_label(sources, provenance)
    ))
}

fn value_label(sources: &SourceMap, value: &ValueProvenanceFact) -> String {
    match value {
        ValueProvenanceFact::Independent => "storage-independent".to_string(),
        ValueProvenanceFact::Origins(origins) => origins
            .iter()
            .map(|origin| origin_label(sources, origin))
            .collect::<Vec<_>>()
            .join(" + "),
        ValueProvenanceFact::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            let mut parts = Vec::new();
            if let Some(fallback) = fallback {
                parts.push(format!("other storage: {}", value_label(sources, fallback)));
            }
            parts.extend(
                fields.iter().map(|(name, value)| {
                    format!("field `{name}`: {}", value_label(sources, value))
                }),
            );
            parts.extend(elements.iter().map(|(index, value)| {
                format!("element `{index}`: {}", value_label(sources, value))
            }));
            format!("aggregate ({})", parts.join(", "))
        }
        ValueProvenanceFact::Fallible { success, error } => {
            let mut parts = Vec::new();
            if let Some(success) = success {
                parts.push(format!("success: {}", value_label(sources, success)));
            }
            if let Some(error) = error {
                parts.push(format!("error: {}", value_label(sources, error)));
            }
            format!("fallible ({})", parts.join(", "))
        }
    }
}

fn origin_label(sources: &SourceMap, origin: &StorageOriginFact) -> String {
    match origin {
        StorageOriginFact::Static => "static storage".to_string(),
        StorageOriginFact::Input(span) => {
            format!("input `{}`", span_label(sources, *span, "parameter"))
        }
        StorageOriginFact::Scope(span) => {
            format!("scope `{}`", span_label(sources, *span, "binding"))
        }
        StorageOriginFact::Region(span) => {
            format!("region `{}`", span_label(sources, *span, "region"))
        }
        StorageOriginFact::Unknown => "unknown storage".to_string(),
    }
}

fn span_label<'a>(sources: &'a SourceMap, span: ByteSpan, fallback: &'a str) -> &'a str {
    sources
        .get(span.source)
        .and_then(|source| source.text().get(span.start..span.end))
        .unwrap_or(fallback)
}

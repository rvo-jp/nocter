//! User-facing projection of compiler-owned allocation and storage facts.

use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::{CallableSemanticFact, StorageOriginFact, ValueProvenanceFact};
use std::collections::BTreeSet;

const MAX_PRESENTED_ORIGINS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SemanticDetail {
    AllocationEffect(AllocationEffectPresentation),
    ResultProvenance(ResultProvenancePresentation),
}

impl SemanticDetail {
    pub(crate) fn render_markdown(&self) -> String {
        match self {
            Self::AllocationEffect(effect) => effect.render_markdown(),
            Self::ResultProvenance(provenance) => provenance.render_markdown(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllocationEffectPresentation {
    subject: Option<String>,
}

impl AllocationEffectPresentation {
    pub(crate) fn current_context() -> Self {
        Self { subject: None }
    }

    pub(crate) fn current_context_for(subject: impl Into<String>) -> Self {
        Self {
            subject: Some(subject.into()),
        }
    }

    pub(crate) fn render_markdown(&self) -> String {
        match &self.subject {
            Some(subject) => {
                format!("**Allocation effect:** {subject} uses the current allocation context.")
            }
            None => "**Allocation effect:** uses the current allocation context.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResultProvenancePresentation {
    Shared(OriginList),
    Fallible {
        success: Option<OriginList>,
        error: Option<OriginList>,
    },
}

impl ResultProvenancePresentation {
    pub(crate) fn from_current_allocation_context() -> Self {
        Self::Shared(OriginList::one("the current allocation context"))
    }

    pub(crate) fn render_markdown(&self) -> String {
        let body = match self {
            Self::Shared(origins) => format!("storage from {}", origins.render()),
            Self::Fallible { success, error } => {
                let mut branches = Vec::new();
                if let Some(success) = success {
                    branches.push(format!("success storage from {}", success.render()));
                }
                if let Some(error) = error {
                    branches.push(format!("error storage from {}", error.render()));
                }
                branches.join("; ")
            }
        };
        format!("**Result provenance:** {body}.")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OriginList {
    labels: Vec<String>,
    omitted: usize,
}

impl OriginList {
    fn one(label: impl Into<String>) -> Self {
        Self {
            labels: vec![label.into()],
            omitted: 0,
        }
    }

    fn from_labels(labels: BTreeSet<String>) -> Option<Self> {
        if labels.is_empty() {
            return None;
        }
        let omitted = labels.len().saturating_sub(MAX_PRESENTED_ORIGINS);
        Some(Self {
            labels: labels.into_iter().take(MAX_PRESENTED_ORIGINS).collect(),
            omitted,
        })
    }

    fn render(&self) -> String {
        let mut labels = self.labels.clone();
        if self.omitted > 0 {
            labels.push(format!("{} other source(s)", self.omitted));
        }
        labels.join(" + ")
    }
}

pub(crate) fn semantic_details_for_callable(
    sources: &SourceMap,
    fact: &CallableSemanticFact,
) -> Vec<SemanticDetail> {
    let mut details = Vec::new();
    if fact.needs_current_allocation_context {
        details.push(SemanticDetail::AllocationEffect(
            AllocationEffectPresentation::current_context(),
        ));
    }
    if let Some(provenance) = fact
        .result
        .as_ref()
        .and_then(|value| result_provenance_presentation(sources, fact, value))
    {
        details.push(SemanticDetail::ResultProvenance(provenance));
    }
    details
}

pub(crate) fn semantic_details_for_callable_result(
    sources: &SourceMap,
    fact: &CallableSemanticFact,
    result_type: &crate::ast::TypeExpr,
    resolved: &crate::resolve::ResolveOutput,
) -> Vec<SemanticDetail> {
    semantic_details_for_callable(sources, fact)
        .into_iter()
        .filter(|detail| {
            !matches!(detail, SemanticDetail::ResultProvenance(_))
                || crate::typecheck::type_expr_carries_storage(result_type, resolved)
        })
        .collect()
}

fn result_provenance_presentation(
    sources: &SourceMap,
    fact: &CallableSemanticFact,
    value: &ValueProvenanceFact,
) -> Option<ResultProvenancePresentation> {
    if let ValueProvenanceFact::Fallible { success, error } = value {
        let success = success
            .as_deref()
            .and_then(|value| origin_list(sources, fact, value));
        let error = error
            .as_deref()
            .and_then(|value| origin_list(sources, fact, value));
        return match (&success, &error) {
            (None, None) => None,
            (Some(success), Some(error)) if success == error => {
                Some(ResultProvenancePresentation::Shared(success.clone()))
            }
            _ => Some(ResultProvenancePresentation::Fallible { success, error }),
        };
    }
    origin_list(sources, fact, value).map(ResultProvenancePresentation::Shared)
}

fn origin_list(
    sources: &SourceMap,
    fact: &CallableSemanticFact,
    value: &ValueProvenanceFact,
) -> Option<OriginList> {
    let mut labels = BTreeSet::new();
    collect_origin_labels(sources, fact, value, &mut labels);
    OriginList::from_labels(labels)
}

fn collect_origin_labels(
    sources: &SourceMap,
    fact: &CallableSemanticFact,
    value: &ValueProvenanceFact,
    labels: &mut BTreeSet<String>,
) {
    match value {
        ValueProvenanceFact::Independent => {}
        ValueProvenanceFact::Origins(origins) => {
            for origin in origins {
                if let Some(label) = origin_label(sources, fact, origin) {
                    labels.insert(label);
                }
            }
        }
        ValueProvenanceFact::Aggregate {
            fallback,
            fields,
            elements,
        } => {
            if let Some(fallback) = fallback {
                collect_origin_labels(sources, fact, fallback, labels);
            }
            for value in fields.values().chain(elements.values()) {
                collect_origin_labels(sources, fact, value, labels);
            }
        }
        ValueProvenanceFact::Fallible { success, error } => {
            for value in success.iter().chain(error.iter()) {
                collect_origin_labels(sources, fact, value, labels);
            }
        }
    }
}

fn origin_label(
    sources: &SourceMap,
    fact: &CallableSemanticFact,
    origin: &StorageOriginFact,
) -> Option<String> {
    match origin {
        StorageOriginFact::Static => Some("static storage".to_string()),
        StorageOriginFact::CurrentAllocationContext => {
            Some("the current allocation context".to_string())
        }
        StorageOriginFact::Input(span) if fact.storage_inputs.contains(span) => Some(format!(
            "input `{}`",
            span_label(sources, *span, "parameter")
        )),
        StorageOriginFact::Input(_) | StorageOriginFact::Scope(_) => None,
        StorageOriginFact::Region(span) => {
            Some(format!("region `{}`", span_label(sources, *span, "region")))
        }
        StorageOriginFact::Unknown => Some("unknown storage".to_string()),
    }
}

fn span_label<'a>(sources: &'a SourceMap, span: ByteSpan, fallback: &'a str) -> &'a str {
    sources
        .get(span.source)
        .and_then(|source| source.text().get(span.start..span.end))
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceMap;
    use std::collections::{BTreeMap, HashSet};

    #[test]
    fn aggregates_hide_representation_and_scalar_dataflow() {
        let text = "len storage";
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text.to_string());
        let len = ByteSpan::new(source, 0, 3);
        let storage = ByteSpan::new(source, 4, 11);
        let fact = CallableSemanticFact {
            result: Some(ValueProvenanceFact::Aggregate {
                fallback: None,
                fields: BTreeMap::from([
                    (
                        "end_index".to_string(),
                        ValueProvenanceFact::Origins(vec![StorageOriginFact::Input(len)]),
                    ),
                    (
                        "private_storage".to_string(),
                        ValueProvenanceFact::Origins(vec![StorageOriginFact::Input(storage)]),
                    ),
                ]),
                elements: BTreeMap::new(),
            }),
            result_may_contain_allocation: false,
            needs_current_allocation_context: false,
            storage_inputs: HashSet::from([storage]),
        };

        let details = semantic_details_for_callable(&sources, &fact);
        assert_eq!(
            details,
            vec![SemanticDetail::ResultProvenance(
                ResultProvenancePresentation::Shared(OriginList::one("input `storage`"))
            )]
        );
        let markdown = details[0].render_markdown();
        assert_eq!(
            markdown,
            "**Result provenance:** storage from input `storage`."
        );
        assert!(!markdown.contains("private_storage"));
        assert!(!markdown.contains("len"));
    }

    #[test]
    fn origin_lists_have_a_stable_size_bound() {
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, "a b c d e f".to_string());
        let spans = (0..6)
            .map(|index| ByteSpan::new(source, index * 2, index * 2 + 1))
            .collect::<Vec<_>>();
        let fact = CallableSemanticFact {
            result: Some(ValueProvenanceFact::Origins(
                spans
                    .iter()
                    .copied()
                    .map(StorageOriginFact::Input)
                    .collect(),
            )),
            result_may_contain_allocation: false,
            needs_current_allocation_context: false,
            storage_inputs: spans.iter().copied().collect(),
        };

        let markdown = semantic_details_for_callable(&sources, &fact)[0].render_markdown();
        assert!(markdown.contains("2 other source(s)"), "{markdown}");
        assert!(!markdown.contains("input `e`"), "{markdown}");
    }

    #[test]
    fn fallible_results_preserve_only_meaningful_storage_branches() {
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, "value".to_string());
        let value = ByteSpan::new(source, 0, 5);
        let fact = CallableSemanticFact {
            result: Some(ValueProvenanceFact::Fallible {
                success: Some(Box::new(ValueProvenanceFact::Origins(vec![
                    StorageOriginFact::Input(value),
                ]))),
                error: Some(Box::new(ValueProvenanceFact::Origins(vec![
                    StorageOriginFact::Static,
                ]))),
            }),
            result_may_contain_allocation: false,
            needs_current_allocation_context: false,
            storage_inputs: HashSet::from([value]),
        };

        assert_eq!(
            semantic_details_for_callable(&sources, &fact)[0].render_markdown(),
            "**Result provenance:** success storage from input `value`; error storage from static storage."
        );
    }
}

use super::model::{
    IndexedOccurrence, PackageSemanticIndex, StableIdentityKind, StableSemanticIdentity,
    StableSourceIdentity, StableSourceSpan,
};
use crate::analysis::occurrences::{SemanticIdentity, SemanticOccurrence};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::package::PackageGraph;
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) struct PackageSemanticIndexBuilder<'a> {
    generation: u64,
    graph: Option<&'a PackageGraph>,
    sources: HashMap<StableSourceIdentity, Arc<str>>,
    occurrences: Vec<IndexedOccurrence>,
}

pub(crate) fn stable_semantic_identity_at(
    sources: &SourceMap,
    file: &FileAnalysis,
    offset: usize,
    graph: Option<&PackageGraph>,
) -> Option<StableSemanticIdentity> {
    let mut builder = PackageSemanticIndexBuilder::new(0, graph);
    builder.identity_at(sources, file, offset)
}

impl<'a> PackageSemanticIndexBuilder<'a> {
    pub(crate) fn new(generation: u64, graph: Option<&'a PackageGraph>) -> Self {
        Self {
            generation,
            graph,
            sources: HashMap::new(),
            occurrences: Vec::new(),
        }
    }

    pub(crate) fn add_analysis(&mut self, sources: &SourceMap, analysis: &CompileUnitAnalysis) {
        for file in &analysis.files {
            for occurrence in file.occurrences.iter() {
                let Some(indexed) = self.index_occurrence(sources, occurrence) else {
                    continue;
                };
                self.occurrences.push(indexed);
            }
        }
    }

    pub(crate) fn identity_at(
        &mut self,
        sources: &SourceMap,
        file: &FileAnalysis,
        offset: usize,
    ) -> Option<StableSemanticIdentity> {
        let occurrence = file.occurrences.at_offset(offset)?;
        self.stable_identity(sources, occurrence.identity?)
    }

    pub(crate) fn finish(mut self) -> PackageSemanticIndex {
        self.occurrences.sort_by(|left, right| {
            (
                &left.span.source,
                left.span.start,
                left.span.end,
                &left.identity,
            )
                .cmp(&(
                    &right.span.source,
                    right.span.start,
                    right.span.end,
                    &right.identity,
                ))
        });
        self.occurrences.dedup_by(|left, right| {
            left.span == right.span
                && left.identity == right.identity
                && left.role == right.role
                && left.kind == right.kind
        });
        PackageSemanticIndex::new(self.generation, self.sources, self.occurrences)
    }

    fn index_occurrence(
        &mut self,
        sources: &SourceMap,
        occurrence: &SemanticOccurrence,
    ) -> Option<IndexedOccurrence> {
        Some(IndexedOccurrence {
            identity: self.stable_identity(sources, occurrence.identity?)?,
            span: self.stable_span(sources, occurrence.focus_span)?,
            role: occurrence.role,
            kind: occurrence.kind,
        })
    }

    fn stable_identity(
        &mut self,
        sources: &SourceMap,
        identity: SemanticIdentity,
    ) -> Option<StableSemanticIdentity> {
        let (kind, declaration) = match identity {
            SemanticIdentity::Declaration(span) => (StableIdentityKind::Declaration, span),
            SemanticIdentity::Member(span) => (StableIdentityKind::Member, span),
            SemanticIdentity::Local(span) => (StableIdentityKind::Local, span),
            SemanticIdentity::GenericParameter(span) => {
                (StableIdentityKind::GenericParameter, span)
            }
        };
        Some(StableSemanticIdentity {
            kind,
            declaration: self.stable_span(sources, declaration)?,
        })
    }

    fn stable_span(&mut self, sources: &SourceMap, span: ByteSpan) -> Option<StableSourceSpan> {
        let source = sources.get(span.source)?;
        let absolute_path = source.absolute_path().cloned();
        let package = absolute_path
            .as_deref()
            .and_then(|path| self.graph?.package_containing(path))
            .map(|package| package.id().as_str().to_string());
        let identity =
            StableSourceIdentity::new(package, absolute_path, source.display_path().to_string());
        self.sources
            .entry(identity.clone())
            .or_insert_with(|| Arc::from(source.text()));
        Some(StableSourceSpan {
            source: identity,
            start: span.start,
            end: span.end,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_namespace_import_text;

    #[test]
    fn joins_semantic_identity_across_independent_source_maps() {
        let root = "use lib/math.answer\nfunc first(): i32 { return answer() }\n";
        let module = "pub func answer(): i32 { return 42 }\n";
        let (first_sources, first) = analyze_namespace_import_text(root, module);
        let (second_sources, second) = analyze_namespace_import_text(root, module);
        let mut builder = PackageSemanticIndexBuilder::new(7, None);
        builder.add_analysis(&first_sources, &first);
        builder.add_analysis(&second_sources, &second);
        let index = builder.finish();

        assert_eq!(index.generation(), 7);
        let answer_occurrences = index
            .occurrences()
            .iter()
            .filter(|occurrence| {
                index
                    .source_text(&occurrence.span.source)
                    .and_then(|text| text.get(occurrence.span.start..occurrence.span.end))
                    == Some("answer")
            })
            .count();
        assert_eq!(answer_occurrences, 3);
    }
}

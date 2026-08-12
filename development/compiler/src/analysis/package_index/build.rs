use super::model::{
    IndexedExport, IndexedExportKind, IndexedOccurrence, PackageSemanticIndex, StableIdentityKind,
    StableSemanticIdentity, StableSourceIdentity, StableSourceSpan,
};
use crate::analysis::occurrences::{SemanticIdentity, SemanticOccurrence};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::Item;
use crate::package::PackageGraph;
use crate::resolve::SymbolKind;
use crate::semantic::{DefinitionKind, SemanticDb};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) struct PackageSemanticIndexBuilder<'a> {
    generation: u64,
    graph: Option<&'a PackageGraph>,
    sources: HashMap<StableSourceIdentity, Arc<str>>,
    occurrences: Vec<IndexedOccurrence>,
    exports: Vec<IndexedExport>,
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
            exports: Vec::new(),
        }
    }

    pub(crate) fn add_analysis(&mut self, sources: &SourceMap, analysis: &CompileUnitAnalysis) {
        for file in &analysis.files {
            self.add_exports(sources, analysis, file);
            for occurrence in file.occurrences.iter() {
                let Some(indexed) =
                    self.index_occurrence(sources, &analysis.semantic_db, occurrence)
                else {
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
        self.stable_identity(sources, &file.resolved.semantic_db, occurrence.identity?)
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
        self.exports.sort_by(|left, right| {
            (&left.name, &left.absolute_path, left.kind).cmp(&(
                &right.name,
                &right.absolute_path,
                right.kind,
            ))
        });
        self.exports.dedup();
        PackageSemanticIndex::new(
            self.generation,
            self.sources,
            self.occurrences,
            self.exports,
        )
    }

    fn add_exports(
        &mut self,
        sources: &SourceMap,
        analysis: &CompileUnitAnalysis,
        file: &FileAnalysis,
    ) {
        let Some(source) = sources.get(file.ast.span.source) else {
            return;
        };
        let Some(absolute_path) = source.absolute_path().cloned() else {
            return;
        };
        for item in &file.ast.items {
            let export = match item {
                Item::Function(function)
                    if !function.visibility.is_private() && function.owner.is_none() =>
                {
                    Some((
                        function.name.clone(),
                        IndexedExportKind::Function,
                        function.visibility,
                    ))
                }
                Item::Primitive(primitive) if !primitive.visibility.is_private() => Some((
                    primitive.name.clone(),
                    IndexedExportKind::Function,
                    primitive.visibility,
                )),
                Item::TypeAlias(alias) if !alias.visibility.is_private() => Some((
                    alias.name.clone(),
                    IndexedExportKind::Type,
                    alias.visibility,
                )),
                Item::Struct(struct_) if !struct_.visibility.is_private() => Some((
                    struct_.name.clone(),
                    IndexedExportKind::Type,
                    struct_.visibility,
                )),
                Item::Enum(enum_) if !enum_.visibility.is_private() => Some((
                    enum_.name.clone(),
                    IndexedExportKind::Type,
                    enum_.visibility,
                )),
                Item::Interface(interface) if !interface.visibility.is_private() => Some((
                    interface.name.clone(),
                    IndexedExportKind::Type,
                    interface.visibility,
                )),
                Item::Import(import) if !import.visibility.is_private() => {
                    match (
                        import.path.segments.last(),
                        analysis.import_sources.get(&import.path.span),
                    ) {
                        (Some(target), Some(source)) => {
                            resolved_import_kind(analysis, source.source, target)
                                .map(|kind| (import.alias.name.clone(), kind, import.visibility))
                        }
                        _ => None,
                    }
                }
                Item::FromImport(import) if !import.visibility.is_private() => {
                    let Some(visibility) = file
                        .resolved
                        .visibility_boundary(import.visibility, file.ast.span.source)
                    else {
                        continue;
                    };
                    for imported in &import.names {
                        let name = imported.local_name();
                        let Some(kind) = file
                            .resolved
                            .symbols
                            .symbol_by_name(name)
                            .and_then(|symbol| export_kind(&symbol.kind))
                        else {
                            continue;
                        };
                        self.exports.push(IndexedExport {
                            name: name.to_string(),
                            kind,
                            absolute_path: absolute_path.clone(),
                            visibility: visibility.clone(),
                        });
                    }
                    None
                }
                Item::Import(_)
                | Item::FromImport(_)
                | Item::Function(_)
                | Item::Test(_)
                | Item::Primitive(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_)
                | Item::Interface(_)
                | Item::Instance(_)
                | Item::Conformance(_)
                | Item::Construct(_) => None,
                Item::Destruct(_) => None,
            };
            if let Some((name, kind, visibility)) = export
                && let Some(visibility) = file
                    .resolved
                    .visibility_boundary(visibility, file.ast.span.source)
            {
                self.exports.push(IndexedExport {
                    name,
                    kind,
                    absolute_path: absolute_path.clone(),
                    visibility,
                });
            }
        }
    }

    fn index_occurrence(
        &mut self,
        sources: &SourceMap,
        semantic_db: &SemanticDb,
        occurrence: &SemanticOccurrence,
    ) -> Option<IndexedOccurrence> {
        Some(IndexedOccurrence {
            identity: self.stable_identity(sources, semantic_db, occurrence.identity?)?,
            span: self.stable_span(sources, occurrence.focus_span)?,
            role: occurrence.role,
            kind: occurrence.kind,
        })
    }

    fn stable_identity(
        &mut self,
        sources: &SourceMap,
        semantic_db: &SemanticDb,
        identity: SemanticIdentity,
    ) -> Option<StableSemanticIdentity> {
        let (kind, declaration) = match identity {
            SemanticIdentity::Local(span) => (StableIdentityKind::Local, span),
            SemanticIdentity::Definition(id) => {
                let definition = semantic_db.definition(id)?;
                let kind = if definition.kind == DefinitionKind::GenericParameter {
                    StableIdentityKind::GenericParameter
                } else if definition.owner.is_some()
                    || definition.kind == DefinitionKind::AssociatedFunction
                {
                    StableIdentityKind::Member
                } else {
                    StableIdentityKind::Declaration
                };
                (kind, definition.anchor)
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

fn export_kind(kind: &SymbolKind) -> Option<IndexedExportKind> {
    match kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) => Some(IndexedExportKind::Function),
        SymbolKind::Type(_) => Some(IndexedExportKind::Type),
        SymbolKind::Imported(_) => None,
    }
}

fn resolved_import_kind(
    analysis: &CompileUnitAnalysis,
    source: crate::source::SourceId,
    name: &str,
) -> Option<IndexedExportKind> {
    analysis
        .file_by_source(source)?
        .resolved
        .symbols
        .symbol_by_name(name)
        .and_then(|symbol| export_kind(&symbol.kind))
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

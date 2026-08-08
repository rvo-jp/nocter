use super::{ImportKind, ImportSource, ImportSourceMap};
use crate::ast::{AstFile, FromImportItem};
use crate::source::{ByteSpan, SourceId};
use std::collections::HashMap;

/// Indexes semantic modules independently from their physical source files.
///
/// Same-module source imports form connected components. Each component is
/// represented by one declaration-order-independent aggregate AST while every
/// original span keeps its physical `SourceId` for diagnostics and navigation.
pub(super) struct MergedModules {
    module_by_source: HashMap<SourceId, SourceId>,
    modules: HashMap<SourceId, AstFile>,
}

impl MergedModules {
    pub(super) fn new(files: &[AstFile], import_sources: &ImportSourceMap) -> Self {
        let mut parents = files
            .iter()
            .map(|ast| (ast.span.source, ast.span.source))
            .collect::<HashMap<_, _>>();

        for (span, imported) in import_sources {
            if imported.kind == ImportKind::Source
                && parents.contains_key(&span.source)
                && parents.contains_key(&imported.source)
            {
                union(&mut parents, span.source, imported.source);
            }
        }

        let mut module_by_source = HashMap::new();
        for ast in files {
            let root = find(&mut parents, ast.span.source);
            module_by_source.insert(ast.span.source, root);
        }

        let mut modules: HashMap<SourceId, AstFile> = HashMap::new();
        for ast in files {
            let root = module_by_source[&ast.span.source];
            modules
                .entry(root)
                .and_modify(|module| module.items.extend(ast.items.clone()))
                .or_insert_with(|| ast.clone());
        }

        Self {
            module_by_source,
            modules,
        }
    }
}

pub(super) struct ModuleIndex<'a> {
    merged: &'a MergedModules,
}

impl<'a> ModuleIndex<'a> {
    pub(super) fn new(merged: &'a MergedModules) -> Self {
        Self { merged }
    }

    pub(super) fn import_ast(
        &self,
        item: &FromImportItem,
        import_sources: &ImportSourceMap,
    ) -> Option<(&'a AstFile, ImportSource)> {
        self.import_ast_for_span(item.path.span, import_sources)
    }

    pub(super) fn import_ast_for_span(
        &self,
        path_span: ByteSpan,
        import_sources: &ImportSourceMap,
    ) -> Option<(&'a AstFile, ImportSource)> {
        let import_source = *import_sources.get(&path_span)?;
        self.ast_for_source(import_source.source)
            .map(|ast| (ast, import_source))
    }

    pub(super) fn ast_for_source(&self, source: SourceId) -> Option<&'a AstFile> {
        let root = self.merged.module_by_source.get(&source)?;
        self.merged.modules.get(root)
    }

    pub(super) fn asts(&self) -> impl Iterator<Item = &'a AstFile> + '_ {
        self.merged.modules.values()
    }
}

fn find(parents: &mut HashMap<SourceId, SourceId>, source: SourceId) -> SourceId {
    let parent = parents[&source];
    if parent == source {
        return source;
    }
    let root = find(parents, parent);
    parents.insert(source, root);
    root
}

fn union(parents: &mut HashMap<SourceId, SourceId>, left: SourceId, right: SourceId) {
    let left_root = find(parents, left);
    let right_root = find(parents, right);
    if left_root != right_root {
        parents.insert(right_root, left_root);
    }
}

pub(super) fn is_relative_module_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

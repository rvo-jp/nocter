use super::{ImportSource, ImportSourceMap};
use crate::ast::{AstFile, FromImportItem};
use crate::source::{ByteSpan, SourceId};
use crate::source_modules::SourceModuleMap;
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
        let membership = SourceModuleMap::new(files, import_sources);
        let module_by_source = files
            .iter()
            .map(|ast| {
                let source = ast.span.source;
                (source, membership.module(source).unwrap_or(source))
            })
            .collect::<HashMap<_, _>>();

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

pub(super) fn is_relative_module_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

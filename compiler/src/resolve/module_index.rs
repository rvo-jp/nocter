use super::{ImportSource, ImportSourceMap};
use crate::ast::{AstFile, FromImportItem};
use crate::source::{ByteSpan, SourceId};
use std::collections::HashMap;

pub(super) struct ModuleIndex<'a> {
    by_source: HashMap<SourceId, &'a AstFile>,
}

impl<'a> ModuleIndex<'a> {
    pub(super) fn new(files: &'a [AstFile]) -> Self {
        let mut by_source = HashMap::new();

        for ast in files {
            by_source.insert(ast.span.source, ast);
        }

        Self { by_source }
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
        self.by_source
            .get(&import_source.source)
            .copied()
            .map(|ast| (ast, import_source))
    }
}

pub(super) fn is_relative_module_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

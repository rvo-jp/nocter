//! Directory-module membership shared by semantic source consumers.

use crate::ast::AstFile;
use crate::resolve::{ImportKind, ImportSourceMap};
use crate::source::SourceId;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SourceModuleMap {
    module_by_source: HashMap<SourceId, SourceId>,
}

impl SourceModuleMap {
    pub(crate) fn new(files: &[AstFile], import_sources: &ImportSourceMap) -> Self {
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

        let module_by_source = files
            .iter()
            .map(|ast| {
                let source = ast.span.source;
                (source, find(&mut parents, source))
            })
            .collect();
        Self { module_by_source }
    }

    pub(crate) fn module(&self, source: SourceId) -> Option<SourceId> {
        self.module_by_source.get(&source).copied()
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
    if left_root == right_root {
        return;
    }
    let (root, child) = if left_root.raw() <= right_root.raw() {
        (left_root, right_root)
    } else {
        (right_root, left_root)
    };
    parents.insert(child, root);
}

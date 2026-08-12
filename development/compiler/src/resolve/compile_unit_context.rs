//! Immutable compile-unit indexes and declaration qualification caches.
//!
//! A compile unit resolves several source views over the same module graph. Type names imported by
//! a declaration source do not depend on which view is currently being resolved, so retaining that
//! environment here avoids repeating recursive import traversal for every source output.

use super::ImportSourceMap;
use super::imports::ImportedTypeName;
use super::module_index::MergedModules;
use crate::ast::AstFile;
use crate::semantic::SemanticDb;
use crate::source::SourceId;
use crate::source_modules::SourceModuleMap;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) struct ResolveCompileUnitContext {
    pub(super) semantic_db: Arc<SemanticDb>,
    pub(super) source_modules: SourceModuleMap,
    pub(super) merged_modules: MergedModules,
    pub(super) imported_type_names: RefCell<HashMap<SourceId, Vec<ImportedTypeName>>>,
}

impl ResolveCompileUnitContext {
    pub(crate) fn new(files: &[AstFile], import_sources: &ImportSourceMap) -> Self {
        Self {
            semantic_db: Arc::new(SemanticDb::from_files(files)),
            source_modules: SourceModuleMap::new(files, import_sources),
            merged_modules: MergedModules::new(files, import_sources),
            imported_type_names: RefCell::new(HashMap::new()),
        }
    }
}
